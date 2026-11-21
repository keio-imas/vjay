use super::*;
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};
struct Frame {
    rgba: Vec<u8>,
    width: u32,
    height: u32,
}
pub struct Receiver {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    input: Option<wgpu::Texture>,
    blit: crate::gpu::blit::Blit,
    mailbox: Arc<Mutex<Option<Frame>>>,
    pool: Arc<Mutex<Vec<Vec<u8>>>>,
    stop: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Receiver {
    pub fn new(
        sdk: Arc<api::Api>,
        name: &str,
        device: &wgpu::Device,
        extent: Extent,
    ) -> PortResult<Self> {
        if name.is_empty() {
            return Err("Choose an NDI source".into());
        }
        let name = api::string(name)?;
        let mailbox = Arc::new(Mutex::new(None::<Frame>));
        let latest = mailbox.clone();
        let pool = Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
        let buffers = pool.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let done = stop.clone();
        let worker = std::thread::Builder::new()
            .name("ndi-receive".into())
            .spawn(move || unsafe {
                let receiver = (sdk.recv_create)(&api::Receive {
                    source: api::Source {
                        name: name.as_ptr(),
                        address: std::ptr::null(),
                    },
                    color: 2,
                    bandwidth: 100,
                    fields: false,
                    name: std::ptr::null(),
                });
                if receiver.is_null() {
                    return;
                }
                let hardware =
                    api::string("<ndi_video_codec type=\"hardware\"/>").expect("constant metadata");
                (sdk.recv_metadata)(
                    receiver,
                    &api::Metadata {
                        length: hardware.as_bytes().len() as i32,
                        timecode: i64::MAX,
                        data: hardware.as_ptr(),
                    },
                );
                // The Runtime decides whether hardware decoding is available.
                while !done.load(Ordering::Acquire) {
                    let mut video = api::Video::default();
                    let kind = (sdk.recv_capture)(
                        receiver,
                        &mut video,
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        100,
                    );
                    if kind != 1 {
                        continue;
                    }
                    if video.width > 0
                        && video.height > 0
                        && video.width <= 4096
                        && video.height <= 4096
                        && video.stride >= video.width * 4
                        && video.stride <= video.width * 4 + 4096
                        && !video.data.is_null()
                        && [u32::from_le_bytes(*b"RGBA"), u32::from_le_bytes(*b"RGBX")]
                            .contains(&video.fourcc)
                    {
                        let size = video.width as usize * video.height as usize * 4;
                        let mut rgba = buffers
                            .lock()
                            .ok()
                            .and_then(|mut p| p.pop())
                            .unwrap_or_default();
                        rgba.resize(size, 0);
                        let row = video.width as usize * 4;
                        for y in 0..video.height as usize {
                            rgba[y * row..(y + 1) * row].copy_from_slice(
                                std::slice::from_raw_parts(
                                    video.data.add(y * video.stride as usize),
                                    row,
                                ),
                            );
                        }
                        if video.fourcc == u32::from_le_bytes(*b"RGBX") {
                            for p in rgba.as_chunks_mut::<4>().0 {
                                p[3] = 255;
                            }
                        }
                        if let Ok(mut slot) = latest.lock()
                            && let Some(old) = slot.replace(Frame {
                                rgba,
                                width: video.width as u32,
                                height: video.height as u32,
                            })
                            && let Ok(mut p) = buffers.lock()
                        {
                            p.push(old.rgba);
                        }
                    }
                    (sdk.recv_free)(receiver, &video);
                }
                (sdk.recv_destroy)(receiver);
            })
            .map_err(|e| e.to_string())?;
        let texture = super::super::texture(device, extent);
        let view = texture.create_view(&Default::default());
        Ok(Self {
            texture,
            view,
            input: None,
            blit: crate::gpu::blit::Blit::new(device, wgpu::TextureFormat::Rgba8Unorm),
            mailbox,
            pool,
            stop,
            worker: Some(worker),
        })
    }
}
impl VideoReceiver for Receiver {
    fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }
    fn update(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
    ) -> PortResult<()> {
        if let Some(frame) = self.mailbox.try_lock().ok().and_then(|mut s| s.take()) {
            if self
                .input
                .as_ref()
                .is_none_or(|t| t.width() != frame.width || t.height() != frame.height)
            {
                let texture =
                    super::super::texture(device, Extent::new(frame.width, frame.height)?);
                self.blit
                    .prepare(device, &texture.create_view(&Default::default()));
                self.input = Some(texture);
            }
            let input = self.input.as_ref().expect("received format");
            queue.write_texture(
                input.as_image_copy(),
                &frame.rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(frame.width * 4),
                    rows_per_image: Some(frame.height),
                },
                input.size(),
            );
            self.blit.encode(encoder, &self.view);
            if let Ok(mut pool) = self.pool.lock() {
                pool.push(frame.rgba);
            }
        }
        Ok(())
    }
}
impl Drop for Receiver {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}
