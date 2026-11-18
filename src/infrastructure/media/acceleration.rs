use super::{VideoInfo, cache, process::ChildSlot};
use std::{
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};
use vjay_application::ports::{Extent, PortResult};

pub(super) struct PreparedVideo {
    pub path: PathBuf,
    pub hardware: bool,
    pub filter: String,
    pub frames: u64,
    pub _cache: Option<cache::CacheFile>,
}
pub(super) fn prepare(
    ffmpeg: &std::path::Path,
    info: &VideoInfo,
    extent: Extent,
    hardware: bool,
    slot: &ChildSlot,
    stop: &Arc<AtomicBool>,
) -> PortResult<PreparedVideo> {
    let needs_cache = hardware
        && !matches!(
            (info.codec.as_str(), info.format.as_str()),
            ("h264", "yuv420p" | "nv12")
                | ("hevc", "yuv420p" | "yuv420p10le" | "p010le")
                | ("prores", "yuv422p10le" | "p210le")
        );
    if hardware {
        super::videotoolbox::require_hardware(if needs_cache { "prores" } else { &info.codec })?;
    }
    let cached = if needs_cache {
        Some(cache::prepare(ffmpeg, info, slot, stop)?)
    } else {
        None
    };
    let path = cached
        .as_ref()
        .map_or_else(|| info.path.clone(), |c| c.path.clone());
    let scale = if hardware {
        if info.width == 0 || info.height == 0 {
            return Err("Video dimensions are missing".into());
        }
        let ratio = (extent.width as f64 / info.width as f64)
            .min(extent.height as f64 / info.height as f64);
        let width = ((info.width as f64 * ratio).floor() as u32 / 2 * 2).max(2);
        let height = ((info.height as f64 * ratio).floor() as u32 / 2 * 2).max(2);
        let format = if needs_cache || info.format.contains("422") || info.format == "p210le" {
            "p210le"
        } else if info.format.contains("10") {
            "p010le"
        } else {
            "nv12"
        };
        format!(
            "scale_vt=w={width}:h={height},hwdownload,format={format},format=rgba,crop={}:{}",
            width.min(extent.width),
            height.min(extent.height)
        )
    } else {
        format!(
            "scale={}:{}:force_original_aspect_ratio=decrease,format=rgba",
            extent.width, extent.height
        )
    };
    Ok(PreparedVideo {
        path,
        hardware,
        frames: (info.duration * info.fps - 1e-6).ceil().max(1.0) as u64,
        _cache: cached,
        filter: format!(
            "{scale},fps={:.12}:eof_action=pass,pad={}:{}:(ow-iw)/2:(oh-ih)/2,setsar=1",
            info.fps, extent.width, extent.height
        ),
    })
}
