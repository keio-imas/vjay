use std::sync::Arc;
use vjay_application::{ports::*, project::Point, session::Session};
use vjay_domain::{builtins::registry, graph::Connection};
use vjay_infrastructure::{
    file_collection::FileCollectionFactory, gpu::WgpuRuntimeFactory, media::FfmpegDecoderFactory,
    storage::JsonProjectStore,
};

#[test]
#[ignore = "requires macOS VideoToolbox, ffmpeg, and a native GPU"]
#[cfg(target_os = "macos")]
fn folder_video_switches_on_click_index_and_signal_using_hardware_decoding() {
    let directory = tempfile::tempdir().unwrap();
    for (name, color) in [("a.mp4", "red"), ("b.mp4", "blue")] {
        assert!(
            std::process::Command::new("ffmpeg")
                .args([
                    "-hide_banner",
                    "-loglevel",
                    "error",
                    "-f",
                    "lavfi",
                    "-i",
                    &format!("color={color}:size=160x90:rate=25:duration=1"),
                    "-c:v",
                    "libx264",
                    "-pix_fmt",
                    "yuv420p",
                    "-threads",
                    "1",
                    "-y"
                ])
                .arg(directory.path().join(name))
                .status()
                .unwrap()
                .success()
        );
    }
    let runtime = WgpuRuntimeFactory::headless(
        Arc::new(FfmpegDecoderFactory::videotoolbox(
            "ffmpeg".into(),
            "ffprobe".into(),
        )),
        vjay_infrastructure::gpu::programs::builtin_programs(),
    )
    .unwrap()
    .with_asset_sources(Arc::new(FileCollectionFactory::default()));
    let mut session = Session::new(
        Box::new(registry()),
        &runtime,
        Box::new(JsonProjectStore::new(directory.path().join("patch.json"))),
        Extent::new(160, 90).unwrap(),
    )
    .unwrap();
    let out = session.project().output.unwrap();
    let video = session
        .add_node("LoadableVideo", Point { x: 0.0, y: 0.0 })
        .unwrap();
    let mut settings = session
        .project()
        .graph
        .node(video)
        .unwrap()
        .settings
        .clone();
    settings.properties.insert(
        "collection.path".into(),
        directory.path().to_string_lossy().into_owned(),
    );
    session.set_settings(video, settings).unwrap();
    session
        .connect(Connection {
            from: video,
            output: 0,
            to: out,
            input: 0,
        })
        .unwrap();
    session.render_at(0.0).unwrap();
    let red = session.capture().unwrap().rgba;
    assert!(red[0] > 240 && red[2] < 5);
    assert_eq!(session.asset_source_items(video).len(), 2);
    session.set_parameter(video, 6, 1.0).unwrap();
    session.render_at(0.04).unwrap();
    let blue = session.capture().unwrap().rgba;
    assert!(blue[0] < 5 && blue[2] > 240);
    let value = session
        .add_node("Constant", Point { x: 0.0, y: 0.0 })
        .unwrap();
    session.set_parameter(value, 0, 0.0).unwrap();
    session
        .connect(Connection {
            from: value,
            output: 0,
            to: video,
            input: 7,
        })
        .unwrap();
    session.render_at(0.08).unwrap();
    session.render_at(0.12).unwrap();
    let red = session.capture().unwrap().rgba;
    assert!(red[0] > 240 && red[2] < 5);
    assert_eq!(session.project().assets.len(), 1);
    session.set_parameter(video, 0, 0.5).unwrap();
    session.render_at(0.16).unwrap();
    let red = session.capture().unwrap().rgba;
    assert!(red[0] > 240 && red[2] < 5);
    for name in ["a.mp4", "b.mp4"] {
        assert!(directory.path().join(name).exists());
    }
}
