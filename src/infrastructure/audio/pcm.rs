use super::decoder::{ChildSlot, Shared};
use std::{
    io::Read,
    path::{Path, PathBuf},
    process::{ChildStdout, Command, Stdio},
    sync::{Arc, atomic::Ordering},
};
use vjay_application::ports::PortResult;
mod cache;

pub(super) const FRAMES: usize = 4096;
pub(super) type Buffer = Box<[[f32; 2]]>;

pub(super) struct PcmReader {
    ffmpeg: PathBuf,
    path: PathBuf,
    rate: u32,
    duration: f64,
    total_frames: u64,
    child: ChildSlot,
    state: Arc<Shared>,
    stdout: Option<ChildStdout>,
    next_frame: Option<u64>,
    cache: cache::Cache,
    scratch: Buffer,
}
impl PcmReader {
    pub fn duration(ffprobe: &Path, path: &Path) -> PortResult<f64> {
        let probe = Command::new(ffprobe)
            .args([
                "-v",
                "error",
                "-protocol_whitelist",
                "file,pipe",
                "-show_entries",
                "format=duration:stream=codec_type",
                "-of",
                "json",
            ])
            .arg(path)
            .output()
            .map_err(|e| e.to_string())?;
        if !probe.status.success() {
            return Err(format!(
                "Audio probe: {}",
                String::from_utf8_lossy(&probe.stderr).trim()
            ));
        }
        let info: serde_json::Value =
            serde_json::from_slice(&probe.stdout).map_err(|e| e.to_string())?;
        if !info["streams"]
            .as_array()
            .is_some_and(|streams| streams.iter().any(|s| s["codec_type"] == "audio"))
        {
            return Err(format!("{} has no audio track", path.display()));
        }
        info["format"]["duration"]
            .as_str()
            .and_then(|s| s.parse::<f64>().ok())
            .filter(|v| v.is_finite() && *v > 0.0)
            .ok_or_else(|| "Invalid audio duration".into())
    }
    pub fn new(
        ffmpeg: PathBuf,
        path: PathBuf,
        rate: u32,
        duration: f64,
        child: ChildSlot,
        state: Arc<Shared>,
    ) -> Self {
        Self {
            ffmpeg,
            path,
            rate,
            duration,
            total_frames: (duration * rate as f64).round().max(1.0) as u64,
            child,
            state,
            stdout: None,
            next_frame: None,
            cache: Default::default(),
            scratch: vec![[0.0; 2]; FRAMES].into_boxed_slice(),
        }
    }
    pub fn read_range(&mut self, seconds: f64, output: &mut [[f32; 2]]) -> PortResult<()> {
        let start = f64::from_bits(self.state.range_start.load(Ordering::Acquire));
        let end = f64::from_bits(self.state.range_end.load(Ordering::Acquire)).min(self.duration);
        let looping = self.state.looping.load(Ordering::Acquire);
        let length = (end - start).max(1.0 / self.rate as f64);
        let mut position = seconds;
        let mut written = 0;
        while written < output.len() {
            if looping {
                position = start + (position - start).rem_euclid(length);
            } else if position >= end {
                output[written..].fill([0.0; 2]);
                break;
            }
            let count = (((end - position) * self.rate as f64).ceil().max(1.0) as usize)
                .min(output.len() - written);
            self.read_window(position, &mut output[written..written + count])?;
            written += count;
            position += count as f64 / self.rate as f64;
        }
        Ok(())
    }
    pub fn read_window(&mut self, seconds: f64, output: &mut [[f32; 2]]) -> PortResult<()> {
        let mut position = ((seconds.rem_euclid(self.duration) * self.rate as f64) as u64)
            .min(self.total_frames - 1);
        let mut written = 0;
        while written < output.len() {
            let key = position / FRAMES as u64;
            let offset = position as usize % FRAMES;
            self.ensure_page(key)?;
            let page = self.cache.get(key).ok_or("Missing decoded PCM page")?;
            let count = (page.len() - offset).min(output.len() - written);
            output[written..written + count].copy_from_slice(&page[offset..offset + count]);
            position = (position + count as u64) % self.total_frames;
            written += count;
        }
        Ok(())
    }
    fn ensure_page(&mut self, key: u64) -> PortResult<()> {
        if self.cache.contains(key) {
            return Ok(());
        }
        let start = key * FRAMES as u64;
        let count = (self.total_frames - start).min(FRAMES as u64) as usize;
        if self.next_frame != Some(start) {
            self.start(start)?;
        }
        self.stdout
            .as_mut()
            .ok_or("Audio decoder not started")?
            .read_exact(bytemuck::cast_slice_mut(&mut self.scratch[..count]))
            .map_err(|e| format!("Audio decoder: {e}"))?;
        for frame in &mut self.scratch[..count] {
            for sample in frame {
                let value = f32::from_le_bytes(sample.to_ne_bytes());
                *sample = if value.is_finite() { value } else { 0.0 };
            }
        }
        // FFmpeg's input seek may also affect stream_loop's restart point.
        // Reopen at zero on a cache miss across the clip boundary instead of
        // treating the tail's next bytes as the beginning of the file.
        self.next_frame =
            (start + (count as u64) < self.total_frames).then_some(start + count as u64);
        self.cache.insert(key, &self.scratch[..count]);
        Ok(())
    }
    fn start(&mut self, frame: u64) -> PortResult<()> {
        let mut slot = self.child.lock().map_err(|e| e.to_string())?;
        if self.state.stop.load(Ordering::Acquire) {
            return Err("Audio decoder stopped".into());
        }
        if let Some(mut old) = slot.take() {
            let _ = old.kill();
            let _ = old.wait();
        }
        self.stdout = None;
        let seconds = frame as f64 / self.rate as f64;
        let mut child = Command::new(&self.ffmpeg)
            .args([
                "-v",
                "error",
                "-nostdin",
                "-threads",
                "1",
                "-filter_threads",
                "1",
                "-stream_loop",
                "-1",
                "-protocol_whitelist",
                "file,pipe",
                "-ss",
                &format!("{seconds:.9}"),
                "-i",
            ])
            .arg(&self.path)
            .args([
                "-map",
                "0:a:0",
                "-vn",
                "-sn",
                "-dn",
                "-ac",
                "2",
                "-ar",
                &self.rate.to_string(),
                "-f",
                "f32le",
                "pipe:1",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Audio decoder: {e}"))?;
        self.stdout = child.stdout.take();
        *slot = Some(child);
        self.state.decoder_starts.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}

impl Drop for PcmReader {
    fn drop(&mut self) {
        if let Ok(mut slot) = self.child.lock()
            && let Some(mut child) = slot.take()
        {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
