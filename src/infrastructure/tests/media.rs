use std::{
    fs,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
use vjay_application::{
    media::{FrameDelivery, VideoDecoderFactory},
    ports::Extent,
};
use vjay_infrastructure::media::FfmpegDecoderFactory;

#[test]
#[ignore = "requires ffmpeg and ffprobe on PATH"]
fn video_decodes_seeks_loops_and_recycles_frames() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("vjay-media-{}-{nonce}.mp4", std::process::id()));
    let status = Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=96x64:rate=30",
            "-t",
            "1",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-y",
        ])
        .arg(&path)
        .status()
        .unwrap();
    assert!(status.success());
    let f = FfmpegDecoderFactory::new("ffmpeg".into(), "ffprobe".into());
    let mut decoder = f.open(&path, Extent::new(65, 37).unwrap()).unwrap();
    assert!((decoder.duration() - 1.0).abs() < 0.05);
    let first = decoder
        .frame_at(0.0, FrameDelivery::Exact)
        .unwrap()
        .unwrap();
    assert_eq!(first.rgba.len(), 65 * 37 * 4);
    assert!(
        first
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| pixel[3] == 255)
    );
    assert!(
        decoder
            .frame_at(0.0, FrameDelivery::Latest)
            .unwrap()
            .is_none()
    );
    let next = decoder
        .frame_at(0.5, FrameDelivery::Exact)
        .unwrap()
        .unwrap();
    assert_ne!(first.rgba, next.rgba);
    decoder.recycle(next);
    let backward = decoder
        .frame_at(0.1, FrameDelivery::Exact)
        .unwrap()
        .unwrap();
    let expected = backward.rgba.clone();
    decoder.recycle(backward);
    let forward = decoder
        .frame_at(0.6, FrameDelivery::Exact)
        .unwrap()
        .unwrap();
    decoder.recycle(forward);
    let looped = decoder
        .frame_at(1.1, FrameDelivery::Exact)
        .unwrap()
        .unwrap();
    assert_eq!(looped.rgba, expected);
    let near_end = decoder
        .frame_at(0.99, FrameDelivery::Exact)
        .unwrap()
        .unwrap();
    assert!((near_end.seconds - 29.0 / 30.0).abs() < 1e-6);
    decoder.recycle(near_end);
    decoder.recycle(first);
    decoder.recycle(looped);
    drop(decoder);
    fs::remove_file(path).unwrap();
    assert!(
        f.open(
            std::path::Path::new("/missing/vjay-test.mp4"),
            Extent::new(64, 36).unwrap()
        )
        .is_err()
    );
}
