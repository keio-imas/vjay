use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use vjay_application::{
    asset_sources::AssetSourceFactory, media::FrameDelivery, ports::*, session::Session,
};
use vjay_domain::{
    builtins::{ids, registry},
    execution::AssetRequest,
    graph::Connection,
};
use vjay_infrastructure::{
    gpu::WgpuRuntimeFactory, media::FfmpegDecoderFactory, storage::JsonProjectStore,
    web_media::YtdlpAssetFactory,
};
fn executable() -> PathBuf {
    std::env::var_os("VJAY_TEST_YTDLP")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.venv-web/bin/yt-dlp")
        })
}
#[test]
fn url_source_rejects_file_urls_before_starting_workers() {
    let f = YtdlpAssetFactory::new("unused".into(), "ffmpeg".into());
    for location in [
        "file:///etc/passwd",
        "javascript:alert(1)",
        "https://name:password@example.com/video",
    ] {
        let r = AssetRequest {
            provider: "web-video",
            selection_parameter: None,
            file_output: false,
            location: location.into(),
            options: Default::default(),
        };
        assert!(f.start(&r).is_err());
    }
}
#[test]
#[ignore = "requires network, yt-dlp, FFmpeg, and a native GPU"]
fn youtube_url_is_cached_and_rendered_through_the_gpu_video_decoder() {
    let url = std::env::var("VJAY_TEST_URL")
        .unwrap_or_else(|_| "https://www.youtube.com/watch?v=jNQXAC9IVRw".into());
    let dir = tempfile::tempdir().unwrap();
    let factory = Arc::new(
        YtdlpAssetFactory::new(executable(), "ffmpeg".into()).with_cache(dir.path().into()),
    );
    let media = Arc::new(FfmpegDecoderFactory::videotoolbox(
        "ffmpeg".into(),
        "ffprobe".into(),
    ));
    let runtime = WgpuRuntimeFactory::headless(
        media,
        vjay_infrastructure::gpu::programs::builtin_programs(),
    )
    .unwrap()
    .with_asset_sources(factory.clone());
    let mut s = Session::new(
        Box::new(registry()),
        &runtime,
        Box::new(JsonProjectStore::new(dir.path().join("unused.json"))),
        Extent::new(320, 180).unwrap(),
    )
    .unwrap();
    let output = s.project().output.unwrap();
    let web = s
        .add_node(
            "InternalBrowser",
            vjay_application::project::Point { x: 20.0, y: 30.0 },
        )
        .unwrap();
    let mut settings = s.project().graph.node(web).unwrap().settings.clone();
    settings.properties.insert("url".into(), url.clone());
    settings
        .properties
        .insert("max_height".into(), "360".into());
    s.set_settings(web, settings).unwrap();
    s.connect(Connection {
        from: web,
        output: 0,
        to: output,
        input: 0,
    })
    .unwrap();
    s.render_at(0.0).unwrap();
    let frame = s.capture().unwrap();
    assert!(
        frame
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .any(|p| p[0] > 10 && p[1] > 10)
    );
    assert!(s.media_status().unwrap_or("").contains("GPU"));
    let path = s.project().assets[0].path.clone();
    assert!(path.is_file());
    let request = AssetRequest {
        provider: "web-video",
        selection_parameter: None,
        file_output: false,
        location: url,
        options: s
            .project()
            .graph
            .node(web)
            .unwrap()
            .settings
            .properties
            .clone(),
    };
    let cached = YtdlpAssetFactory::new("missing-downloader".into(), "ffmpeg".into())
        .with_cache(dir.path().into());
    let start = Instant::now();
    let resolved = cached
        .start(&request)
        .unwrap()
        .poll(FrameDelivery::Exact)
        .unwrap()
        .unwrap();
    assert_eq!(resolved.path(), path);
    assert!(start.elapsed() < Duration::from_secs(5));
    let local = s
        .add_node(
            ids::MULTIPLY,
            vjay_application::project::Point { x: 0.0, y: 0.0 },
        )
        .unwrap();
    s.set_parameter(local, 1, 5.0).unwrap();
    s.render_at(0.1).unwrap();
    assert_eq!(s.project().assets[0].path, path);
}

#[cfg(unix)]
#[test]
#[ignore = "requires native process execution and 512 MiB free cache space"]
fn dropping_a_pending_import_terminates_its_own_downloader() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("downloader");
    std::fs::write(
        &script,
        "#!/bin/sh\nprintf '%s' $$ > \"$0.pid\"\nsleep 30\n",
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
    let factory = YtdlpAssetFactory::new(script.clone(), "ffmpeg".into())
        .with_cache(dir.path().join("cache"));
    let request = AssetRequest {
        provider: "web-video",
        selection_parameter: None,
        file_output: false,
        location: "https://example.invalid/video".into(),
        options: Default::default(),
    };
    let job = factory.start(&request).unwrap();
    let pid_file = script.with_extension("pid");
    let start = Instant::now();
    while !pid_file.is_file() {
        assert!(start.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(10));
    }
    let pid: libc::pid_t = std::fs::read_to_string(&pid_file).unwrap().parse().unwrap();
    drop(job);
    let start = Instant::now();
    // SAFETY: signal zero only probes the process created by this test.
    while unsafe { libc::kill(pid, 0) } == 0 {
        assert!(start.elapsed() < Duration::from_secs(3));
        std::thread::sleep(Duration::from_millis(10));
    }
}
