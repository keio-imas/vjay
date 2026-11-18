use std::{
    path::PathBuf,
    process::Child,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, TryRecvError},
    },
};
use vjay_application::{
    asset_sources::{AssetJob, AssetSourceFactory, ResolvedAsset},
    ports::PortResult,
};
use vjay_domain::execution::AssetRequest;
mod cache;
mod download;
#[derive(Clone)]
pub struct YtdlpAssetFactory {
    state: Arc<cache::State>,
    executable: PathBuf,
    ffmpeg: PathBuf,
}
impl YtdlpAssetFactory {
    pub fn new(executable: PathBuf, ffmpeg: PathBuf) -> Self {
        Self {
            state: Arc::new(cache::State::new(
                std::env::temp_dir().join("vjay-web-video-v1"),
            )),
            executable,
            ffmpeg,
        }
    }
    pub fn with_cache(mut self, path: PathBuf) -> Self {
        self.state = Arc::new(cache::State::new(path));
        self
    }
}
type ResultAsset = PortResult<Box<dyn ResolvedAsset>>;
struct Job {
    receiver: Receiver<ResultAsset>,
    cancel: Arc<AtomicBool>,
    child: Arc<Mutex<Option<Child>>>,
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
        if let Ok(mut child) = self.child.lock()
            && let Some(child) = child.as_mut()
        {
            download::terminate(child);
        }
    }
}
impl AssetJob for Job {
    fn poll(&mut self, delivery: vjay_application::media::FrameDelivery) -> Option<ResultAsset> {
        if delivery == vjay_application::media::FrameDelivery::Exact {
            return Some(
                self.receiver
                    .recv()
                    .unwrap_or_else(|_| Err("URL video worker stopped".into())),
            );
        }
        match self.receiver.try_recv() {
            Ok(v) => Some(v),
            Err(TryRecvError::Empty) => None,
            Err(_) => Some(Err("URL video worker stopped".into())),
        }
    }
}
impl AssetSourceFactory for YtdlpAssetFactory {
    fn start(&self, request: &AssetRequest) -> PortResult<Box<dyn AssetJob>> {
        if request.provider != "web-video" {
            return Err("Unknown URL media provider".into());
        }
        let url =
            url::Url::parse(&request.location).map_err(|_| "Enter a valid http(s) video URL")?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err("Use an http(s) URL without embedded credentials".into());
        }
        let height = request
            .options
            .get("max_height")
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(1080);
        if !(360..=2160).contains(&height) {
            return Err("Maximum height must be 360–2160".into());
        }
        let (sender, receiver) = mpsc::sync_channel(1);
        let permit = self.state.worker()?;
        let cancel = Arc::new(AtomicBool::new(false));
        let child = Arc::new(Mutex::new(None));
        let factory = self.clone();
        let request = request.clone();
        let flag = cancel.clone();
        let process = child.clone();
        std::thread::Builder::new()
            .name("vjay-url-video".into())
            .spawn(move || {
                let _permit = permit;
                let result = factory.resolve(&request, height, &flag, &process);
                let _ = sender.send(result);
            })
            .map_err(|e| e.to_string())?;
        Ok(Box::new(Job {
            receiver,
            cancel,
            child,
        }))
    }
}
impl YtdlpAssetFactory {
    fn resolve(
        &self,
        request: &AssetRequest,
        height: u32,
        cancel: &AtomicBool,
        child: &Arc<Mutex<Option<Child>>>,
    ) -> ResultAsset {
        use std::{
            hash::{Hash, Hasher},
            time::Duration,
        };
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        request.hash(&mut hasher);
        let path = self.state.root.join(format!("{:016x}", hasher.finish()));
        let guard = loop {
            if cancel.load(Ordering::Acquire) {
                return Err("URL import cancelled".into());
            }
            match self.state.download.try_lock() {
                Ok(lock) => break lock,
                Err(std::sync::TryLockError::WouldBlock) => {
                    std::thread::sleep(Duration::from_millis(50))
                }
                Err(_) => return Err("URL cache is unavailable".into()),
            }
        };
        std::fs::create_dir_all(&self.state.root).map_err(|e| e.to_string())?;
        let file = path.join("source.mkv");
        if !file.is_file() {
            self.state.reserve()?;
            let staging = tempfile::Builder::new()
                .prefix("download-")
                .tempdir_in(&self.state.root)
                .map_err(|e| e.to_string())?;
            download::run(
                &self.executable,
                &self.ffmpeg,
                &request.location,
                height,
                staging.path(),
                cancel,
                child,
            )?;
            if !staging.path().join("source.mkv").is_file() {
                return Err("The URL did not produce a playable video file".into());
            }
            let _ = std::fs::remove_file(staging.path().join("download.log"));
            let _ = std::fs::remove_dir_all(staging.path().join("deno-cache"));
            std::fs::rename(staging.path(), &path).map_err(|e| e.to_string())?;
        }
        let asset = self.state.lease(file)?;
        drop(guard);
        Ok(Box::new(asset))
    }
}
