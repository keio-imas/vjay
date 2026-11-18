use std::sync::Arc;
use vjay_application::{compiler::compile, media::FrameDelivery, ports::*};
use vjay_domain::builtins::ids;
use vjay_domain::{
    builtins::registry as builtin_registry,
    graph::{Connection, Graph, NodeId},
    node::NodeFactory,
    settings::NodeSettings,
};
use vjay_infrastructure::{gpu::WgpuRuntimeFactory, media::FfmpegDecoderFactory};
fn wire(g: &mut Graph, from: NodeId, port: usize, to: NodeId, input: usize) {
    g.connect(Connection {
        from,
        output: port,
        to,
        input,
    })
    .unwrap();
}
fn render(renderer: &mut dyn RenderBackend, g: &Graph, out: NodeId) -> Vec<u8> {
    let plan = compile(g, out, &builtin_registry()).unwrap();
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
#[test]
#[ignore = "requires a native GPU"]
fn custom_glsl_and_multiple_texture_outputs_produce_expected_pixels() {
    let f = builtin_registry();
    let runtime = WgpuRuntimeFactory::headless(
        Arc::new(FfmpegDecoderFactory::new("ffmpeg".into(), "ffprobe".into())),
        vjay_infrastructure::gpu::programs::builtin_programs(),
    )
    .unwrap();
    let mut renderer = runtime.create_renderer().unwrap();
    let mut g = Graph::default();
    let shader = g.add(ids::GLSL, &f).unwrap();
    let split = g.add(ids::RGB_SPLIT, &f).unwrap();
    let mix = g.add(ids::MIX, &f).unwrap();
    let out = g.add(ids::OUTPUT, &f).unwrap();
    g.set_settings(
        shader,
        NodeSettings {
            shader: "void mainImage(out vec4 c,in vec2 p){c=vec4(0.2,0.4,0.8,1.0);}".into(),
            ..Default::default()
        },
        &f,
    )
    .unwrap();
    wire(&mut g, shader, 0, split, 0);
    wire(&mut g, split, 0, mix, 0);
    wire(&mut g, split, 2, mix, 1);
    wire(&mut g, mix, 0, out, 0);
    let plan = compile(&g, out, &f).unwrap();
    assert_eq!(plan.steps.iter().filter(|s| s.node == split).count(), 2);
    let bytes = render(renderer.as_mut(), &g, out);
    assert!(
        bytes
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| p[..3].iter().all(|&v| (v as i16 - 128).abs() <= 1) && p[3] == 255)
    );
    // Disconnected Program renders black and never reads a stale pooled texture.
    g.disconnect(out, 0);
    let bytes = render(renderer.as_mut(), &g, out);
    assert!(
        bytes
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| *p == [0, 0, 0, 255])
    );
    // Naga reports invalid edits before committing a new pipeline.
    assert!(
        renderer
            .validate_shader("void mainImage(out vec4 c,in vec2 p){c=broken;}")
            .is_err()
    );
}
#[test]
#[ignore = "requires a native GPU"]
fn all_new_texture_nodes_render_and_glsl_samples_connected_input() {
    let f = builtin_registry();
    let runtime = WgpuRuntimeFactory::headless(
        Arc::new(FfmpegDecoderFactory::new("ffmpeg".into(), "ffprobe".into())),
        vjay_infrastructure::gpu::programs::builtin_programs(),
    )
    .unwrap();
    let mut renderer = runtime.create_renderer().unwrap();
    for kind in [
        ids::NOISE,
        ids::CHECKERBOARD,
        ids::RAMP,
        ids::INVERT,
        ids::POSTERIZE,
        ids::PIXELATE,
        ids::EDGE,
        ids::GLSL,
    ] {
        let mut g = Graph::default();
        let pattern = g.add(ids::PATTERN, &f).unwrap();
        let effect = g.add(kind, &f).unwrap();
        let out = g.add(ids::OUTPUT, &f).unwrap();
        for (input, _) in f.get(kind).unwrap().descriptor().inputs.iter().enumerate() {
            wire(&mut g, pattern, 0, effect, input);
        }
        if kind == ids::GLSL {
            g.set_settings(effect,NodeSettings{shader:"void mainImage(out vec4 c,in vec2 p){c=texture(iChannel0,p/iResolution.xy);}".into(),..Default::default()}, &f).unwrap();
        }
        wire(&mut g, effect, 0, out, 0);
        let bytes = render(renderer.as_mut(), &g, out);
        assert_eq!(bytes.len(), 64 * 40 * 4);
        assert!(
            bytes.as_chunks::<4>().0.iter().any(|p| p[..3] != [0, 0, 0]),
            "{kind:?} produced only black"
        );
    }
}
