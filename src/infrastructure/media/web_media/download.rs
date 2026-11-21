use super::cache;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use vjay_application::ports::PortResult;
pub(super) fn terminate(child: &mut Child) {
    if child.try_wait().ok().flatten().is_some() {
        return;
    }
    #[cfg(unix)]
    // SAFETY: this process group was created for this downloader only.
    unsafe {
        libc::kill(-(child.id() as i32), libc::SIGTERM);
    }
    let _ = child.kill();
}
pub(super) fn run(
    executable: &Path,
    ffmpeg: &Path,
    url: &str,
    height: u32,
    root: &Path,
    cancel: &AtomicBool,
    process: &Arc<Mutex<Option<Child>>>,
) -> PortResult<()> {
    let log = root.join("download.log");
    let mut command = Command::new(executable);
    command
        .args([
            "--ignore-config",
            "--no-cache-dir",
            "--no-playlist",
            "--playlist-items",
            "1",
            "--no-progress",
            "--no-warnings",
            "--no-plugin-dirs",
            "--socket-timeout",
            "15",
            "--retries",
            "2",
            "--fragment-retries",
            "2",
            "--max-filesize",
            "2G",
            "--match-filter",
            "!is_live",
            "--merge-output-format",
            "mkv",
            "--remux-video",
            "mkv",
        ])
        .arg("-f")
        .arg(format!(
            "bv*[height<={height}][vcodec^=avc1]+ba/b[height<={height}]/best[height<={height}]"
        ))
        .arg("-o")
        .arg(root.join("source.%(ext)s"))
        .env("DENO_DIR", root.join("deno-cache"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(File::create(&log).map_err(|e| e.to_string())?));
    if ffmpeg.is_absolute() || ffmpeg.components().count() > 1 {
        command.arg("--ffmpeg-location").arg(ffmpeg);
    }
    command.arg("--").arg(url);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut slot = process
        .lock()
        .map_err(|_| "Downloader process is unavailable")?;
    if cancel.load(Ordering::Acquire) {
        return Err("URL import cancelled".into());
    }
    let child = command
        .spawn()
        .map_err(|e| format!("Cannot start yt-dlp: {e}"))?;
    *slot = Some(child);
    drop(slot);
    let start = Instant::now();
    let mut next_budget = Instant::now();
    let result = loop {
        let failure = if cancel.load(Ordering::Acquire) {
            Some("URL import cancelled".to_string())
        } else if start.elapsed() > Duration::from_secs(900) {
            Some("URL import timed out after 15 minutes".into())
        } else if Instant::now() >= next_budget {
            next_budget = Instant::now() + Duration::from_millis(500);
            if cache::staging_size(root) > cache::CLIP_LIMIT * 2 {
                Some("URL import exceeds the file size limit".into())
            } else {
                cache::disk_space(root).err()
            }
        } else {
            None
        };
        let mut guard = process
            .lock()
            .map_err(|_| "Downloader process is unavailable")?;
        let child = guard.as_mut().ok_or("Downloader process is unavailable")?;
        if let Some(error) = failure {
            terminate(child);
            let _ = child.wait();
            break Err(error);
        }
        let status = child.try_wait().map_err(|e| e.to_string())?;
        drop(guard);
        if let Some(status) = status {
            break if status.success() {
                Ok(())
            } else {
                Err(format!("URL video import failed: {}", tail(&log)))
            };
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    process
        .lock()
        .map_err(|_| "Downloader process is unavailable")?
        .take();
    result?;
    let bytes = std::fs::metadata(root.join("source.mkv"))
        .map_err(|e| e.to_string())?
        .len();
    if bytes == 0 || bytes > cache::CLIP_LIMIT {
        return Err("URL video is empty or exceeds 2 GiB".into());
    }
    Ok(())
}
fn tail(path: &Path) -> String {
    let Ok(mut f) = File::open(path) else {
        return "See URL and network availability".into();
    };
    let length = f.metadata().map_or(0, |m| m.len());
    let _ = f.seek(SeekFrom::Start(length.saturating_sub(4096)));
    let mut bytes = Vec::new();
    let _ = f.take(4096).read_to_end(&mut bytes);
    String::from_utf8_lossy(&bytes).trim().to_string()
}
