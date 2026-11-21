use super::acceleration::PreparedVideo;
use std::{
    io::Read,
    path::PathBuf,
    process::{Child, ChildStdout, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread::JoinHandle,
};
use vjay_application::ports::PortResult;

pub(super) type ChildSlot = Arc<Mutex<Option<Child>>>;
pub(super) struct DecoderProcess {
    pub slot: ChildSlot,
    pub stop: Arc<AtomicBool>,
    ffmpeg: PathBuf,
    prepared: PreparedVideo,
    fps: f64,
    stdout: Option<ChildStdout>,
    next: u64,
    start: u64,
    diagnostics: Arc<Mutex<Vec<u8>>>,
    reader: Option<JoinHandle<()>>,
    starts: Arc<AtomicU64>,
}
impl DecoderProcess {
    pub fn new(
        slot: ChildSlot,
        stop: Arc<AtomicBool>,
        ffmpeg: PathBuf,
        prepared: PreparedVideo,
        fps: f64,
        starts: Arc<AtomicU64>,
    ) -> Self {
        Self {
            slot,
            stop,
            ffmpeg,
            prepared,
            fps,
            stdout: None,
            next: 0,
            start: 0,
            diagnostics: Arc::new(Mutex::new(Vec::new())),
            reader: None,
            starts,
        }
    }
    fn close(&mut self) {
        if let Ok(mut slot) = self.slot.lock()
            && let Some(mut child) = slot.take()
        {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.stdout.take();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
    fn restart(&mut self, index: u64) -> PortResult<()> {
        self.close();
        if self.stop.load(Ordering::Acquire) {
            return Err("Video decoder cancelled".into());
        }
        let mut command = Command::new(&self.ffmpeg);
        command.args(["-v", "error", "-nostdin"]);
        if self.prepared.hardware {
            command.args([
                "-hwaccel",
                "videotoolbox",
                "-hwaccel_output_format",
                "videotoolbox_vld",
            ]);
        }
        command
            .args([
                "-threads",
                "4",
                "-stream_loop",
                "-1",
                "-protocol_whitelist",
                "file,pipe",
                "-ss",
                &format!("{:.12}", (index as f64 / self.fps - 0.0000001).max(0.0)),
                "-i",
            ])
            .arg(&self.prepared.path)
            .args([
                "-map",
                "0:v:0",
                "-an",
                "-sn",
                "-dn",
                "-vf",
                &self.prepared.filter,
                "-threads",
                "2",
                "-pix_fmt",
                "rgba",
                "-f",
                "rawvideo",
                "pipe:1",
            ]);
        let mut child = command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Cannot run FFmpeg: {e}"))?;
        self.starts.fetch_add(1, Ordering::Relaxed);
        self.stdout = child.stdout.take();
        let mut stderr = child.stderr.take().ok_or("Missing decoder diagnostics")?;
        self.diagnostics.lock().map_err(|e| e.to_string())?.clear();
        let diagnostics = self.diagnostics.clone();
        self.reader = Some(std::thread::spawn(move || {
            let mut bytes = [0; 1024];
            while let Ok(count) = stderr.read(&mut bytes) {
                if count == 0 {
                    break;
                }
                if let Ok(mut tail) = diagnostics.lock() {
                    let discard = (tail.len() + count).saturating_sub(8192);
                    tail.drain(..discard);
                    tail.extend_from_slice(&bytes[..count]);
                }
            }
        }));
        *self.slot.lock().map_err(|e| e.to_string())? = Some(child);
        self.next = index;
        self.start = index;
        Ok(())
    }
    pub fn read(&mut self, index: u64, rgba: &mut [u8]) -> PortResult<()> {
        // A full-source sequential stream stays open across its natural loop.
        // Explicit backwards seeks still restart, preserving exact seek contents.
        if self.stdout.is_some()
            && self.start == 0
            && index < self.next
            && self.next >= self.prepared.frames.saturating_sub(2)
            && index < 2
        {
            while self.next < self.prepared.frames {
                self.read_next(rgba)?;
            }
            self.next = 0;
        }
        if self.stdout.is_none()
            || index < self.next
            || index.saturating_sub(self.next) > (self.fps * 2.0) as u64
        {
            self.restart(index)?;
        }
        while self.next <= index {
            self.read_next(rgba)?;
        }
        Ok(())
    }
    fn read_next(&mut self, rgba: &mut [u8]) -> PortResult<()> {
        if let Err(error) = self
            .stdout
            .as_mut()
            .ok_or("Missing decoder pipe")?
            .read_exact(rgba)
        {
            self.close();
            let diagnostic = self.diagnostics.lock().map_err(|e| e.to_string())?;
            return Err(format!(
                "Video decode failed: {error}. {}",
                String::from_utf8_lossy(&diagnostic).trim()
            ));
        }
        self.next += 1;
        Ok(())
    }
}
impl Drop for DecoderProcess {
    fn drop(&mut self) {
        self.close();
    }
}
