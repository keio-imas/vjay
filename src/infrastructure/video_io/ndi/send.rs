use super::*;
use std::sync::{
    Condvar, Mutex,
    atomic::{AtomicBool, Ordering},
};
pub type Pool = Arc<Mutex<Vec<Vec<u8>>>>;
pub struct Mailbox {
    pub frame: Mutex<Option<Vec<u8>>>,
    pub ready: Condvar,
    pub stop: AtomicBool,
}
pub struct Worker {
    pub source_name: String,
    pub mailbox: Arc<Mailbox>,
    pub pool: Pool,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Worker {
    pub fn new(api: Arc<api::Api>, name: &str, extent: Extent) -> PortResult<Self> {
        let name = api::string(name)?;
        let mailbox = Arc::new(Mailbox {
            frame: Mutex::new(None),
            ready: Condvar::new(),
            stop: AtomicBool::new(false),
        });
        let pending = mailbox.clone();
        let pool = Arc::new(Mutex::new(
            (0..3)
                .map(|_| vec![0; (extent.width * extent.height * 4) as usize])
                .collect::<Vec<_>>(),
        ));
        let buffers = pool.clone();
        let (start, started) = std::sync::mpsc::sync_channel(1);
        let thread = std::thread::Builder::new()
            .name("ndi-send".into())
            .spawn(move || unsafe {
                let sender = (api.send_create)(&api::SendSettings {
                    name: name.as_ptr(),
                    groups: std::ptr::null(),
                    video_clock: false,
                    audio_clock: false,
                });
                if sender.is_null() {
                    let _ = start.send(None);
                    return;
                }
                let full_name = api::name((api.send_name)(sender)).unwrap_or_default();
                let _ = start.send(Some(full_name));
                let mut held: Option<Vec<u8>> = None;
                loop {
                    let mut slot = match pending.frame.lock() {
                        Ok(s) => s,
                        Err(_) => break,
                    };
                    while slot.is_none() && !pending.stop.load(Ordering::Acquire) {
                        slot = match pending.ready.wait(slot) {
                            Ok(s) => s,
                            Err(_) => return,
                        };
                    }
                    if pending.stop.load(Ordering::Acquire) {
                        break;
                    }
                    let mut data = slot.take().expect("ready frame");
                    drop(slot);
                    let frame = api::Video {
                        width: extent.width as i32,
                        height: extent.height as i32,
                        aspect: extent.width as f32 / extent.height as f32,
                        stride: (extent.width * 4) as i32,
                        data: data.as_mut_ptr(),
                        ..Default::default()
                    };
                    (api.send_video)(sender, &frame);
                    if let Some(old) = held.replace(data)
                        && let Ok(mut pool) = buffers.lock()
                    {
                        pool.push(old);
                    }
                }
                (api.send_video)(sender, std::ptr::null());
                (api.send_destroy)(sender);
                drop(held);
            })
            .map_err(|e| e.to_string())?;
        let source_name = started
            .recv_timeout(std::time::Duration::from_secs(3))
            .ok()
            .flatten();
        if source_name.is_none() {
            mailbox.stop.store(true, Ordering::Release);
            mailbox.ready.notify_all();
            let _ = thread.join();
            return Err("NDI sender initialization failed".into());
        }
        Ok(Self {
            source_name: source_name.unwrap_or_default(),
            mailbox,
            pool,
            thread: Some(thread),
        })
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.mailbox.stop.store(true, Ordering::Release);
        self.mailbox.ready.notify_all();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
