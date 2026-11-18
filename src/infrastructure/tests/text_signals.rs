use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use vjay_application::{
    compiler::compile_with_previews,
    control_io::ControlEvent,
    media::{FrameDelivery, PreparedImageFactory},
    ports::*,
    signals::SignalEngine,
};
use vjay_domain::{
    builtins::{ids, registry},
    execution::PreparedImage,
    graph::{Connection, Graph},
};
use vjay_infrastructure::{
    gpu::WgpuRuntimeFactory,
    media::FfmpegDecoderFactory,
    services::nodes::RekordboxHistoryNode,
    text_images::{PreparedImageRegistry, TextImageProvider},
};
struct Counted {
    calls: Arc<AtomicUsize>,
    inner: PreparedImageRegistry,
}
impl PreparedImageFactory for Counted {
    fn load(&self, p: &PreparedImage, e: Extent) -> PortResult<Vec<u8>> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.inner.load(p, e)
    }
}
#[test]
#[ignore = "requires a native GPU and installed fonts"]
fn rekordbox_title_and_album_update_text_textures_without_preparing_again() {
    let mut f = registry();
    f.register(RekordboxHistoryNode).unwrap();
    let mut g = Graph::default();
    let source = g.add("RekordboxHistory", &f).unwrap();
    let text = g.add("Text", &f).unwrap();
    let album = g.add("Text", &f).unwrap();
    let out = g.add(ids::OUTPUT, &f).unwrap();
    for (from, output, to, input) in [
        (source, 4, text, 0),
        (source, 6, album, 0),
        (text, 0, out, 0),
    ] {
        g.connect(Connection {
            from,
            output,
            to,
            input,
        })
        .unwrap();
    }
    let calls = Arc::new(AtomicUsize::new(0));
    let runtime = WgpuRuntimeFactory::headless(
        Arc::new(FfmpegDecoderFactory::new("ffmpeg".into(), "ffprobe".into())),
        vjay_infrastructure::gpu::programs::builtin_programs(),
    )
    .unwrap()
    .with_prepared_images(Arc::new(Counted {
        calls: calls.clone(),
        inner: PreparedImageRegistry(vec![Box::new(TextImageProvider::default())]),
    }));
    let mut renderer = runtime.create_renderer().unwrap();
    let extent = Extent::new(320, 180).unwrap();
    let mut plan = compile_with_previews(&g, Some(out), &f).unwrap();
    let mut signals = SignalEngine::default();
    signals.rebuild(&g, &f).unwrap();
    signals.bind(&mut plan);
    renderer.prepare(&plan, extent).unwrap();
    let event = |title: &str| ControlEvent::NodeText {
        node: source,
        values: vec![
            Arc::from(title),
            Arc::from("Artist"),
            Arc::from("Album title"),
        ],
    };
    signals.events.push(event("Song A"));
    signals.tick(0.0);
    signals.modulate(&mut plan);
    let frame = FrameContext {
        seconds: 0.0,
        timeline_seconds: 0.0,
        extent,
        delivery: FrameDelivery::Exact,
    };
    renderer.render(&plan, frame).unwrap();
    let a = renderer.capture().unwrap().rgba;
    assert!(a.as_chunks::<4>().0.iter().any(|p| p[0] > 0));
    assert_eq!(signals.text_output(album, 1), Some("Album title"));
    let before = calls.load(Ordering::Relaxed);
    for i in 1..15 {
        signals.events.push(event("Song A"));
        signals.tick(i as f64 / 60.0);
        signals.modulate(&mut plan);
        renderer.render(&plan, frame).unwrap();
    }
    assert_eq!(calls.load(Ordering::Relaxed), before);
    signals.events.push(event("Song B"));
    signals.tick(0.3);
    signals.modulate(&mut plan);
    renderer.render(&plan, frame).unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), before + 1);
    assert_ne!(a, renderer.capture().unwrap().rgba);
    signals.rebuild(&g, &f).unwrap();
    signals.bind(&mut plan);
    signals.tick(0.4);
    signals.modulate(&mut plan);
    renderer.render(&plan, frame).unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), before + 1);
    signals.events.push(event(""));
    signals.tick(0.5);
    signals.modulate(&mut plan);
    renderer.render(&plan, frame).unwrap();
    assert!(
        renderer
            .capture()
            .unwrap()
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| p[0] == 0 && p[1] == 0 && p[2] == 0)
    );
}

#[test]
fn legacy_workspace_zoom_defaults_and_new_zoom_roundtrips() {
    let old = serde_json::json!({"library_width":230.0,"inspector_width":280.0,"top_height":284.0,"preview_ratio":0.5});
    let mut layout: vjay_application::workspace::WorkspaceLayout =
        serde_json::from_value(old).unwrap();
    assert_eq!(layout.patch_zoom, 1.0);
    layout.patch_zoom = 0.5;
    let restored: vjay_application::workspace::WorkspaceLayout =
        serde_json::from_value(serde_json::to_value(layout).unwrap()).unwrap();
    assert_eq!(restored, layout);
    layout.patch_zoom = 0.0;
    assert!(!layout.is_valid());
}

#[test]
#[ignore = "requires a native GPU and installed fonts"]
fn font_family_size_and_style_inputs_change_pixels_without_repreparing() {
    let nodes = registry();
    let mut g = Graph::default();
    let text = g.add("Text", &nodes).unwrap();
    let family = g.add("TextValue", &nodes).unwrap();
    let size = g.add(ids::CONSTANT, &nodes).unwrap();
    let weight = g.add(ids::CONSTANT, &nodes).unwrap();
    let style = g.add(ids::CONSTANT, &nodes).unwrap();
    let out = g.add(ids::OUTPUT, &nodes).unwrap();
    for (from, to, input) in [
        (family, text, 1),
        (size, text, 6),
        (weight, text, 7),
        (style, text, 8),
        (text, out, 0),
    ] {
        g.connect(Connection {
            from,
            output: 0,
            to,
            input,
        })
        .unwrap();
    }
    let calls = Arc::new(AtomicUsize::new(0));
    let runtime = WgpuRuntimeFactory::headless(
        Arc::new(FfmpegDecoderFactory::new("ffmpeg".into(), "ffprobe".into())),
        vjay_infrastructure::gpu::programs::builtin_programs(),
    )
    .unwrap()
    .with_prepared_images(Arc::new(Counted {
        calls: calls.clone(),
        inner: PreparedImageRegistry(vec![Box::new(TextImageProvider::default())]),
    }));
    let mut renderer = runtime.create_renderer().unwrap();
    let extent = Extent::new(320, 180).unwrap();
    let mut signals = SignalEngine::default();
    let mut plan = compile_with_previews(&g, Some(out), &nodes).unwrap();
    let mut pixels = Vec::new();
    for (i, font, font_size, font_weight, font_style) in [
        (0, "monospace", 64.0, 3.0, 0.0),
        (1, "monospace", 120.0, 3.0, 0.0),
        (2, "serif", 120.0, 3.0, 0.0),
        (3, "serif", 120.0, 6.0, 1.0),
    ] {
        let mut settings = g.node(family).unwrap().settings.clone();
        settings
            .properties
            .insert("content.text".into(), font.into());
        g.set_settings(family, settings, &nodes).unwrap();
        for (node, value) in [
            (size, font_size),
            (weight, font_weight),
            (style, font_style),
        ] {
            g.set_parameter(node, 0, value, &nodes).unwrap();
        }
        signals.rebuild(&g, &nodes).unwrap();
        signals.bind(&mut plan);
        signals.tick(i as f64 / 60.0);
        signals.modulate(&mut plan);
        if i == 0 {
            renderer.prepare(&plan, extent).unwrap();
        }
        let frame = FrameContext {
            seconds: 0.0,
            timeline_seconds: 0.0,
            extent,
            delivery: FrameDelivery::Exact,
        };
        renderer.render(&plan, frame).unwrap();
        pixels.push(renderer.capture().unwrap().rgba);
        let before = calls.load(Ordering::Relaxed);
        signals.tick((i as f64 + 0.5) / 60.0);
        signals.modulate(&mut plan);
        renderer.render(&plan, frame).unwrap();
        assert_eq!(calls.load(Ordering::Relaxed), before);
    }
    assert!(pixels.windows(2).all(|p| p[0] != p[1]));
}
