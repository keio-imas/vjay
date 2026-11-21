#![cfg(target_os = "macos")]
use std::{path::Path, sync::Arc};
use vjay_application::{ports::*, project::ProjectDocument, session::Session};
use vjay_domain::{builtins::registry, graph::NodeId};
use vjay_infrastructure::{
    file_collection::FileCollectionFactory, gpu::WgpuRuntimeFactory, media::FfmpegDecoderFactory,
    storage::JsonProjectStore,
};

fn clip(path: &Path, color: &str) {
    let output = std::process::Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            &format!("color={color}:size=160x90:rate=25:duration=2"),
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-threads",
            "1",
            "-y",
        ])
        .arg(path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
fn settle(session: &mut Session) {
    for _ in 0..5 {
        session.render_at(0.0).unwrap();
    }
}
fn load_folder(session: &mut Session, node: NodeId, folder: &Path) {
    let mut settings = session.project().graph.node(node).unwrap().settings.clone();
    settings.properties.insert(
        "collection.path".into(),
        folder.to_string_lossy().into_owned(),
    );
    session.set_settings(node, settings).unwrap();
    settle(session);
}

#[test]
#[ignore = "requires native Metal GPU, ffmpeg and VideoToolbox; generates its own clips"]
fn two_deck_sample_loads_files_formats_seek_time_and_crossfades_gpu_video() {
    let directory = tempfile::tempdir().unwrap();
    for (name, color) in [("a", "red"), ("b", "blue")] {
        std::fs::create_dir(directory.path().join(name)).unwrap();
        clip(&directory.path().join(name).join("clip.mp4"), color);
    }
    let doc: ProjectDocument =
        serde_json::from_str(include_str!("../../../sample/anime-club.vjay.json")).unwrap();
    let find = |label: &str| {
        doc.nodes
            .iter()
            .find(|n| n.settings.label == label)
            .unwrap()
            .id
    };
    let a = find("A / VIDEO");
    let b = find("B / VIDEO");
    let runtime = WgpuRuntimeFactory::headless(
        Arc::new(FfmpegDecoderFactory::videotoolbox(
            "ffmpeg".into(),
            "ffprobe".into(),
        )),
        vjay_infrastructure::gpu::programs::builtin_programs(),
    )
    .unwrap()
    .with_asset_sources(Arc::new(FileCollectionFactory::default()))
    .with_gpu_video(Some(Arc::new(
        vjay_infrastructure::media::macos::MetalMovieFactory::new(
            "ffmpeg".into(),
            "ffprobe".into(),
        ),
    )));
    let mut session = Session::new(
        Box::new(registry()),
        &runtime,
        Box::new(JsonProjectStore::new(directory.path().join("unused.json"))),
        Extent::new(160, 90).unwrap(),
    )
    .unwrap();
    session.restore(doc.clone()).unwrap();
    settle(&mut session);
    assert!(session.plan().unwrap().program_active);
    assert!(session.project().assets.is_empty());
    load_folder(&mut session, find("A / FILES"), &directory.path().join("a"));
    let red = session.capture().unwrap().rgba;
    assert!(red[0] > 230 && red[2] < 15, "A must play while B is empty");
    load_folder(&mut session, find("B / FILES"), &directory.path().join("b"));
    for (label, position) in [("A / SEEK + TIMECODE", 0.25), ("B / SEEK + TIMECODE", 0.75)] {
        session.set_parameter(find(label), 0, position).unwrap();
    }
    settle(&mut session);
    for (label, expected) in [
        ("A / Seek", "00:00:00.500 / 00:00:02.000"),
        ("B / Seek", "00:00:01.500 / 00:00:02.000"),
    ] {
        let control = session
            .project()
            .controls
            .iter()
            .find(|c| c.label == label)
            .unwrap();
        assert_eq!(session.control_readout(control, 3), expected);
    }
    session
        .set_parameter(find("CROSSFADE A / B"), 0, 1.0)
        .unwrap();
    settle(&mut session);
    let blue = session.capture().unwrap().rgba;
    assert!(blue[0] < 15 && blue[2] > 230);
    session
        .set_parameter(find("MASTER / BLACKOUT"), 0, 1.0)
        .unwrap();
    settle(&mut session);
    assert!(
        session
            .capture()
            .unwrap()
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| p[..3] == [0, 0, 0])
    );
    assert!((session.media_state(a).unwrap().position - 0.25).abs() < 1e-5);
    assert!((session.media_state(b).unwrap().position - 0.75).abs() < 1e-5);
}
