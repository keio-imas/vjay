use super::pcm::{Buffer, FRAMES, PcmReader};
use std::{
    path::Path,
    process::Child,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use vjay_application::ports::PortResult;

pub(super) type ChildSlot = Arc<Mutex<Option<Child>>>;
pub(super) struct Chunk {
    pub epoch: u64,
    pub start: f64,
    pub frames: Buffer,
}
pub(super) struct Playback {
    pub chunks: rtrb::Consumer<Chunk>,
    pub recycle: rtrb::Producer<Buffer>,
}
#[repr(align(64))]
pub(super) struct Shared {
    pub range_start: AtomicU64,
    pub range_end: AtomicU64,
    pub looping: AtomicBool,
    pub gain: AtomicU32,
    pub rate: AtomicU32,
    pub epoch: AtomicU64,
    pub seek: AtomicU64,
    pub active_epoch: AtomicU64,
    pub playhead: AtomicU64,
    pub consumed: AtomicU64,
    pub decoder_starts: AtomicU64,
    pub stop: AtomicBool,
    pub error: Mutex<Option<String>>,
}
impl Default for Shared {
    fn default() -> Self {
        Self {
            range_start: AtomicU64::new(0.0f64.to_bits()),
            range_end: AtomicU64::new(f64::INFINITY.to_bits()),
            looping: AtomicBool::new(true),
            gain: AtomicU32::new(0.0f32.to_bits()),
            rate: AtomicU32::new(1.0f32.to_bits()),
            epoch: AtomicU64::new(1),
            seek: AtomicU64::new(0.0f64.to_bits()),
            active_epoch: AtomicU64::new(0),
            playhead: AtomicU64::new(f64::NAN.to_bits()),
            consumed: AtomicU64::new(0),
            decoder_starts: AtomicU64::new(0),
            stop: AtomicBool::new(false),
            error: Mutex::new(None),
        }
    }
}
pub(super) struct AudioWorker {
    pub shared: Arc<Shared>,
    child: ChildSlot,
    worker: Option<JoinHandle<()>>,
    pub duration: f64,
    pub sample_rate: u32,
}
impl AudioWorker {
    pub fn open(
        ffmpeg: &Path,
        ffprobe: &Path,
        path: &Path,
        sample_rate: u32,
    ) -> PortResult<(Self, Playback)> {
        if sample_rate == 0 {
            return Err("Invalid audio sample rate".into());
        }
        let path = path.canonicalize().map_err(|e| e.to_string())?;
        let duration = PcmReader::duration(ffprobe, &path)?;
        let shared = Arc::new(Shared::default());
        let child = Arc::new(Mutex::new(None));
        let (mut pending, chunks) = rtrb::RingBuffer::new(2);
        let (mut recycle, mut available) = rtrb::RingBuffer::new(4);
        // Four buffers cover the callback, decoder and two queued chunks. Recycling
        // capacity is the entire pool, so returning a buffer never frees memory in RT.
        for _ in 0..4 {
            let _ = recycle.push(vec![[0.0; 2]; FRAMES].into_boxed_slice());
        }
        let state = shared.clone();
        let slot = child.clone();
        let ffmpeg = ffmpeg.to_path_buf();
        let worker = thread::Builder::new()
            .name("vjay-audio-decode".into())
            .spawn(move || {
                let mut reader = PcmReader::new(
                    ffmpeg,
                    path,
                    sample_rate,
                    duration,
                    slot.clone(),
                    state.clone(),
                );
                let mut epoch = 0;
                let mut cursor = 0.0;
                'decode: while !state.stop.load(Ordering::Acquire) {
                    let mut frames = match available.pop() {
                        Ok(buffer) => buffer,
                        Err(_) => {
                            thread::sleep(Duration::from_millis(1));
                            continue;
                        }
                    };
                    let requested = state.epoch.load(Ordering::Acquire);
                    if requested != epoch {
                        cursor = f64::from_bits(state.seek.load(Ordering::Acquire));
                        epoch = requested;
                    }
                    // Complete this window even if a newer request arrives. Publishing
                    // complete PCM before switching generations prevents seek starvation.
                    if let Err(error) = reader.read_range(cursor, &mut frames) {
                        if !state.stop.load(Ordering::Acquire)
                            && let Ok(mut target) = state.error.lock()
                        {
                            *target = Some(error);
                        }
                        break;
                    }
                    let mut chunk = Chunk {
                        epoch,
                        start: cursor,
                        frames,
                    };
                    cursor += FRAMES as f64 / sample_rate as f64;
                    let start = f64::from_bits(state.range_start.load(Ordering::Acquire));
                    let end = f64::from_bits(state.range_end.load(Ordering::Acquire)).min(duration);
                    if state.looping.load(Ordering::Acquire) {
                        cursor = start
                            + (cursor - start)
                                .rem_euclid((end - start).max(1.0 / sample_rate as f64));
                    }
                    loop {
                        if state.stop.load(Ordering::Acquire) {
                            break 'decode;
                        }
                        match pending.push(chunk) {
                            Ok(()) => break,
                            Err(rtrb::PushError::Full(value)) => {
                                chunk = value;
                                thread::sleep(Duration::from_millis(1));
                            }
                        }
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        Ok((
            Self {
                shared,
                child,
                worker: Some(worker),
                duration,
                sample_rate,
            },
            Playback { chunks, recycle },
        ))
    }
    pub fn seek(&self, seconds: f64) {
        if seconds.is_finite() {
            self.shared.seek.store(seconds.to_bits(), Ordering::Release);
            self.shared.epoch.fetch_add(1, Ordering::AcqRel);
        }
    }
}
impl Drop for AudioWorker {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
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
