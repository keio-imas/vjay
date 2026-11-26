use super::{
    VideoInfo, acceleration,
    process::{ChildSlot, DecoderProcess},
};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError},
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use vjay_application::{
    media::{FrameDelivery, VideoDecoder, VideoFrame},
    ports::{Extent, PortResult},
};

pub(super) struct FfmpegStream {
    requests: Option<SyncSender<()>>,
    wanted: Arc<AtomicU64>,
    results: Receiver<VideoFrame>,
    recycle: SyncSender<Vec<u8>>,
    worker: Option<JoinHandle<()>>,
    child: ChildSlot,
    stop: Arc<AtomicBool>,
    ready: Arc<AtomicBool>,
    failed: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
    duration: f64,
    fps: f64,
    hardware: bool,
    requested: u64,
    delivered: Option<u64>,
    starts: Arc<AtomicU64>,
    frames_delivered: u64,
    pixel_bytes: u64,
}
impl FfmpegStream {
    pub fn open(
        ffmpeg: PathBuf,
        info: VideoInfo,
        extent: Extent,
        hardware: bool,
    ) -> PortResult<Self> {
        Extent::new(extent.width, extent.height)?;
        let (send_request, requests) = mpsc::sync_channel(1);
        let (results, receive_result) = mpsc::sync_channel(2);
        let (recycle, buffers) = mpsc::sync_channel(3);
        let child = Arc::new(Mutex::new(None));
        let stop = Arc::new(AtomicBool::new(false));
        let ready = Arc::new(AtomicBool::new(false));
        let failed = Arc::new(AtomicBool::new(false));
        let error = Arc::new(Mutex::new(None));
        let wanted = Arc::new(AtomicU64::new(0));
        let starts = Arc::new(AtomicU64::new(0));
        let worker_starts = starts.clone();
        let (duration, fps) = (info.duration, info.fps);
        let (worker_child, worker_stop, worker_ready, worker_failed, worker_error, worker_wanted) = (
            child.clone(),
            stop.clone(),
            ready.clone(),
            failed.clone(),
            error.clone(),
            wanted.clone(),
        );
        let worker = thread::Builder::new()
            .name("vjay-video".into())
            .spawn(move || {
                let run = || -> PortResult<()> {
                    let prepared = acceleration::prepare(
                        &ffmpeg,
                        &info,
                        extent,
                        hardware,
                        &worker_child,
                        &worker_stop,
                    )?;
                    let mut process = DecoderProcess::new(
                        worker_child.clone(),
                        worker_stop.clone(),
                        ffmpeg,
                        prepared,
                        fps,
                        worker_starts,
                    );
                    let size = extent.width as usize * extent.height as usize * 4;
                    let mut spare = None;
                    let mut published = None;
                    while requests.recv().is_ok() {
                        if worker_stop.load(Ordering::Acquire) {
                            break;
                        }
                        while requests.try_recv().is_ok() {}
                        let index = worker_wanted.load(Ordering::Acquire);
                        if published == Some(index) {
                            continue;
                        }
                        let mut rgba = spare
                            .take()
                            .or_else(|| buffers.try_recv().ok())
                            .unwrap_or_else(|| vec![0; size]);
                        process.read(index, &mut rgba)?;
                        let mut frame = VideoFrame {
                            rgba,
                            seconds: index as f64 / fps,
                        };
                        loop {
                            if worker_stop.load(Ordering::Acquire) {
                                return Ok(());
                            }
                            match results.try_send(frame) {
                                Ok(()) => {
                                    published = Some(index);
                                    worker_ready.store(true, Ordering::Release);
                                    break;
                                }
                                Err(mpsc::TrySendError::Full(pending)) => {
                                    frame = pending;
                                    if worker_wanted.load(Ordering::Acquire) != index {
                                        spare = Some(frame.rgba);
                                        break;
                                    }
                                    thread::sleep(Duration::from_millis(2));
                                }
                                Err(mpsc::TrySendError::Disconnected(_)) => return Ok(()),
                            }
                        }
                    }
                    Ok(())
                };
                if let Err(message) = run() {
                    if let Ok(mut error) = worker_error.lock() {
                        *error = Some(message);
                    }
                    worker_failed.store(true, Ordering::Release);
                }
            })
            .map_err(|e| e.to_string())?;
        send_request.send(()).map_err(|e| e.to_string())?;
        Ok(Self {
            requests: Some(send_request),
            wanted,
            results: receive_result,
            recycle,
            worker: Some(worker),
            child,
            stop,
            ready,
            failed,
            error,
            duration,
            fps,
            hardware,
            requested: 0,
            delivered: None,
            starts,
            frames_delivered: 0,
            pixel_bytes: 0,
        })
    }
    fn failure(&self) -> String {
        self.error
            .lock()
            .ok()
            .and_then(|s| s.clone())
            .unwrap_or_else(|| "Video worker disconnected".into())
    }
}
impl VideoDecoder for FfmpegStream {
    fn metrics(&self) -> vjay_application::media::MediaMetrics {
        let starts = self.starts.load(Ordering::Relaxed);
        vjay_application::media::MediaMetrics {
            frames_delivered: self.frames_delivered,
            cpu_pixel_bytes: self.pixel_bytes,
            decoder_starts: starts,
            seek_resets: starts.saturating_sub(1),
            cache_hits: 0,
        }
    }
    fn duration(&self) -> f64 {
        self.duration
    }
    fn failed(&self) -> bool {
        self.failed.load(Ordering::Acquire)
    }
    fn ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }
    fn status(&self) -> Option<&str> {
        if self.failed() {
            None
        } else if !self.ready() {
            Some("Preparing video…")
        } else if self.hardware {
            Some("VideoToolbox · GPU decode / scale")
        } else {
            Some("FFmpeg · CPU decode / upload")
        }
    }
    fn frame_at(
        &mut self,
        seconds: f64,
        delivery: FrameDelivery,
    ) -> PortResult<Option<VideoFrame>> {
        if !seconds.is_finite() {
            return Err("Invalid video playhead".into());
        }
        if self.failed.load(Ordering::Acquire) {
            return Err(self.failure());
        }
        let index = ((seconds.rem_euclid(self.duration) * self.fps + 1e-7).floor() as u64)
            .min((self.duration * self.fps - 1e-6).ceil().max(1.0) as u64 - 1);
        if index != self.requested {
            self.wanted.store(index, Ordering::Release);
            if let Some(requests) = &self.requests {
                let _ = requests.try_send(());
            }
            self.requested = index;
        }
        let mut latest = None;
        loop {
            match self.results.try_recv() {
                Ok(frame) => {
                    if let Some(old) = latest.replace(frame) {
                        self.recycle(old);
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return Err(self.failure()),
            }
        }
        if delivery == FrameDelivery::Exact && self.delivered != Some(index) {
            while latest
                .as_ref()
                .map(|f| (f.seconds * self.fps).round() as u64)
                != Some(index)
            {
                if let Some(old) = latest.take() {
                    self.recycle(old);
                }
                // Exact delivery is reserved for export/tests, never the UI thread.
                latest = Some(
                    self.results
                        .recv_timeout(Duration::from_secs(120))
                        .map_err(|_| self.failure())?,
                );
            }
        }
        if let Some(frame) = &latest {
            self.frames_delivered += 1;
            self.pixel_bytes += frame.rgba.len() as u64;
            self.delivered = Some((frame.seconds * self.fps).round() as u64);
        }
        Ok(latest)
    }
    fn recycle(&mut self, frame: VideoFrame) {
        let _ = self.recycle.try_send(frame.rgba);
    }
}
impl Drop for FfmpegStream {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.requests.take();
        if let Ok(mut slot) = self.child.lock()
            && let Some(child) = slot.as_mut()
        {
            let _ = child.kill();
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
