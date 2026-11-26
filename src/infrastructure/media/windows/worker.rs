use super::{
    device::{Apartment, Device},
    processor::Processor,
    reader::Reader,
    transfer::{Published, Recycled, Slot},
};
use std::{
    path::PathBuf,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{Receiver, RecvTimeoutError},
    },
    time::Duration,
};
use vjay_application::ports::{Extent, PortResult};

#[derive(Default)]
pub(super) struct Completion {
    pub frame: Option<Published>,
    pub duration: f64,
    pub fps: f64,
    pub seeks: u64,
    pub ready: bool,
    pub error: Option<String>,
}
pub(super) type Mailbox = Arc<(Mutex<Completion>, Condvar)>;

pub(super) struct Worker {
    pub device: wgpu::Device,
    pub path: PathBuf,
    pub extent: Extent,
    pub wanted: Arc<AtomicU64>,
    pub stop: Arc<AtomicBool>,
    pub completion: Mailbox,
    pub requests: Receiver<()>,
    pub recycled: Receiver<Recycled>,
}
impl Worker {
    pub fn run(self) {
        if let Err(error) = self.decode() {
            if let Ok(mut done) = self.completion.0.lock() {
                done.error = Some(error);
            }
            self.completion.1.notify_all();
        }
    }

    fn decode(&self) -> PortResult<()> {
        let _apartment =
            Apartment::new().map_err(|e| format!("Media Foundation startup failed: {e}"))?;
        let native = Device::new(&self.device)
            .map_err(|e| format!("Cannot create GPU video device: {e}"))?;
        let mut reader = Reader::open(&native, &self.path)?;
        if !reader.duration.is_finite() || reader.duration <= 0.0 {
            return Err("Video has no finite duration".into());
        }
        {
            let mut done = self.completion.0.lock().map_err(|e| e.to_string())?;
            done.duration = reader.duration;
            done.fps = reader.fps;
        }
        // The first read may negotiate aligned decoder dimensions and display
        // apertures. Build the persistent processor only after that negotiation.
        while !self.stop.load(Ordering::Acquire) {
            let key = self.wanted.load(Ordering::Acquire);
            if reader.at(key, &self.wanted)?.is_some() {
                break;
            }
        }
        if self.stop.load(Ordering::Acquire) {
            return Ok(());
        }
        let slots = (0..3)
            .map(|_| Slot::new(&native, &self.device, self.extent))
            .collect::<windows::core::Result<Vec<_>>>()
            .map_err(|e| format!("Cannot share GPU video textures: {e}"))?;
        let mut processor = Processor::new(&native, &reader.media_type, self.extent, &slots)
            .map_err(|e| format!("GPU video conversion is unavailable: {e}"))?;
        let mut free = slots
            .into_iter()
            .map(|slot| Recycled {
                slot,
                consumed: 0,
                produced: 0,
            })
            .collect::<Vec<_>>();
        let (mut published, mut value) = (None, 0u64);
        while !self.stop.load(Ordering::Acquire) {
            match self.requests.recv_timeout(Duration::from_millis(50)) {
                Ok(()) => (),
                Err(RecvTimeoutError::Timeout) => {
                    processor.reap();
                    continue;
                }
                Err(RecvTimeoutError::Disconnected) => break,
            }
            while self.requests.try_recv().is_ok() {}
            let key = self.wanted.load(Ordering::Acquire);
            if key == u64::MAX || published == Some(key) {
                continue;
            }
            // Source Reader can block waiting for its fixed decoder surface
            // pool. Release completed samples before entering that blocking API;
            // otherwise retaining them for GPU conversion could exhaust the pool.
            processor.finish_reads().map_err(|e| e.to_string())?;
            let Some(sample) = reader.at(key, &self.wanted)? else {
                continue;
            };
            while let Ok(slot) = self.recycled.try_recv() {
                free.push(slot);
            }
            // Replacing an unconsumed mailbox frame recycles it without a DX12
            // wait. A consumed slot returns only after its copy's fence was queued.
            if let Some(old) = self
                .completion
                .0
                .lock()
                .map_err(|e| e.to_string())?
                .frame
                .take()
            {
                free.push(Recycled {
                    slot: old.slot,
                    produced: old.value,
                    consumed: 0,
                });
            }
            let waiting = std::time::Instant::now();
            let recycled = loop {
                processor.reap();
                let available = free.iter().position(|slot| unsafe {
                    slot.slot.consumer.d11.GetCompletedValue() >= slot.consumed
                        && slot.slot.producer.d11.GetCompletedValue() >= slot.produced
                });
                if let Some(index) = available {
                    break free.swap_remove(index);
                }
                if self.stop.load(Ordering::Acquire) {
                    return Ok(());
                }
                if waiting.elapsed() > Duration::from_secs(5) {
                    return Err(
                        "GPU video transfer timed out waiting for a reusable surface".into(),
                    );
                }
                // Only the worker waits. Bounding actual GPU work prevents seek
                // storms from filling either command queue or decoder surface pool.
                if let Ok(slot) = self.recycled.recv_timeout(Duration::from_millis(2)) {
                    free.push(slot);
                }
            };
            value = value.checked_add(1).ok_or("GPU video fence overflow")?;
            processor
                .blit(&native, &sample, &recycled, value)
                .map_err(|e| format!("GPU video processing failed: {e}"))?;
            let mut done = self.completion.0.lock().map_err(|e| e.to_string())?;
            done.frame = Some(Published {
                slot: recycled.slot,
                key,
                value,
            });
            done.seeks = reader.seeks;
            done.ready = true;
            drop(done);
            self.completion.1.notify_all();
            published = Some(key);
        }
        Ok(())
    }
}
