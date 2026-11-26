use super::{
    reader::TICKS,
    transfer::{self, Recycled},
    worker::{Completion, Mailbox, Worker},
};
use std::{
    path::PathBuf,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, SyncSender},
    },
    thread::JoinHandle,
    time::Duration,
};
use vjay_application::{
    media::{FrameDelivery, MediaMetrics},
    ports::{Extent, PortResult},
};

pub(super) struct NativeMovie {
    texture: wgpu::Texture,
    device: wgpu::Device,
    requests: Option<SyncSender<()>>,
    recycled: SyncSender<Recycled>,
    wanted: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
    completion: Mailbox,
    worker: Option<JoinHandle<()>>,
    delivered: Option<u64>,
    frames: u64,
}

impl NativeMovie {
    pub fn open(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        path: PathBuf,
        extent: Extent,
    ) -> PortResult<Self> {
        let texture = transfer::output(device, queue, extent);
        let completion = Arc::new((Mutex::new(Completion::default()), Condvar::new()));
        let wanted = Arc::new(AtomicU64::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let (requests, receive_requests) = mpsc::sync_channel(1);
        let (recycled, receive_recycled) = mpsc::sync_channel(3);
        let worker = Worker {
            device: device.clone(),
            path,
            extent,
            completion: completion.clone(),
            wanted: wanted.clone(),
            stop: stop.clone(),
            requests: receive_requests,
            recycled: receive_recycled,
        };
        let worker = std::thread::Builder::new()
            .name("vjay-dxva-video".into())
            .spawn(move || worker.run())
            .map_err(|e| e.to_string())?;
        requests.send(()).map_err(|e| e.to_string())?;
        Ok(Self {
            texture,
            device: device.clone(),
            requests: Some(requests),
            recycled,
            wanted,
            stop,
            completion,
            worker: Some(worker),
            delivered: None,
            frames: 0,
        })
    }
}

impl crate::gpu::GpuVideoDecoder for NativeMovie {
    fn request_frame(&mut self, seconds: f64) -> PortResult<()> {
        if !seconds.is_finite() {
            return Err("Invalid video playhead".into());
        }
        let done = self.completion.0.lock().map_err(|e| e.to_string())?;
        let fps = if done.fps > 0.0 { done.fps } else { 30.0 };
        let seconds = if done.duration > 0.0 {
            seconds.clamp(0.0, (done.duration - 1.0 / fps).max(0.0))
        } else {
            seconds.max(0.0)
        };
        let key = ((((seconds * fps + 1e-4).floor() / fps) * TICKS)
            .round()
            .min(i64::MAX as f64 - 1024.0)) as u64;
        drop(done);
        if self.wanted.swap(key, Ordering::AcqRel) != key
            && let Some(requests) = &self.requests
        {
            let _ = requests.try_send(());
        }
        Ok(())
    }
    fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }
    fn duration(&self) -> f64 {
        self.completion.0.lock().map_or(0.0, |done| done.duration)
    }
    fn ready(&self) -> bool {
        self.completion.0.lock().is_ok_and(|done| done.ready)
    }
    fn failed(&self) -> bool {
        self.completion
            .0
            .lock()
            .is_ok_and(|done| done.error.is_some())
    }
    fn status(&self) -> Option<&str> {
        Some(if self.ready() {
            "DXVA · D3D11 → D3D12 GPU frames"
        } else {
            "Preparing Windows GPU video…"
        })
    }
    fn metrics(&self) -> MediaMetrics {
        MediaMetrics {
            frames_delivered: self.frames,
            cpu_pixel_bytes: 0,
            decoder_starts: u64::from(self.ready()),
            seek_resets: self.completion.0.lock().map_or(0, |done| done.seeks),
            cache_hits: 0,
        }
    }
    fn frame_at(
        &mut self,
        seconds: f64,
        delivery: FrameDelivery,
        queue: &wgpu::Queue,
    ) -> PortResult<bool> {
        self.request_frame(seconds)?;
        let target = self.wanted.load(Ordering::Acquire);
        let mut done = self.completion.0.lock().map_err(|e| e.to_string())?;
        if let Some(error) = &done.error {
            return Err(error.clone());
        }
        if self.delivered == Some(target) {
            return Ok(false);
        }
        if delivery == FrameDelivery::Exact {
            let (result, timeout) = self
                .completion
                .1
                .wait_timeout_while(done, Duration::from_secs(120), |done| {
                    done.error.is_none()
                        && done.frame.as_ref().is_none_or(|frame| frame.key != target)
                })
                .map_err(|e| e.to_string())?;
            done = result;
            if let Some(error) = &done.error {
                return Err(error.clone());
            }
            if timeout.timed_out() {
                return Err("Windows GPU video frame timed out".into());
            }
        }
        let frame = done.frame.take();
        drop(done);
        let Some(frame) = frame else {
            return Ok(false);
        };
        let copied = self.delivered != Some(frame.key);
        if copied {
            frame.copy_to(&self.device, queue, &self.texture)?;
        }
        let key = frame.key;
        self.recycled
            .try_send(Recycled {
                slot: frame.slot,
                produced: frame.value,
                consumed: if copied { frame.value } else { 0 },
            })
            .map_err(|e| e.to_string())?;
        if copied {
            self.delivered = Some(key);
            self.frames += 1;
        }
        Ok(copied)
    }
}
impl Drop for NativeMovie {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.wanted.store(u64::MAX, Ordering::Release);
        self.requests.take();
        if let Some(worker) = self.worker.take() {
            let _ = std::thread::Builder::new()
                .name("vjay-dxva-reaper".into())
                .spawn(move || {
                    let _ = worker.join();
                });
        }
    }
}
