use std::sync::Arc;
use vjay_application::{compiler::compile_with_previews, media::FrameDelivery, ports::*};
use vjay_domain::{
    builtins::{ids, registry},
    graph::{Connection, Graph},
};
use vjay_infrastructure::{gpu::WgpuRuntimeFactory, media::FfmpegDecoderFactory};
#[test]
#[ignore = "requires a native GPU"]
fn independent_outputs_keep_their_textures_and_shared_branches_render_once() {
    let nodes = registry();
    let mut g = Graph::default();
    let red = g.add(ids::SOLID, &nodes).unwrap();
    let blue = g.add(ids::SOLID, &nodes).unwrap();
    for (p, v) in [(0, 1.0), (1, 0.0), (2, 0.0)] {
        g.set_parameter(red, p, v, &nodes).unwrap();
    }
    for (p, v) in [(0, 0.0), (1, 0.0), (2, 1.0)] {
        g.set_parameter(blue, p, v, &nodes).unwrap();
    }
    let left = g.add(ids::OUTPUT, &nodes).unwrap();
    let right = g.add(ids::OUTPUT, &nodes).unwrap();
    let duplicate = g.add(ids::OUTPUT, &nodes).unwrap();
    for (source, target) in [(red, left), (blue, right), (red, duplicate)] {
        g.connect(Connection {
            from: source,
            output: 0,
            to: target,
            input: 0,
        })
        .unwrap();
    }
    let plan = compile_with_previews(&g, Some(left), &nodes).unwrap();
    assert_eq!(plan.steps.len(), 2);
    assert_eq!(plan.outputs.len(), 3);
    assert_ne!(plan.outputs[0].slot, plan.outputs[1].slot);
    assert_eq!(plan.outputs[0].slot, plan.outputs[2].slot);
    // An unavailable network sender must leave independent screen outputs usable.
    let ndi = g.add("NdiOut", &nodes).unwrap();
    let mut settings = g.node(ndi).unwrap().settings.clone();
    settings.properties.insert("enabled".into(), "On".into());
    g.set_settings(ndi, settings, &nodes).unwrap();
    g.connect(Connection {
        from: red,
        output: 0,
        to: ndi,
        input: 0,
    })
    .unwrap();
    let plan = compile_with_previews(&g, Some(left), &nodes).unwrap();
    let runtime = WgpuRuntimeFactory::headless(
        Arc::new(FfmpegDecoderFactory::new("ffmpeg".into(), "ffprobe".into())),
        vjay_infrastructure::gpu::programs::builtin_programs(),
    )
    .unwrap();
    let mut renderer = runtime.create_renderer().unwrap();
    let extent = Extent::new(64, 40).unwrap();
    renderer.prepare(&plan, extent).unwrap();
    assert!(renderer.node_error(ndi).is_some());
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
    for (out, pixel) in [
        (left, [255, 0, 0, 255]),
        (right, [0, 0, 255, 255]),
        (duplicate, [255, 0, 0, 255]),
    ] {
        assert!(
            renderer
                .capture_output(out)
                .unwrap()
                .rgba
                .as_chunks::<4>()
                .0
                .iter()
                .all(|p| *p == pixel)
        );
    }
}
