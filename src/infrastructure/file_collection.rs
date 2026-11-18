use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::SystemTime,
};
use vjay_application::{
    asset_sources::{AssetItem, AssetJob, AssetSourceFactory, ResolvedAsset},
    media::FrameDelivery,
    ports::PortResult,
};
use vjay_domain::execution::AssetRequest;
/// Adapter dispatch is assembled in main; application only knows AssetSourceFactory.
pub struct AssetSourceRegistry(pub Vec<(&'static str, Arc<dyn AssetSourceFactory>)>);
impl AssetSourceFactory for AssetSourceRegistry {
    fn start(&self, r: &AssetRequest) -> PortResult<Box<dyn AssetJob>> {
        self.0
            .iter()
            .find(|(id, _)| *id == r.provider)
            .ok_or_else(|| format!("Unknown asset provider: {}", r.provider))?
            .1
            .start(r)
    }
}
type Catalog = Arc<[AssetItem]>;
type Cache = HashMap<(PathBuf, String), (SystemTime, Catalog)>;
#[derive(Default)]
pub struct FileCollectionFactory {
    cache: Arc<Mutex<Cache>>,
    workers: Arc<AtomicUsize>,
}
struct FileAsset {
    path: PathBuf,
    items: Catalog,
}
impl ResolvedAsset for FileAsset {
    fn path(&self) -> &Path {
        &self.path
    }
    fn items(&self) -> Catalog {
        self.items.clone()
    }
}
struct Permit(Arc<AtomicUsize>);
impl Drop for Permit {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Release);
    }
}
type Reply = PortResult<Box<dyn ResolvedAsset>>;
struct Job(mpsc::Receiver<Reply>);
impl AssetJob for Job {
    fn poll(&mut self, d: FrameDelivery) -> Option<Reply> {
        match d {
            FrameDelivery::Latest => self.0.try_recv().ok(),
            FrameDelivery::Exact => Some(
                self.0
                    .recv()
                    .unwrap_or_else(|_| Err("Collection worker stopped".into())),
            ),
        }
    }
}
impl AssetSourceFactory for FileCollectionFactory {
    fn start(&self, r: &AssetRequest) -> PortResult<Box<dyn AssetJob>> {
        if !["video-collection", "file-collection"].contains(&r.provider) {
            return Err("Unsupported collection provider".into());
        }
        if self.workers.fetch_add(1, Ordering::AcqRel) >= 8 {
            self.workers.fetch_sub(1, Ordering::Release);
            return Err("Too many collection requests; retry after loading finishes".into());
        }
        let permit = Permit(self.workers.clone());
        let path = PathBuf::from(&r.location);
        let index = r
            .options
            .get("index")
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(0);
        let filter = r
            .options
            .get("filter")
            .cloned()
            .unwrap_or_else(|| "video".into());
        let cache = self.cache.clone();
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("video-collection".into())
            .spawn(move || {
                let _permit = permit;
                let result = (|| {
                    let modified = std::fs::metadata(&path)
                        .and_then(|m| m.modified())
                        .map_err(|e| format!("Cannot open collection: {e}"))?;
                    let mut cache = cache.lock().map_err(|_| "Collection cache unavailable")?;
                    let items = if let Some((_, items)) = cache
                        .get(&(path.clone(), filter.clone()))
                        .filter(|(stamp, _)| *stamp == modified)
                    {
                        items.clone()
                    } else {
                        let items = read_collection(&path, &filter)?;
                        if cache.len() >= 16 {
                            cache.clear();
                        }
                        cache.insert((path.clone(), filter.clone()), (modified, items.clone()));
                        items
                    };
                    let item = items.get(index).ok_or_else(|| {
                        format!(
                            "Clip index {index} is outside 0–{}",
                            items.len().saturating_sub(1)
                        )
                    })?;
                    Ok(Box::new(FileAsset {
                        path: item.path.clone(),
                        items,
                    }) as Box<dyn ResolvedAsset>)
                })();
                let _ = tx.send(result);
            })
            .map_err(|e| e.to_string())?;
        Ok(Box::new(Job(rx)))
    }
}
fn video(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        [
            "mp4", "mov", "m4v", "mkv", "webm", "avi", "mpg", "mpeg", "mts", "m2ts", "mxf",
        ]
        .contains(&e.to_ascii_lowercase().as_str())
    })
}
fn supported(path: &Path, filter: &str) -> bool {
    match filter {
        "video" => video(path),
        "image" => path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
            ["png", "jpg", "jpeg", "webp", "gif", "bmp", "tif", "tiff"]
                .contains(&e.to_ascii_lowercase().as_str())
        }),
        _ => true,
    }
}
fn read_collection(path: &Path, filter: &str) -> PortResult<Catalog> {
    let mut paths = Vec::new();
    if path.is_dir() {
        for entry in std::fs::read_dir(path).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let p = entry.path();
            if supported(&p, filter) && p.is_file() {
                paths.push(p);
                if paths.len() > 4096 {
                    return Err("Collection exceeds 4096 clips".into());
                }
            }
        }
        paths.sort_by_cached_key(|p| {
            p.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase()
        });
    } else if supported(path, filter)
        && !path.extension().is_some_and(|e| e == "m3u" || e == "m3u8")
    {
        paths.push(path.to_owned());
    } else {
        let metadata = std::fs::metadata(path).map_err(|e| e.to_string())?;
        if metadata.len() > 4 * 1024 * 1024 {
            return Err("Playlist exceeds 4 MiB".into());
        }
        let text =
            std::fs::read_to_string(path).map_err(|e| format!("Playlist must be UTF-8: {e}"))?;
        for line in text
            .trim_start_matches('\u{feff}')
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
        {
            let p = PathBuf::from(line);
            let p = if p.is_absolute() {
                p
            } else {
                path.parent().unwrap_or(Path::new(".")).join(p)
            };
            if !supported(&p, filter) {
                return Err(format!("Unsupported playlist entry: {line}"));
            }
            paths.push(p);
            if paths.len() > 4096 {
                return Err("Playlist exceeds 4096 clips".into());
            }
        }
    }
    if paths.is_empty() {
        return Err("Collection contains no supported videos".into());
    }
    Ok(paths
        .into_iter()
        .map(|path| AssetItem {
            label: path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            path,
        })
        .collect())
}

#[cfg(test)]
mod tests;
