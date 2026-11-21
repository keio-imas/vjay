pub mod audio;
pub mod file_collection;
pub mod images;
#[cfg(target_os = "macos")]
pub mod macos;
pub mod text_images;
pub mod web_media;
use std::path::{Path, PathBuf};
use vjay_application::{
    media::{VideoDecoder, VideoDecoderFactory},
    ports::{Extent, PortResult},
};

mod acceleration;
pub(crate) mod budget;
mod cache;
mod process;
mod stream;
mod videotoolbox;

/// Portable FFmpeg adapter. Native composition roots select a hardware policy.
pub struct FfmpegDecoderFactory {
    ffmpeg: PathBuf,
    ffprobe: PathBuf,
    hardware: bool,
}
impl FfmpegDecoderFactory {
    pub fn new(ffmpeg: PathBuf, ffprobe: PathBuf) -> Self {
        Self {
            ffmpeg,
            ffprobe,
            hardware: false,
        }
    }
    pub fn videotoolbox(ffmpeg: PathBuf, ffprobe: PathBuf) -> Self {
        Self {
            ffmpeg,
            ffprobe,
            hardware: true,
        }
    }
}

#[derive(Clone)]
struct VideoInfo {
    path: PathBuf,
    codec: String,
    format: String,
    width: u32,
    height: u32,
    duration: f64,
    fps: f64,
}
impl VideoDecoderFactory for FfmpegDecoderFactory {
    fn open(&self, path: &Path, extent: Extent) -> PortResult<Box<dyn VideoDecoder>> {
        let info = probe(&self.ffprobe, path)?;
        Ok(Box::new(stream::FfmpegStream::open(
            self.ffmpeg.clone(),
            info,
            extent,
            self.hardware,
        )?))
    }
}
fn probe(ffprobe: &Path, path: &Path) -> PortResult<VideoInfo> {
    let path = path
        .canonicalize()
        .map_err(|e| format!("Cannot open video: {e}"))?;
    if !path.is_file() {
        return Err("Choose a local video file".into());
    }
    let output = std::process::Command::new(ffprobe)
        .args([
            "-v",
            "error",
            "-protocol_whitelist",
            "file,pipe",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=codec_name,pix_fmt,width,height,avg_frame_rate,duration:format=duration",
            "-of",
            "json",
        ])
        .arg(&path)
        .output()
        .map_err(|e| format!("Cannot run ffprobe: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "Video probe failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let metadata: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
    let video = metadata["streams"]
        .as_array()
        .and_then(|s| s.first())
        .ok_or("File has no video stream")?;
    let duration = video["duration"]
        .as_str()
        .and_then(|s| s.parse::<f64>().ok())
        .or_else(|| {
            metadata["format"]["duration"]
                .as_str()
                .and_then(|s| s.parse().ok())
        })
        .filter(|d| d.is_finite() && *d > 0.0)
        .ok_or("Video has no finite duration")?;
    let fps = video["avg_frame_rate"]
        .as_str()
        .and_then(|rate| {
            let (n, d) = rate.split_once('/')?;
            Some(n.parse::<f64>().ok()? / d.parse::<f64>().ok()?)
        })
        .filter(|f| f.is_finite() && *f > 0.0)
        .unwrap_or(30.0)
        .clamp(1.0, 120.0);
    let info = VideoInfo {
        path,
        codec: video["codec_name"].as_str().unwrap_or("").into(),
        format: video["pix_fmt"].as_str().unwrap_or("").into(),
        width: video["width"].as_u64().unwrap_or(0) as u32,
        height: video["height"].as_u64().unwrap_or(0) as u32,
        duration,
        fps,
    };
    Ok(info)
}
