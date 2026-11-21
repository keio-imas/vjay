use super::cache::{Pixels, RasterCache};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc::{self, Receiver, SyncSender, TrySendError},
};
use vjay_application::{
    media::{ImageLoader, PreparedImageFactory},
    ports::*,
};
use vjay_domain::execution::PreparedImage;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Source {
    File {
        path: std::path::PathBuf,
        asset: vjay_domain::node::AssetId,
    },
    Prepared(PreparedImage),
}
pub(super) struct Job {
    pub ticket: u64,
    pub current: Arc<AtomicU64>,
    pub source: Arc<Source>,
    pub extent: Extent,
}
pub(super) struct Completed {
    pub ticket: u64,
    pub source: Arc<Source>,
    pub pixels: PortResult<Pixels>,
}
pub(super) struct Pool {
    requests: Option<SyncSender<Job>>,
    results: Option<Receiver<Completed>>,
    stop: Arc<AtomicBool>,
    workers: Vec<std::thread::JoinHandle<()>>,
}
impl Pool {
    pub fn new(
        files: Option<Arc<dyn ImageLoader>>,
        generated: Option<Arc<dyn PreparedImageFactory>>,
    ) -> PortResult<Self> {
        // Only two results and two active rasterizations can retain CPU images.
        let (send, receive) = mpsc::sync_channel::<Job>(16);
        let (results, completed) = mpsc::sync_channel(2);
        let receive = Arc::new(Mutex::new(receive));
        let stop = Arc::new(AtomicBool::new(false));
        let mut workers = Vec::new();
        let cache = Arc::new(Mutex::new(RasterCache::default()));
        for index in 0..2 {
            let (receive, results, stop, files, generated) = (
                receive.clone(),
                results.clone(),
                stop.clone(),
                files.clone(),
                generated.clone(),
            );
            let cache = cache.clone();
            workers.push(
                std::thread::Builder::new()
                    .name(format!("vjay-image-{index}"))
                    .spawn(move || {
                        while !stop.load(Ordering::Acquire) {
                            let job = match receive.lock().ok().and_then(|r| r.recv().ok()) {
                                Some(job) => job,
                                None => break,
                            };
                            if job.current.load(Ordering::Acquire) != job.ticket {
                                continue;
                            }
                            let cached = cache
                                .lock()
                                .ok()
                                .and_then(|mut c| c.get(&job.source, job.extent));
                            let pixels = if let Some(cached) = cached {
                                Ok(cached)
                            } else {
                                match job.source.as_ref() {
                                    Source::File { path, .. } => files
                                        .as_ref()
                                        .ok_or_else(|| "Image loader adapter is unavailable".into())
                                        .and_then(|f| f.load(path, job.extent)),
                                    Source::Prepared(source) => generated
                                        .as_ref()
                                        .ok_or_else(|| {
                                            "Generated image adapter is unavailable".into()
                                        })
                                        .and_then(|f| f.load(source, job.extent)),
                                }
                                .and_then(|pixels| {
                                    if pixels.len()
                                        == (job.extent.width * job.extent.height * 4) as usize
                                    {
                                        Ok(Arc::new(pixels))
                                    } else {
                                        Err("Image provider returned an invalid size".into())
                                    }
                                })
                            };
                            if let Ok(pixels) = &pixels
                                && let Ok(mut cache) = cache.lock()
                            {
                                cache.insert(job.source.clone(), job.extent, pixels.clone());
                            }
                            if job.current.load(Ordering::Acquire) != job.ticket
                                || stop.load(Ordering::Acquire)
                            {
                                continue;
                            }
                            if results
                                .send(Completed {
                                    ticket: job.ticket,
                                    source: job.source,
                                    pixels,
                                })
                                .is_err()
                            {
                                break;
                            }
                        }
                    })
                    .map_err(|e| e.to_string())?,
            );
        }
        Ok(Self {
            requests: Some(send),
            results: Some(completed),
            stop,
            workers,
        })
    }
    pub fn submit(&self, job: Job) -> PortResult<Option<Job>> {
        match self
            .requests
            .as_ref()
            .ok_or("Image workers stopped")?
            .try_send(job)
        {
            Ok(()) => Ok(None),
            Err(TrySendError::Full(job)) => Ok(Some(job)),
            Err(TrySendError::Disconnected(_)) => Err("Image workers stopped".into()),
        }
    }
    pub fn poll(&self) -> PortResult<Option<Completed>> {
        match self
            .results
            .as_ref()
            .ok_or("Image workers stopped")?
            .try_recv()
        {
            Ok(value) => Ok(Some(value)),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => Err("Image workers stopped".into()),
        }
    }
    pub fn wait(&self) -> PortResult<Option<Completed>> {
        match self
            .results
            .as_ref()
            .ok_or("Image workers stopped")?
            .recv_timeout(std::time::Duration::from_millis(2))
        {
            Ok(done) => Ok(Some(done)),
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err("Image workers stopped".into()),
        }
    }
}
impl Drop for Pool {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.requests.take();
        self.results.take(); // Release workers blocked on the bounded result queue.
        let workers = std::mem::take(&mut self.workers);
        let _ = std::thread::Builder::new()
            .name("vjay-image-reaper".into())
            .spawn(move || {
                for worker in workers {
                    let _ = worker.join();
                }
            });
    }
}

#[cfg(test)]
mod tests;
