//! Fenced headless playback latency, not presentation FPS. Uses only caller-provided fixtures.
use std::{path::PathBuf, sync::Arc, time::Instant};
use vjay_application::{ports::*, session::Session};
use vjay_infrastructure::{
    gpu::WgpuRuntimeFactory, media::FfmpegDecoderFactory, storage::JsonProjectStore,
};

fn main() -> Result<(), String> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() < 2 {
        return Err(
            "Usage: playback_bench A.mp4 B.mp4 [WIDTHxHEIGHT] [FRAMES] [legacy|native]".into(),
        );
    }
    let size = args.get(2).map_or("1920x1080", String::as_str);
    let (w, h) = size.split_once('x').ok_or("Use WIDTHxHEIGHT")?;
    let extent = Extent::new(
        w.parse().map_err(|_| "width")?,
        h.parse().map_err(|_| "height")?,
    )?;
    let frames: usize = args
        .get(3)
        .map_or(Ok(120), |s| s.parse())
        .map_err(|_| "frames")?;
    if !(4..=3600).contains(&frames) {
        return Err("Frames: 4..3600".into());
    }
    let mode = args.get(4).map_or("legacy", String::as_str);
    if !matches!(mode, "legacy" | "native") {
        return Err("Mode must be legacy or native".into());
    }
    #[cfg(not(target_os = "macos"))]
    if mode == "native" {
        return Err("The native movie adapter requires macOS Metal".into());
    }
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("sample/anime-club.vjay.json");
    let store = JsonProjectStore::new(path);
    let mut doc = store.load()?;
    for node in &mut doc.nodes {
        node.settings.preview = false;
    }
    let find = |label: &str| {
        doc.nodes
            .iter()
            .find(|n| n.settings.label == label)
            .unwrap()
            .id
    };
    let (a, b) = (find("A / VIDEO"), find("B / VIDEO"));
    let runtime = WgpuRuntimeFactory::headless(
        Arc::new(FfmpegDecoderFactory::videotoolbox(
            "ffmpeg".into(),
            "ffprobe".into(),
        )),
        vjay_infrastructure::gpu::programs::builtin_programs(),
    )?;
    #[cfg(target_os = "macos")]
    let runtime = if mode == "native" {
        runtime.with_gpu_video(Some(Arc::new(
            vjay_infrastructure::media::macos::MetalMovieFactory::new(
                "ffmpeg".into(),
                "ffprobe".into(),
            ),
        )))
    } else {
        runtime
    };
    let mut session = Session::new(
        Box::new(vjay_domain::builtins::registry()),
        &runtime,
        Box::new(store),
        extent,
    )?;
    session.restore(doc.clone())?;
    session.import_video(PathBuf::from(&args[0]), Some(a))?;
    session.import_video(PathBuf::from(&args[1]), Some(b))?;
    for _ in 0..5 {
        session.render_at(0.0)?;
    }
    session.wait_idle()?;
    println!(
        "mode,pattern,width,height,frames,p50_ms,p95_ms,p99_ms,max_ms,total_ms,video_cpu_bytes,delivered,decoder_starts,seek_resets,cache_hits,over_16_67_ms,image_passes,forwarded_sources,pooled_texture_bytes"
    );
    for pattern in ["sequential", "jog", "seek"] {
        let frames = if pattern == "seek" {
            frames.min(16)
        } else {
            frames
        };
        let before = session.media_metrics();
        let mut times = Vec::with_capacity(frames);
        let start = Instant::now();
        for i in 0..frames {
            let frame = Instant::now();
            if pattern != "sequential" {
                let position = if pattern == "jog" {
                    [0.5, 0.502, 0.5, 0.498][i % 4]
                } else {
                    [0.85, 0.15, 0.65, 0.35, 0.95, 0.05, 0.75, 0.25][i % 8]
                };
                session.seek_clip(a, position)?;
                session.seek_clip(b, 1.0 - position)?;
            }
            session.render_at(if pattern == "sequential" {
                i as f64 / 60.0
            } else {
                0.0
            })?;
            session.wait_idle()?;
            times.push(frame.elapsed().as_secs_f64() * 1000.0);
        }
        let total = start.elapsed().as_secs_f64() * 1000.0;
        let after = session.media_metrics();
        let render = session.render_metrics();
        let late = times.iter().filter(|&&t| t > 1000.0 / 60.0).count();
        times.sort_by(f64::total_cmp);
        let percentile = |p: f64| times[((times.len() - 1) as f64 * p).ceil() as usize];
        println!(
            "{mode},{pattern},{},{},{frames},{:.3},{:.3},{:.3},{:.3},{total:.3},{},{},{},{},{},{late},{},{},{}",
            extent.width,
            extent.height,
            percentile(0.5),
            percentile(0.95),
            percentile(0.99),
            times[frames - 1],
            after.cpu_pixel_bytes - before.cpu_pixel_bytes,
            after.frames_delivered - before.frames_delivered,
            after.decoder_starts - before.decoder_starts,
            after.seek_resets - before.seek_resets,
            after.cache_hits - before.cache_hits,
            render.image_passes,
            render.forwarded_sources,
            render.pooled_texture_bytes
        );
    }
    Ok(())
}
