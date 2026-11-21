use crate::ports::{Extent, PortResult};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use vjay_domain::node::AssetId;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MediaAsset {
    pub id: AssetId,
    pub path: PathBuf,
}

pub struct VideoFrame {
    pub rgba: Vec<u8>,
    pub seconds: f64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MediaMetrics {
    pub frames_delivered: u64,
    pub cpu_pixel_bytes: u64,
    pub decoder_starts: u64,
    pub seek_resets: u64,
    pub cache_hits: u64,
}
impl std::ops::AddAssign for MediaMetrics {
    fn add_assign(&mut self, rhs: Self) {
        self.frames_delivered += rhs.frames_delivered;
        self.cpu_pixel_bytes += rhs.cpu_pixel_bytes;
        self.decoder_starts += rhs.decoder_starts;
        self.seek_resets += rhs.seek_resets;
        self.cache_hits += rhs.cache_hits;
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FrameDelivery {
    Latest,
    Exact,
}

/// Nonblocking playback port. Decode work belongs to an adapter worker, not UI.
pub trait VideoDecoder {
    fn metrics(&self) -> MediaMetrics {
        MediaMetrics::default()
    }
    fn duration(&self) -> f64;
    fn failed(&self) -> bool {
        false
    }
    fn ready(&self) -> bool {
        true
    }
    fn status(&self) -> Option<&str> {
        None
    }
    fn frame_at(&mut self, seconds: f64, delivery: FrameDelivery)
    -> PortResult<Option<VideoFrame>>;
    fn recycle(&mut self, frame: VideoFrame);
}

pub trait VideoDecoderFactory: Send + Sync {
    /// Opening probes metadata; preparing frames runs on the adapter worker.
    fn open(&self, path: &Path, extent: Extent) -> PortResult<Box<dyn VideoDecoder>>;
}

/// Normalized source range shared by video and optional audio routing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlaybackWindow {
    pub looping: bool,
    pub start: f32,
    pub end: f32,
}
impl Default for PlaybackWindow {
    fn default() -> Self {
        Self {
            looping: true,
            start: 0.0,
            end: 1.0,
        }
    }
}
impl PlaybackWindow {
    pub fn bounds(self, duration: f64) -> (f64, f64) {
        let start = (self.start as f64).clamp(0.0, 0.999999) * duration;
        let end = (self.end as f64 * duration).clamp(start + duration * 0.000001, duration);
        (start, end)
    }
    pub fn resolve(self, base: f64, position: f32, duration: f64) -> (f64, bool) {
        let (start, end) = self.bounds(duration);
        let length = end - start;
        let time = base + position as f64 * length;
        if self.looping {
            (start + time.rem_euclid(length), false)
        } else {
            (
                start + time.clamp(0.0, (length - 0.000001).max(0.0)),
                time >= length || time < 0.0,
            )
        }
    }
}

pub trait ImageLoader: Send + Sync {
    fn load(&self, path: &Path, extent: Extent) -> PortResult<Vec<u8>>;
}
pub trait PreparedImageFactory: Send + Sync {
    fn load(
        &self,
        source: &vjay_domain::execution::PreparedImage,
        extent: Extent,
    ) -> PortResult<Vec<u8>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn subrange_wraps_or_holds_at_end() {
        let window = PlaybackWindow {
            looping: true,
            start: 0.25,
            end: 0.75,
        };
        assert_eq!(window.resolve(0.0, 0.0, 20.0), (5.0, false));
        assert_eq!(window.resolve(12.0, 0.0, 20.0), (7.0, false));
        assert_eq!(window.resolve(0.0, 0.5, 20.0), (10.0, false));
        let (time, ended) = PlaybackWindow {
            looping: false,
            ..window
        }
        .resolve(12.0, 0.0, 20.0);
        assert!(ended);
        assert!((time - 15.0).abs() < 0.00001);
    }
}
