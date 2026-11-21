use super::*;
use std::{
    path::Path,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
};
pub(crate) mod api;
mod readback;
mod receive;
mod send;
pub struct NdiFactory {
    api: Arc<api::Api>,
    names: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}
impl NdiFactory {
    pub fn new(path: &Path) -> PortResult<Self> {
        let api = api::Api::load(path)?;
        let names = Arc::new(Mutex::new(Vec::new()));
        let latest = names.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let done = stop.clone();
        let sdk = api.clone();
        let worker = std::thread::Builder::new()
            .name("ndi-discovery".into())
            .spawn(move || unsafe {
                let finder = (sdk.find_create)(&api::Finder {
                    local: true,
                    groups: std::ptr::null(),
                    ips: std::ptr::null(),
                });
                if finder.is_null() {
                    return;
                }
                while !done.load(Ordering::Acquire) {
                    (sdk.find_wait)(finder, 250);
                    let mut count = 0;
                    let sources = (sdk.find_sources)(finder, &mut count);
                    if !sources.is_null() && count <= 4096 {
                        let list = (0..count)
                            .filter_map(|i| api::name(sources.add(i as usize)))
                            .collect();
                        if let Ok(mut names) = latest.lock() {
                            *names = list;
                        }
                    }
                }
                (sdk.find_destroy)(finder);
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            api,
            names,
            stop,
            worker: Some(worker),
        })
    }
}
impl VideoIoFactory for NdiFactory {
    fn sources(&self) -> Vec<String> {
        self.names.lock().map(|v| v.clone()).unwrap_or_default()
    }
    fn receiver(
        &self,
        r: &StreamRequest,
        d: &wgpu::Device,
        _: &wgpu::Queue,
        e: Extent,
    ) -> PortResult<Box<dyn VideoReceiver>> {
        Ok(Box::new(receive::Receiver::new(
            self.api.clone(),
            &r.name,
            d,
            e,
        )?))
    }
    fn sender(
        &self,
        r: &StreamRequest,
        d: &wgpu::Device,
        _: &wgpu::Queue,
        e: Extent,
    ) -> PortResult<Box<dyn VideoSender>> {
        Ok(Box::new(readback::Sender::new(
            self.api.clone(),
            &r.name,
            d,
            e,
        )?))
    }
}
impl Drop for NdiFactory {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}
