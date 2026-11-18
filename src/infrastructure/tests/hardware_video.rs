use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use vjay_application::{
    media::{FrameDelivery, VideoDecoderFactory},
    ports::Extent,
};
use vjay_infrastructure::media::FfmpegDecoderFactory;

#[test]
#[ignore = "requires macOS VideoToolbox; set VJAY_TEST_VIDEO to a 4K source"]
fn gpu_video_keeps_up_with_realtime_requests_and_survives_loop_and_seek() {
    let path = PathBuf::from(std::env::var_os("VJAY_TEST_VIDEO").expect("set VJAY_TEST_VIDEO"));
    let extent = std::env::var("VJAY_TEST_EXTENT")
        .ok()
        .and_then(|v| {
            let (w, h) = v.split_once('x')?;
            Extent::new(w.parse().ok()?, h.parse().ok()?).ok()
        })
        .unwrap_or(Extent::new(960, 540).unwrap());
    let factory = FfmpegDecoderFactory::videotoolbox("ffmpeg".into(), "ffprobe".into());
    let preparing = Instant::now();
    let mut decoder = factory.open(&path, extent).unwrap();
    let frame = decoder
        .frame_at(0.0, FrameDelivery::Exact)
        .unwrap()
        .unwrap();
    decoder.recycle(frame);
    let prepared = preparing.elapsed();
    let duration = decoder.duration();
    let start = Instant::now();
    let mut count = 0;
    let mut max_age = 0.0f64;
    let mut max_call = Duration::ZERO;
    let mut last = 0.0;
    for tick in 0..720 {
        let deadline = start + Duration::from_secs_f64(tick as f64 / 60.0);
        std::thread::sleep(deadline.saturating_duration_since(Instant::now()));
        let seconds = tick as f64 / 60.0;
        let requested = seconds % duration;
        let before = Instant::now();
        let frame = decoder.frame_at(requested, FrameDelivery::Latest).unwrap();
        max_call = max_call.max(before.elapsed());
        if let Some(frame) = frame {
            count += 1;
            last = frame.seconds;
            let age = (requested - last).rem_euclid(duration);
            if age < duration * 0.5 {
                max_age = max_age.max(age);
            }
            decoder.recycle(frame);
        }
    }
    assert!(count >= 200, "only {count} frames delivered in 12 seconds");
    assert!(
        max_call < Duration::from_millis(40),
        "playback blocked: {max_call:?}"
    );
    assert!(max_age < 0.30, "stale decoded frame: {max_age}");
    for time in [duration * 0.9, duration * 0.1, duration - 0.001, 0.0] {
        let frame = decoder
            .frame_at(time, FrameDelivery::Exact)
            .unwrap()
            .unwrap();
        decoder.recycle(frame);
    }
    println!(
        "GPU video {}x{}: prepare={prepared:?}, delivered={count}/12s, max call={max_call:?}, max age={max_age:.3}s, last={last:.3}s",
        extent.width, extent.height
    );
}

#[test]
#[ignore = "requires macOS hardware H.264/HEVC encoders and VideoToolbox"]
fn common_4k_h264_and_hevc_decode_directly_on_gpu() {
    for (codec, format) in [
        ("h264_videotoolbox", "nv12"),
        ("hevc_videotoolbox", "p010le"),
    ] {
        let path =
            std::env::temp_dir().join(format!("vjay-native-4k-{}-{codec}.mov", std::process::id()));
        let mut command = std::process::Command::new("ffmpeg");
        command.args([
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=3840x2160:rate=30:duration=1",
            "-c:v",
            codec,
            "-allow_sw",
            "0",
            "-pix_fmt",
            format,
        ]);
        if codec.starts_with("hevc") {
            command.args(["-profile:v", "main10"]);
        }
        assert!(command.arg(&path).status().unwrap().success());
        let f = FfmpegDecoderFactory::videotoolbox("ffmpeg".into(), "ffprobe".into());
        let mut decoder = f.open(&path, Extent::new(1280, 720).unwrap()).unwrap();
        for time in [0.0, 0.5, 0.99, 0.0, 0.2] {
            let frame = decoder
                .frame_at(time, FrameDelivery::Exact)
                .unwrap()
                .unwrap();
            assert_eq!(frame.rgba.len(), 1280 * 720 * 4);
            assert!(frame.rgba.iter().any(|v| *v > 0));
            decoder.recycle(frame);
        }
        drop(decoder);
        std::fs::remove_file(path).unwrap();
    }
}
