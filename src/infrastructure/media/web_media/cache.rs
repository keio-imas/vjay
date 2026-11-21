use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
use vjay_application::{asset_sources::ResolvedAsset, ports::PortResult};
pub(super) const CLIP_LIMIT: u64 = 2 * 1024 * 1024 * 1024;
const CACHE_LIMIT: u64 = 8 * 1024 * 1024 * 1024;
pub(super) struct State {
    pub root: PathBuf,
    pub download: Mutex<()>,
    active: Mutex<HashMap<PathBuf, usize>>,
    workers: AtomicUsize,
}
impl State {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            download: Mutex::new(()),
            active: Mutex::new(HashMap::new()),
            workers: AtomicUsize::new(0),
        }
    }
    pub fn worker(self: &Arc<Self>) -> PortResult<Worker> {
        let mut count = self.workers.load(Ordering::Acquire);
        loop {
            if count >= 8 {
                return Err("At most eight URL imports can be pending".into());
            }
            match self.workers.compare_exchange_weak(
                count,
                count + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(actual) => count = actual,
            }
        }
        Ok(Worker(self.clone()))
    }
    pub fn lease(self: &Arc<Self>, path: PathBuf) -> PortResult<Lease> {
        *self
            .active
            .lock()
            .map_err(|_| "URL cache is unavailable")?
            .entry(path.clone())
            .or_default() += 1;
        Ok(Lease {
            path,
            state: self.clone(),
        })
    }
    pub fn reserve(&self) -> PortResult<()> {
        let active = self
            .active
            .lock()
            .map_err(|_| "URL cache is unavailable")?
            .clone();
        let mut entries = Vec::new();
        let mut total = 0;
        for entry in fs::read_dir(&self.root).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let file = entry.path().join("source.mkv");
            if let Ok(meta) = fs::metadata(&file) {
                total += meta.len();
                entries.push((meta.modified().ok(), file, meta.len()));
            }
        }
        entries.sort_by_key(|e| e.0);
        for (_, path, size) in entries {
            if total + CLIP_LIMIT <= CACHE_LIMIT {
                break;
            }
            if !active.contains_key(&path)
                && let Some(parent) = path.parent()
            {
                fs::remove_dir_all(parent).map_err(|e| e.to_string())?;
                total = total.saturating_sub(size);
            }
        }
        if total + CLIP_LIMIT > CACHE_LIMIT {
            return Err("URL cache is full with active clips".into());
        }
        disk_space(&self.root)
    }
}
pub(super) struct Worker(Arc<State>);
impl Drop for Worker {
    fn drop(&mut self) {
        self.0.workers.fetch_sub(1, Ordering::AcqRel);
    }
}
pub(super) fn disk_space(root: &Path) -> PortResult<()> {
    if crate::media::budget::available(root)? < 512 * 1024 * 1024 {
        return Err("URL cache needs at least 512 MiB free disk space".into());
    }
    Ok(())
}
pub(super) fn staging_size(path: &Path) -> u64 {
    fs::read_dir(path)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}
pub(super) struct Lease {
    path: PathBuf,
    state: Arc<State>,
}
impl ResolvedAsset for Lease {
    fn path(&self) -> &Path {
        &self.path
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        if let Ok(mut active) = self.state.active.lock()
            && let Some(n) = active.get_mut(&self.path)
        {
            *n -= 1;
            if *n == 0 {
                active.remove(&self.path);
            }
        }
    }
}
