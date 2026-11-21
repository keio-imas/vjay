#![cfg(target_os = "macos")]
use std::{
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};
use vjay_application::{media::FrameDelivery, ports::*, project::Point, session::Session};
use vjay_domain::{
    builtins::{ids, registry},
    graph::Connection,
};
use vjay_infrastructure::{
    gpu::{GpuVideoFactory, WgpuRuntimeFactory},
    media::{FfmpegDecoderFactory, macos::MetalMovieFactory},
    storage::JsonProjectStore,
};

fn fixture(path: &Path, codec: &str) {
    let mut command = std::process::Command::new("ffmpeg");
    let pixel = if codec.starts_with("hevc") {
        "p010le"
    } else if codec.starts_with("prores") {
        "yuv422p10le"
    } else {
        "yuv420p"
    };
    command.args(["-v", "error", "-f", "lavfi", "-i",
        "color=red:s=320x180:r=30:d=0.5[r];color=blue:s=320x180:r=30:d=0.5[b];[r][b]concat=n=2:v=1:a=0",
        "-c:v", codec, "-pix_fmt", pixel]);
    if codec.ends_with("videotoolbox") {
        command.args(["-allow_sw", "0"]);
    }
    if codec == "libx264" {
        command.args([
            "-x264-params",
            "b-adapt=0:keyint=60:scenecut=0",
            "-bf",
            "3",
            "-threads",
            "1",
        ]);
    }
    if codec.starts_with("hevc") {
        command.args(["-profile:v", "main10"]);
    }
    let output = command.arg(path).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn factory() -> MetalMovieFactory {
    MetalMovieFactory::new("ffmpeg".into(), "ffprobe".into())
}
fn runtime() -> WgpuRuntimeFactory {
    WgpuRuntimeFactory::headless(
        Arc::new(FfmpegDecoderFactory::videotoolbox(
            "ffmpeg".into(),
            "ffprobe".into(),
        )),
        vjay_infrastructure::gpu::programs::builtin_programs(),
    )
    .unwrap()
    .with_gpu_video(Some(Arc::new(factory())))
}

#[test]
#[ignore = "requires Metal, VideoToolbox and FFmpeg; verifies pixels and persistent decoding"]
fn native_movies_seek_loop_and_cache_without_cpu_pixels() {
    let directory = tempfile::tempdir().unwrap();
    let runtime = runtime();
    for codec in [
        "h264_videotoolbox",
        "libx264",
        "hevc_videotoolbox",
        "prores_videotoolbox",
    ] {
        let path = directory.path().join(format!("{codec}.mov"));
        fixture(&path, codec);
        let mut session = Session::new(
            Box::new(registry()),
            &runtime,
            Box::new(JsonProjectStore::new(directory.path().join("unused.json"))),
            Extent::new(160, 90).unwrap(),
        )
        .unwrap();
        let out = session.project().output.unwrap();
        let video = session.import_video(path, None).unwrap();
        session
            .connect(Connection {
                from: video,
                output: 0,
                to: out,
                input: 0,
            })
            .unwrap();
        for _ in 0..3 {
            session
                .render_at(0.0)
                .unwrap_or_else(|e| panic!("{codec}: {e}"));
        }
        for position in [0.0, 0.7, 0.0, 0.7, 0.0, 0.7, 0.999, 0.0] {
            session.seek_clip(video, position).unwrap();
            for _ in 0..3 {
                session
                    .render_at(0.0)
                    .unwrap_or_else(|e| panic!("{codec}: {e}"));
            }
            let pixels = session.capture().unwrap().rgba;
            let p = &pixels[(45 * 160 + 80) * 4..][..4];
            assert!(
                if position < 0.5 {
                    p[0] > 230 && p[2] < 20
                } else {
                    p[0] < 20 && p[2] > 230
                },
                "{codec} at {position}: {p:?}"
            );
        }
        for frame in 0..30 {
            let position = frame as f32 / 30.0;
            session.seek_clip(video, position).unwrap();
            session
                .render_at(0.0)
                .unwrap_or_else(|e| panic!("{codec} frame {frame}: {e}"));
            let pixels = session.capture().unwrap().rgba;
            let p = &pixels[(45 * 160 + 80) * 4..][..4];
            assert!(
                if frame < 15 {
                    p[0] > 230 && p[2] < 20
                } else {
                    p[0] < 20 && p[2] > 230
                },
                "{codec} frame {frame}: {p:?}"
            );
        }
        let metrics = session.media_metrics();
        assert_eq!(metrics.cpu_pixel_bytes, 0);
        assert_eq!(
            metrics.decoder_starts, 1,
            "seek must reuse the hardware session"
        );
        assert!(metrics.cache_hits >= 4, "{metrics:?}");
        // Uniform edits keep both prepared GPU resources and the decoder alive.
        let transform = session
            .add_node(ids::TRANSFORM, Point { x: 0.0, y: 0.0 })
            .unwrap();
        session.disconnect(out, 0);
        session
            .connect(Connection {
                from: video,
                output: 0,
                to: transform,
                input: 0,
            })
            .unwrap();
        session
            .connect(Connection {
                from: transform,
                output: 0,
                to: out,
                input: 0,
            })
            .unwrap();
        session
            .render_at(0.0)
            .unwrap_or_else(|e| panic!("{codec}: {e}"));
        assert_eq!(session.media_metrics().decoder_starts, 1);
    }
}

#[test]
#[ignore = "requires Metal and VideoToolbox; tests asynchronous delivery under moving requests"]
fn latest_delivery_keeps_progress_and_deletion_does_not_join_decoder() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("clip.mov");
    fixture(&path, "h264_videotoolbox");
    let runtime = runtime();
    let context = runtime.context();
    let mut movies = factory().create(&context.device, &context.queue).unwrap();
    let mut decoder = movies.open(&path, Extent::new(160, 90).unwrap()).unwrap();
    decoder
        .frame_at(0.0, FrameDelivery::Exact, &context.queue)
        .unwrap();
    let before = decoder.metrics();
    let start = Instant::now();
    for frame in 0..120 {
        let deadline = start + Duration::from_secs_f64(frame as f64 / 60.0);
        std::thread::sleep(deadline.saturating_duration_since(Instant::now()));
        decoder
            .frame_at(
                (frame as f64 / 60.0) % 1.0,
                FrameDelivery::Latest,
                &context.queue,
            )
            .unwrap();
        context.queue.submit([]);
    }
    assert!(decoder.metrics().frames_delivered - before.frames_delivered > 35);
    assert_eq!(decoder.metrics().decoder_starts, 1);
    decoder
        .frame_at(0.1, FrameDelivery::Latest, &context.queue)
        .unwrap();
    let dropping = Instant::now();
    drop(decoder);
    assert!(dropping.elapsed() < Duration::from_millis(100));
}
