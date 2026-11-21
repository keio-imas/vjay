use super::ffi::{self, Handle};
use crate::video_io::metal::{self, Fence};
use objc2::rc::Retained;
use std::{
    ffi::CString,
    path::PathBuf,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, SyncSender},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
use vjay_application::{
    media::{FrameDelivery, MediaMetrics},
    ports::*,
};

#[derive(Default)]
struct Completion {
    key: Option<u64>,
    error: Option<String>,
}
pub(super) struct NativeMovie {
    handle: Arc<Handle>,
    texture: wgpu::Texture,
    fence: Fence,
    request: Option<SyncSender<()>>,
    wanted: Arc<AtomicU64>,
    completion: Arc<(Mutex<Completion>, Condvar)>,
    ready: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    child: super::super::process::ChildSlot,
    worker: Option<JoinHandle<()>>,
    requested: u64,
    delivered: Option<u64>,
    frames: u64,
}
impl NativeMovie {
    pub fn open(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        context: Arc<ffi::Context>,
        path: PathBuf,
        extent: Extent,
        ffmpeg: PathBuf,
        ffprobe: PathBuf,
    ) -> PortResult<Self> {
        let texture = crate::video_io::texture(device, extent);
        metal::initialize_texture(device, queue, &texture);
        let native_texture = unsafe { texture.as_hal::<wgpu::hal::api::Metal>() }
            .ok_or("Expected a Metal video texture")?;
        let raw = unsafe {
            ffi::vjay_movie_create(
                context.0,
                native_texture.raw_handle() as *const _ as ffi::Raw,
            )
        };
        drop(native_texture);
        if raw.is_null() {
            return Err("Cannot create native movie source".into());
        }
        let handle = Arc::new(Handle(raw));
        let completion = Arc::new((Mutex::new(Completion::default()), Condvar::new()));
        let ready = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let wanted = Arc::new(AtomicU64::new(0));
        let child = Arc::new(Mutex::new(None));
        let (request, requests) = mpsc::sync_channel(1);
        let (h, c, r, s, w, ch) = (
            handle.clone(),
            completion.clone(),
            ready.clone(),
            stop.clone(),
            wanted.clone(),
            child.clone(),
        );
        let worker = std::thread::Builder::new()
            .name("vjay-native-video".into())
            .spawn(move || {
                let run = || -> PortResult<()> {
                    if s.load(Ordering::Acquire) {
                        return Ok(());
                    }
                    let info = super::super::probe(&ffprobe, &path)?;
                    let prepared =
                        super::super::acceleration::prepare(&ffmpeg, &info, extent, true, &ch, &s)?;
                    if s.load(Ordering::Acquire) {
                        return Ok(());
                    }
                    let path = CString::new(prepared.path.as_os_str().as_encoded_bytes())
                        .map_err(|_| "Invalid movie path")?;
                    let mut error = [0; 1024];
                    if !unsafe {
                        ffi::vjay_movie_open(h.0, path.as_ptr(), error.as_mut_ptr(), error.len())
                    } {
                        return Err(ffi::message(&error));
                    }
                    while requests.recv().is_ok() && !s.load(Ordering::Acquire) {
                        while requests.try_recv().is_ok() {}
                        let key = w.load(Ordering::Acquire);
                        let result = unsafe {
                            ffi::vjay_movie_decode(h.0, key, error.as_mut_ptr(), error.len())
                        };
                        if result < 0 {
                            return Err(ffi::message(&error));
                        }
                        if result > 0 {
                            c.0.lock().map_err(|e| e.to_string())?.key = Some(key);
                            r.store(true, Ordering::Release);
                            c.1.notify_all();
                        }
                    }
                    Ok(())
                };
                if let Err(error) = run() {
                    if let Ok(mut done) = c.0.lock() {
                        done.error = Some(error);
                    }
                    c.1.notify_all();
                }
            })
            .map_err(|e| e.to_string())?;
        request.send(()).map_err(|e| e.to_string())?;
        Ok(Self {
            handle,
            texture,
            fence: Fence::new(device)?,
            request: Some(request),
            wanted,
            completion,
            ready,
            stop,
            child,
            worker: Some(worker),
            requested: 0,
            delivered: None,
            frames: 0,
        })
    }
}
impl crate::gpu::GpuVideoDecoder for NativeMovie {
    fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }
    fn duration(&self) -> f64 {
        unsafe { ffi::vjay_movie_duration(self.handle.0) }
    }
    fn ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }
    fn failed(&self) -> bool {
        self.completion.0.lock().is_ok_and(|c| c.error.is_some())
    }
    fn status(&self) -> Option<&str> {
        Some(if self.ready() {
            "VideoToolbox · native GPU frames"
        } else {
            "Preparing native GPU video…"
        })
    }
    fn metrics(&self) -> MediaMetrics {
        MediaMetrics {
            frames_delivered: self.frames,
            cpu_pixel_bytes: 0,
            decoder_starts: unsafe { ffi::vjay_movie_stat(self.handle.0, 0) },
            seek_resets: unsafe { ffi::vjay_movie_stat(self.handle.0, 1) },
            cache_hits: unsafe { ffi::vjay_movie_stat(self.handle.0, 2) },
        }
    }
    fn frame_at(
        &mut self,
        seconds: f64,
        delivery: FrameDelivery,
        queue: &wgpu::Queue,
    ) -> PortResult<bool> {
        if !seconds.is_finite() {
            return Err("Invalid video playhead".into());
        }
        let fps = unsafe { ffi::vjay_movie_fps(self.handle.0) };
        // Normalized controls use f32. Snap only sub-frame rounding noise at
        // exact frame boundaries; do not round ordinary playback up a frame.
        let key = (seconds.max(0.0) * fps + 1e-4).floor() as u64;
        if key != self.requested {
            self.wanted.store(key, Ordering::Release);
            unsafe { ffi::vjay_movie_request(self.handle.0, key) };
            if let Some(request) = &self.request {
                let _ = request.try_send(());
            }
            self.requested = key;
        }
        let mut completion = self.completion.0.lock().map_err(|e| e.to_string())?;
        if delivery == FrameDelivery::Exact {
            let (result, timeout) = self
                .completion
                .1
                .wait_timeout_while(completion, Duration::from_secs(120), |c| {
                    c.key != Some(key) && c.error.is_none()
                })
                .map_err(|e| e.to_string())?;
            completion = result;
            if timeout.timed_out() {
                return Err("Native movie frame timed out".into());
            }
        }
        if let Some(error) = &completion.error {
            return Err(error.clone());
        }
        if self.delivered == Some(key) {
            return Ok(false);
        }
        let completed = completion.key;
        drop(completion);
        self.fence.value += 1;
        let start = Instant::now();
        loop {
            if unsafe {
                ffi::vjay_movie_encode(
                    self.handle.0,
                    key,
                    Retained::as_ptr(&self.fence.event).cast_mut().cast(),
                    self.fence.value,
                )
            } {
                self.fence.wait_in_wgpu(queue)?;
                self.delivered = Some(key);
                self.frames += 1;
                return Ok(true);
            }
            if delivery == FrameDelivery::Latest {
                // A busy decoder may finish behind the current clock. Present its
                // newest completed request instead of waiting forever for an exact
                // match to a continuously moving target. Exact export never does this.
                if let Some(completed) = completed
                    && self.delivered != Some(completed)
                    && unsafe {
                        ffi::vjay_movie_encode(
                            self.handle.0,
                            completed,
                            Retained::as_ptr(&self.fence.event).cast_mut().cast(),
                            self.fence.value,
                        )
                    }
                {
                    self.fence.wait_in_wgpu(queue)?;
                    self.delivered = Some(completed);
                    self.frames += 1;
                    return Ok(true);
                }
                return Ok(false);
            }
            if start.elapsed() > Duration::from_secs(10) {
                return Err("Native video GPU queue timed out".into());
            }
            std::thread::sleep(Duration::from_micros(100));
        }
    }
}
impl Drop for NativeMovie {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        unsafe { ffi::vjay_movie_request(self.handle.0, u64::MAX) };
        self.request.take();
        if let Ok(mut child) = self.child.lock()
            && let Some(child) = child.as_mut()
        {
            let _ = child.kill();
        }
        if let Some(worker) = self.worker.take() {
            // Teardown may wait for native I/O; never join it on the UI thread.
            let _ = std::thread::Builder::new()
                .name("vjay-video-reaper".into())
                .spawn(move || {
                    let _ = worker.join();
                });
        }
    }
}
