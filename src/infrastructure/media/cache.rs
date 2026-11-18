use super::{VideoInfo, budget, process::ChildSlot};
use std::{
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use vjay_application::ports::PortResult;

static BUILD: Mutex<()> = Mutex::new(());
static USED: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
pub(super) struct CacheFile {
    pub path: PathBuf,
}
impl CacheFile {
    fn lease(path: PathBuf) -> PortResult<Self> {
        USED.lock().map_err(|e| e.to_string())?.push(path.clone());
        Ok(Self { path })
    }
}
impl Drop for CacheFile {
    fn drop(&mut self) {
        if let Ok(mut used) = USED.lock()
            && let Some(i) = used.iter().position(|p| *p == self.path)
        {
            used.swap_remove(i);
        }
    }
}

/// Only app-owned files in this versioned directory are written or removed.
pub(super) fn prepare(
    ffmpeg: &Path,
    info: &VideoInfo,
    slot: &ChildSlot,
    stop: &Arc<AtomicBool>,
) -> PortResult<CacheFile> {
    let _guard = loop {
        if stop.load(Ordering::Acquire) {
            return Err("Video preparation cancelled".into());
        }
        match BUILD.try_lock() {
            Ok(guard) => break guard,
            Err(std::sync::TryLockError::Poisoned(_)) => {
                return Err("Video cache lock failed".into());
            }
            Err(std::sync::TryLockError::WouldBlock) => (),
        }
        std::thread::sleep(Duration::from_millis(30));
    };
    let root = std::env::temp_dir().join("vjay-video-cache-v1");
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    }
    let metadata = fs::metadata(&info.path).map_err(|e| e.to_string())?;
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    info.path.hash(&mut hash);
    metadata.len().hash(&mut hash);
    metadata.modified().ok().hash(&mut hash);
    let key = hash.finish();
    let cached = root.join(format!("{key:016x}.mov"));
    if cached.is_file() {
        return CacheFile::lease(cached);
    }
    trim(&root);
    budget::check(&root, None)?;
    let partial = root.join(format!("{key:016x}-{}.partial.mov", std::process::id()));
    let errors = root.join(format!("{key:016x}-{}.log", std::process::id()));
    let log = fs::File::create(&errors).map_err(|e| e.to_string())?;
    let child = Command::new(ffmpeg)
        .args([
            "-hide_banner",
            "-v",
            "error",
            "-nostdin",
            "-threads",
            "8",
            "-i",
        ])
        .arg(&info.path)
        .args([
            "-map",
            "0:v:0",
            "-an",
            "-sn",
            "-dn",
            "-c:v",
            "prores_videotoolbox",
            "-profile:v",
            "3",
            "-allow_sw",
            "0",
            "-pix_fmt",
            "p210le",
            "-y",
        ])
        .arg(&partial)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(log)
        .spawn()
        .map_err(|e| e.to_string())?;
    *slot.lock().map_err(|e| e.to_string())? = Some(child);
    let mut budget_error = None;
    let mut last_check = std::time::Instant::now();
    let result = loop {
        if last_check.elapsed() > Duration::from_secs(1) {
            budget_error = budget::check(&root, Some(&partial)).err();
            last_check = std::time::Instant::now();
        }
        let mut locked = slot.lock().map_err(|e| e.to_string())?;
        let child = locked.as_mut().ok_or("Cache encoder unavailable")?;
        if stop.load(Ordering::Acquire) || budget_error.is_some() {
            let _ = child.kill();
        }
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            locked.take();
            break status;
        }
        drop(locked);
        std::thread::sleep(Duration::from_millis(30));
    };
    let diagnostic = fs::read_to_string(&errors).unwrap_or_default();
    let _ = fs::remove_file(errors);
    if !result.success() {
        let _ = fs::remove_file(partial);
        if let Some(error) = budget_error {
            return Err(error);
        }
        return Err(format!(
            "GPU playback cache failed: {}",
            diagnostic
                .chars()
                .rev()
                .take(2048)
                .collect::<String>()
                .chars()
                .rev()
                .collect::<String>()
                .trim()
        ));
    }
    fs::rename(&partial, &cached).map_err(|e| e.to_string())?;
    let lease = CacheFile::lease(cached)?;
    trim(&root);
    Ok(lease)
}
fn trim(root: &Path) {
    let mut files: Vec<_> = fs::read_dir(root)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|s| s == "mov"))
        .filter_map(|e| {
            Some((
                e.metadata().ok()?.modified().ok()?,
                e.metadata().ok()?.len(),
                e.path(),
            ))
        })
        .collect();
    let mut total: u64 = files.iter().map(|f| f.1).sum();
    if let Ok(used) = USED.lock() {
        files.retain(|f| !used.contains(&f.2));
    }
    files.sort_by_key(|f| f.0);
    for (_, size, path) in files {
        if total < 6 * 1024 * 1024 * 1024 {
            break;
        }
        if fs::remove_file(path).is_ok() {
            total = total.saturating_sub(size);
        }
    }
}
