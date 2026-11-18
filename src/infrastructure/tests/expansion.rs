use std::sync::Arc;
use vjay_application::{compiler::compile, media::FrameDelivery, ports::*};
use vjay_domain::{
    builtins::{ids, registry},
    graph::{Connection, Graph, NodeId},
};
use vjay_infrastructure::{
    gpu::WgpuRuntimeFactory,
    media::FfmpegDecoderFactory,
    text_images::{PreparedImageRegistry, TextImageProvider},
};
fn renderer() -> Box<dyn RenderBackend> {
    WgpuRuntimeFactory::headless(
        Arc::new(FfmpegDecoderFactory::new("ffmpeg".into(), "ffprobe".into())),
        vjay_infrastructure::gpu::programs::builtin_programs(),
    )
    .unwrap()
    .with_prepared_images(Arc::new(PreparedImageRegistry(vec![Box::new(
        TextImageProvider::default(),
    )])))
    .create_renderer()
    .unwrap()
}
fn wire(g: &mut Graph, from: NodeId, to: NodeId, input: usize) {
    g.connect(Connection {
        from,
        to,
        input,
        output: 0,
    })
    .unwrap();
}
fn render(r: &mut dyn RenderBackend, g: &Graph, out: NodeId, time: f64) -> Vec<u8> {
    let extent = Extent::new(128, 80).unwrap();
    let plan = compile(g, out, &registry()).unwrap();
    r.prepare(&plan, extent).unwrap();
    r.render(
        &plan,
        FrameContext {
            seconds: time as f32,
            timeline_seconds: time,
            extent,
            delivery: FrameDelivery::Exact,
        },
    )
    .unwrap();
    r.capture().unwrap().rgba
}
fn source(g: &mut Graph) -> NodeId {
    let f = registry();
    let id = g.add(ids::GLSL, &f).unwrap();
    let mut s = g.node(id).unwrap().settings.clone();
    s.shader =
        "void mainImage(out vec4 c,in vec2 p){vec2 uv=p/iResolution.xy;c=vec4(uv,0.25,1.0);}"
            .into();
    g.set_settings(id, s, &f).unwrap();
    id
}
#[test]
#[ignore = "requires a native GPU"]
fn neutral_values_preserve_pixels_including_one_segment_kaleidoscope_and_3d_transform() {
    let f = registry();
    let mut r = renderer();
    let mut g = Graph::default();
    let image = source(&mut g);
    let out = g.add(ids::OUTPUT, &f).unwrap();
    wire(&mut g, image, out, 0);
    let original = render(r.as_mut(), &g, out, 0.0);
    for (kind, values) in [
        (ids::KALEIDOSCOPE, vec![(0, 1.0)]),
        (ids::BLUR, vec![(0, 0.0)]),
        (ids::INVERT, vec![(0, 0.0)]),
        (ids::EDGE, vec![(0, 0.0)]),
        (ids::POSTERIZE, vec![(0, 256.0)]),
        (ids::LUMA_KEY, vec![(0, 0.0)]),
        (ids::PIXELATE, vec![(0, 1.0)]),
        (ids::TRANSFORM, vec![]),
        ("ChromaKey", vec![(3, 0.0)]),
        ("Mirror", vec![(0, 0.0), (1, 0.0)]),
        ("Tunnel3D", vec![(4, 0.0)]),
        ("Levels", vec![]),
        ("Vignette", vec![(2, 0.0)]),
        ("Crop", vec![]),
        ("Transform3D", vec![]),
        ("DuplicateGrid", vec![(0, 1.0), (1, 1.0), (2, 0.0)]),
    ] {
        let node = g.add(kind, &f).unwrap();
        for (p, v) in values {
            g.set_parameter(node, p, v, &f).unwrap();
        }
        wire(&mut g, image, node, 0);
        wire(&mut g, node, out, 0);
        let frame = render(r.as_mut(), &g, out, 0.0);
        assert!(
            frame
                .iter()
                .zip(&original)
                .all(|(a, b)| (*a as i16 - *b as i16).abs() <= 1),
            "{kind} must preserve its input at neutral values"
        );
    }
}
#[test]
#[ignore = "requires a native GPU"]
fn duplicate_grid_repeats_one_input_and_3d_nodes_render_nonempty_images() {
    let f = registry();
    let mut r = renderer();
    let mut g = Graph::default();
    let image = source(&mut g);
    let out = g.add(ids::OUTPUT, &f).unwrap();
    let grid = g.add("DuplicateGrid", &f).unwrap();
    wire(&mut g, image, grid, 0);
    wire(&mut g, grid, out, 0);
    for (p, v) in [(0, 2.0), (1, 2.0), (2, 0.0)] {
        g.set_parameter(grid, p, v, &f).unwrap();
    }
    let frame = render(r.as_mut(), &g, out, 0.0);
    let pixel = |x: usize, y: usize| &frame[(y * 128 + x) * 4..(y * 128 + x) * 4 + 4];
    assert_eq!(pixel(16, 10), pixel(80, 10));
    assert_eq!(pixel(16, 10), pixel(16, 50));
    let shape = g.add("Primitive3D", &f).unwrap();
    wire(&mut g, shape, out, 0);
    for kind in 0..4 {
        g.set_parameter(shape, 0, kind as f32, &f).unwrap();
        let frame = render(r.as_mut(), &g, out, 0.0);
        assert!(
            frame.as_chunks::<4>().0.iter().any(|p| p[3] > 0),
            "shape {kind}"
        );
        assert!(frame.as_chunks::<4>().0.iter().any(|p| p[3] == 0));
    }
    g.set_parameter(shape, 0, 0.0, &f).unwrap();
    g.set_parameter(shape, 4, 0.1, &f).unwrap();
    g.set_parameter(shape, 5, 10.0, &f).unwrap();
    let distant = compile(&g, out, &f).unwrap();
    let extent = Extent::new(129, 81).unwrap();
    r.prepare(&distant, extent).unwrap();
    r.render(
        &distant,
        FrameContext {
            seconds: 0.0,
            timeline_seconds: 0.0,
            extent,
            delivery: FrameDelivery::Exact,
        },
    )
    .unwrap();
    assert!(
        r.capture().unwrap().rgba[(40 * 129 + 64) * 4 + 3] > 0,
        "valid camera distance and small scale must still reach the shape"
    );
    g.set_parameter(shape, 4, 1.0, &f).unwrap();
    g.set_parameter(shape, 5, 3.0, &f).unwrap();
    g.set_parameter(shape, 0, 1.0, &f).unwrap();
    let cube = render(r.as_mut(), &g, out, 0.0);
    g.set_parameter(shape, 2, 35.0, &f).unwrap();
    assert_ne!(cube, render(r.as_mut(), &g, out, 0.0));
    let tunnel = g.add("Tunnel3D", &f).unwrap();
    wire(&mut g, image, tunnel, 0);
    wire(&mut g, tunnel, out, 0);
    assert_ne!(
        render(r.as_mut(), &g, out, 0.0),
        render(r.as_mut(), &g, out, 1.0)
    );
}
#[test]
#[ignore = "requires a native GPU and installed fonts"]
fn text_renders_transparent_unicode_with_gpu_tint_and_updates_after_content_edits() {
    let f = registry();
    let mut r = renderer();
    let mut g = Graph::default();
    let text = g.add("Text", &f).unwrap();
    let out = g.add(ids::OUTPUT, &f).unwrap();
    wire(&mut g, text, out, 0);
    let mut s = g.node(text).unwrap().settings.clone();
    s.properties
        .insert("content.text".into(), "VJAY 日本語\n123".into());
    g.set_settings(text, s.clone(), &f).unwrap();
    g.set_parameter(text, 0, 0.0, &f).unwrap();
    g.set_parameter(text, 2, 0.0, &f).unwrap();
    let original = render(r.as_mut(), &g, out, 0.0);
    assert!(
        original
            .as_chunks::<4>()
            .0
            .iter()
            .any(|p| p[3] > 0 && p[1] > 0)
    );
    assert!(original.as_chunks::<4>().0.iter().any(|p| p[3] == 0));
    assert!(
        original
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| p[0] == 0 && p[2] == 0)
    );
    s.properties.insert("content.text".into(), "Changed".into());
    g.set_settings(text, s, &f).unwrap();
    assert_ne!(original, render(r.as_mut(), &g, out, 0.0));
}
