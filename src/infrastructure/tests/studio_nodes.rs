use std::sync::Arc;
use vjay_application::{
    compiler::{compile, compile_with_previews},
    media::{FrameDelivery, MediaAsset},
    ports::*,
};
use vjay_domain::{
    builtins::{ids, registry},
    graph::{Connection, Graph, NodeId},
    node::AssetId,
    uniform::ShaderUniform,
};
use vjay_infrastructure::{
    gpu::WgpuRuntimeFactory, images::RasterImageLoader, media::FfmpegDecoderFactory,
};
fn runtime() -> WgpuRuntimeFactory {
    WgpuRuntimeFactory::headless(
        Arc::new(FfmpegDecoderFactory::new("ffmpeg".into(), "ffprobe".into())),
        vjay_infrastructure::gpu::programs::builtin_programs(),
    )
    .unwrap()
    .with_images(Arc::new(RasterImageLoader))
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
fn solid(g: &mut Graph, rgb: [f32; 3]) -> NodeId {
    let f = registry();
    let id = g.add(ids::SOLID, &f).unwrap();
    for (p, v) in rgb.into_iter().enumerate() {
        g.set_parameter(id, p, v, &f).unwrap();
    }
    id
}
fn render(
    renderer: &mut dyn RenderBackend,
    g: &Graph,
    out: Option<NodeId>,
    previews: bool,
) -> Vec<u8> {
    let plan = if previews {
        compile_with_previews(g, out, &registry())
    } else {
        compile(g, out, &registry())
    }
    .unwrap();
    let extent = Extent::new(64, 40).unwrap();
    renderer.prepare(&plan, extent).unwrap();
    renderer
        .render(
            &plan,
            FrameContext {
                seconds: 0.0,
                timeline_seconds: 0.0,
                extent,
                delivery: FrameDelivery::Exact,
            },
        )
        .unwrap();
    renderer.capture().unwrap().rgba
}
fn pixel(data: &[u8], x: usize, y: usize) -> [u8; 4] {
    data[(y * 64 + x) * 4..(y * 64 + x) * 4 + 4]
        .try_into()
        .unwrap()
}
#[test]
#[ignore = "requires a native GPU"]
fn compositing_selector_grid_key_and_preview_only_program_have_expected_pixels() {
    let f = registry();
    let mut renderer = runtime().create_renderer().unwrap();
    let mut g = Graph::default();
    let red = solid(&mut g, [1.0, 0.0, 0.0]);
    let blue = solid(&mut g, [0.0, 0.0, 1.0]);
    let green = solid(&mut g, [0.0, 1.0, 0.0]);
    let white = solid(&mut g, [1.0, 1.0, 1.0]);
    let out = g.add(ids::OUTPUT, &f).unwrap();
    let composite = g.add("Composite", &f).unwrap();
    wire(&mut g, red, composite, 0);
    wire(&mut g, blue, composite, 1);
    wire(&mut g, composite, out, 0);
    g.set_parameter(composite, 2, 0.5, &f).unwrap();
    g.set_parameter(composite, 3, 0.5, &f).unwrap();
    g.set_parameter(composite, 5, 0.5, &f).unwrap();
    let data = render(renderer.as_mut(), &g, Some(out), false);
    assert_eq!(pixel(&data, 0, 0), [255, 0, 0, 255]);
    let center = pixel(&data, 32, 20);
    assert!((center[0] as i16 - 128).abs() <= 1 && (center[2] as i16 - 128).abs() <= 1);
    let selector = g.add("ImageSelector", &f).unwrap();
    wire(&mut g, red, selector, 0);
    wire(&mut g, blue, selector, 7);
    g.set_parameter(selector, 0, 7.0, &f).unwrap();
    wire(&mut g, selector, out, 0);
    assert_eq!(
        pixel(&render(renderer.as_mut(), &g, Some(out), false), 32, 20),
        [0, 0, 255, 255]
    );
    let grid = g.add("Grid", &f).unwrap();
    g.set_parameter(grid, 0, 4.0, &f).unwrap();
    g.set_parameter(grid, 1, 2.0, &f).unwrap();
    g.set_parameter(grid, 2, 0.0, &f).unwrap();
    for (i, node) in [red, blue, green, white, white, blue, red, green]
        .into_iter()
        .enumerate()
    {
        wire(&mut g, node, grid, i);
    }
    wire(&mut g, grid, out, 0);
    let data = render(renderer.as_mut(), &g, Some(out), true);
    assert_eq!(pixel(&data, 8, 10), [255, 0, 0, 255]);
    assert_eq!(pixel(&data, 24, 10), [0, 0, 255, 255]);
    assert_eq!(pixel(&data, 56, 30), [0, 255, 0, 255]);
    let key = g.add("ChromaKey", &f).unwrap();
    wire(&mut g, green, key, 0);
    wire(&mut g, key, composite, 1);
    g.set_parameter(composite, 5, 1.0, &f).unwrap();
    wire(&mut g, composite, out, 0);
    assert_eq!(
        pixel(&render(renderer.as_mut(), &g, Some(out), false), 32, 20),
        [255, 0, 0, 255]
    );
    let data = render(renderer.as_mut(), &g, None, true);
    assert!(data.as_chunks::<4>().0.iter().all(|p| *p == [0, 0, 0, 255]));
}
#[test]
#[ignore = "requires a native GPU"]
fn named_uniforms_beyond_four_and_transparent_png_are_uploaded_correctly() {
    let f = registry();
    let mut renderer = runtime().create_renderer().unwrap();
    let mut g = Graph::default();
    let shader = g.add(ids::GLSL, &f).unwrap();
    let out = g.add(ids::OUTPUT, &f).unwrap();
    let mut settings = g.node(shader).unwrap().settings.clone();
    settings.shader_uniforms = Some(
        (0..64)
            .map(|i| ShaderUniform {
                name: format!("u_{i}"),
                min: 0.0,
                max: 1.0,
                default: 0.0,
            })
            .collect(),
    );
    settings.shader =
        "uniform float u_63;\nvoid mainImage(out vec4 c,in vec2 p){c=vec4(u_0,u_31,u_63,1.0);}"
            .into();
    g.set_settings(shader, settings, &f).unwrap();
    for (p, v) in [(0, 0.25), (31, 0.5), (63, 0.75)] {
        g.set_parameter(shader, p, v, &f).unwrap();
    }
    wire(&mut g, shader, out, 0);
    let result = pixel(&render(renderer.as_mut(), &g, Some(out), false), 32, 20);
    assert_eq!(result, [64, 128, 191, 255]);
    let value = g.add(ids::CONSTANT, &f).unwrap();
    g.set_parameter(value, 0, 0.5, &f).unwrap();
    for input in [2, 65] {
        g.connect(Connection {
            from: value,
            output: 0,
            to: shader,
            input,
        })
        .unwrap();
    }
    let mut signals = vjay_application::signals::SignalEngine::default();
    signals.rebuild(&g, &f).unwrap();
    signals.tick(0.0);
    let mut plan = compile(&g, out, &f).unwrap();
    signals.bind(&mut plan);
    signals.modulate(&mut plan);
    renderer
        .render(
            &plan,
            FrameContext {
                seconds: 0.0,
                timeline_seconds: 0.0,
                extent: Extent::new(64, 40).unwrap(),
                delivery: FrameDelivery::Exact,
            },
        )
        .unwrap();
    assert_eq!(
        pixel(&renderer.capture().unwrap().rgba, 32, 20),
        [128, 128, 128, 255]
    );

    let path = std::env::temp_dir().join(format!("vjay-alpha-{}.png", std::process::id()));
    image::RgbaImage::from_pixel(64, 40, image::Rgba([0, 255, 0, 128]))
        .save(&path)
        .unwrap();
    let photo = g.add("Image", &f).unwrap();
    g.bind_asset(photo, AssetId(7), &f).unwrap();
    renderer.set_assets(&[MediaAsset {
        id: AssetId(7),
        path: path.clone(),
    }]);
    wire(&mut g, photo, out, 0);
    assert_eq!(
        pixel(&render(renderer.as_mut(), &g, Some(out), false), 32, 20),
        [0, 255, 0, 128]
    );
    let replacement =
        path.with_file_name(format!("vjay-alpha-replacement-{}.png", std::process::id()));
    image::RgbaImage::from_pixel(64, 40, image::Rgba([255, 0, 0, 255]))
        .save(&replacement)
        .unwrap();
    renderer.set_assets(&[MediaAsset {
        id: AssetId(7),
        path: replacement.clone(),
    }]);
    assert_eq!(
        pixel(&render(renderer.as_mut(), &g, Some(out), false), 32, 20),
        [255, 0, 0, 255]
    );
    drop(renderer);
    std::fs::remove_file(replacement).unwrap();
    std::fs::remove_file(path).unwrap();
}
#[test]
#[ignore = "requires a native GPU"]
fn all_added_visual_programs_validate_and_render() {
    let f = registry();
    let mut renderer = runtime().create_renderer().unwrap();
    for name in [
        "Composite",
        "ImageSelector",
        "Grid",
        "ChromaKey",
        "Mask",
        "Wipe",
        "Crop",
        "Mirror",
        "Displace",
        "Levels",
        "Threshold",
        "Vignette",
    ] {
        let mut g = Graph::default();
        let source = solid(&mut g, [0.2, 0.4, 0.6]);
        let node = g.add(name, &f).unwrap();
        let out = g.add(ids::OUTPUT, &f).unwrap();
        for input in 0..g.schema(node).unwrap().inputs.len() {
            wire(&mut g, source, node, input);
        }
        wire(&mut g, node, out, 0);
        let data = render(renderer.as_mut(), &g, Some(out), false);
        assert_eq!(data.len(), 64 * 40 * 4);
    }
}

#[test]
#[ignore = "requires a native GPU"]
fn failed_disconnected_media_previews_do_not_stop_a_valid_program() {
    let f = registry();
    let mut renderer = runtime().create_renderer().unwrap();
    let mut g = Graph::default();
    let red = solid(&mut g, [1.0, 0.0, 0.0]);
    let out = g.add(ids::OUTPUT, &f).unwrap();
    wire(&mut g, red, out, 0);
    let video = g.add(ids::VIDEO, &f).unwrap();
    g.bind_asset(video, AssetId(10), &f).unwrap();
    let photo = g.add("Image", &f).unwrap();
    g.bind_asset(photo, AssetId(11), &f).unwrap();
    renderer.set_assets(&[
        MediaAsset {
            id: AssetId(10),
            path: "/missing/vjay-test.mp4".into(),
        },
        MediaAsset {
            id: AssetId(11),
            path: "/missing/vjay-test.png".into(),
        },
    ]);
    assert_eq!(
        pixel(&render(renderer.as_mut(), &g, Some(out), true), 32, 20),
        [255, 0, 0, 255]
    );
    assert!(renderer.node_error(video).is_some());
    assert!(renderer.node_error(photo).is_some());
    wire(&mut g, video, out, 0);
    let plan = compile_with_previews(&g, Some(out), &f).unwrap();
    assert!(
        renderer
            .prepare(&plan, Extent::new(64, 40).unwrap())
            .is_err()
    );
}
