use std::{
    io::{BufWriter, Write},
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{self, SyncSender},
    },
};
use vjay_application::{
    ports::PortResult,
    ports::telemetry::{FrameSample, FrameTrace},
};

/// Opt-in CSV tracing. Filesystem writes and formatting stay on one I/O worker.
pub struct CsvFrameTrace {
    send: SyncSender<FrameSample>,
    dropped: AtomicU64,
}
impl CsvFrameTrace {
    pub fn new(path: PathBuf) -> PortResult<Self> {
        let (send, receive) = mpsc::sync_channel::<FrameSample>(512);
        std::thread::Builder::new().name("vjay-frame-trace".into()).spawn(move || {
            let run = || -> std::io::Result<()> {
                let mut file = BufWriter::with_capacity(65536, std::fs::File::create(path)?);
                writeln!(file, "monotonic_s,interval_ms,target_ms,control_ms,render_ms,total_ms,failed,submissions,image_passes,forwarded_sources,pooled_texture_bytes")?;
                for (i, s) in receive.iter().enumerate() {
                    writeln!(file, "{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{},{},{},{},{}", s.monotonic_seconds,s.interval_seconds*1000.0,s.target_seconds*1000.0,s.control_seconds*1000.0,s.render_seconds*1000.0,s.total_seconds*1000.0,s.failed,s.render.submitted_frames,s.render.image_passes,s.render.forwarded_sources,s.render.pooled_texture_bytes)?;
                    if i % 30 == 29 { file.flush()?; }
                }
                file.flush()
            };
            if let Err(error) = run() { eprintln!("Frame trace stopped: {error}"); }
        }).map_err(|e| e.to_string())?;
        Ok(Self {
            send,
            dropped: AtomicU64::new(0),
        })
    }
}
impl FrameTrace for CsvFrameTrace {
    fn record(&self, sample: FrameSample) {
        if self.send.try_send(sample).is_err() {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
    fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }
}
