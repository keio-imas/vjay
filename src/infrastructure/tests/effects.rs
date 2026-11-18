use std::sync::Arc;
use vjay_application::{compiler::compile, media::FrameDelivery, ports::*};
use vjay_domain::builtins::ids;
use vjay_domain::{
    builtins::registry as builtin_registry,
    graph::{Connection, Graph, NodeId},
    node::NodeFactory,
};
use vjay_infrastructure::{gpu::WgpuRuntimeFactory, media::FfmpegDecoderFactory};

fn add_solid(graph: &mut Graph, rgb: [f32; 3]) -> NodeId {
    let factory = builtin_registry();
    let node = graph.add(ids::SOLID, &factory).unwrap();
    for (i, value) in rgb.into_iter().enumerate() {
        graph.set_parameter(node, i, value, &factory).unwrap();
    }
    node
}
fn render(
    renderer: &mut dyn RenderBackend,
    kind: &str,
    parameters: &[(usize, f32)],
    seconds: f32,
) -> [u8; 4] {
    let f = builtin_registry();
    let mut graph = Graph::default();
    let a = add_solid(&mut graph, [0.8, 0.4, 0.2]);
    let effect = graph.add(kind, &f).unwrap();
    for &(parameter, value) in parameters {
        graph.set_parameter(effect, parameter, value, &f).unwrap();
    }
    for input in 0..f.get(kind).unwrap().descriptor().inputs.len() {
        let from = if input == 0 {
            a
        } else {
            add_solid(&mut graph, [0.2, 0.6, 0.8])
        };
        graph
            .connect(Connection {
                output: 0,
                from,
                to: effect,
                input,
            })
            .unwrap();
    }
    let output = graph.add(ids::OUTPUT, &f).unwrap();
    graph
        .connect(Connection {
            output: 0,
            from: effect,
            to: output,
            input: 0,
        })
        .unwrap();
    let plan = compile(&graph, output, &f).unwrap();
    let extent = Extent::new(31, 19).unwrap();
    renderer.prepare(&plan, extent).unwrap();
    renderer
        .render(
            &plan,
            FrameContext {
                seconds,
                timeline_seconds: seconds as f64,
                extent,
                delivery: FrameDelivery::Exact,
            },
        )
        .unwrap();
    renderer.capture().unwrap().rgba[..4].try_into().unwrap()
}
fn close(actual: [u8; 4], expected: [f32; 4]) {
    for (a, b) in actual.into_iter().zip(expected) {
        assert!(
            (a as f32 / 255.0 - b).abs() < 0.01,
            "{actual:?} != {expected:?}"
        );
    }
}

#[test]
#[ignore = "requires a native GPU"]
fn effects_and_blend_modes_have_expected_pixel_results() {
    let runtime = WgpuRuntimeFactory::headless(
        Arc::new(FfmpegDecoderFactory::new("ffmpeg".into(), "ffprobe".into())),
        vjay_infrastructure::gpu::programs::builtin_programs(),
    )
    .unwrap();
    let mut renderer = runtime.create_renderer().unwrap();
    for kind in [
        ids::TRANSFORM,
        ids::GRADE,
        ids::BLUR,
        ids::KALEIDOSCOPE,
        ids::LUMA_KEY,
    ] {
        close(
            render(renderer.as_mut(), kind, &[], 0.0),
            [0.8, 0.4, 0.2, 1.0],
        );
    }
    close(
        render(renderer.as_mut(), ids::STROBE, &[], 0.0),
        [0.8, 0.4, 0.2, 1.0],
    );
    close(
        render(renderer.as_mut(), ids::STROBE, &[], 0.2),
        [0.0, 0.0, 0.0, 1.0],
    );
    close(
        render(renderer.as_mut(), ids::LUMA_KEY, &[(0, 1.0)], 0.0),
        [0.8, 0.4, 0.2, 0.0],
    );
    close(
        render(renderer.as_mut(), ids::GRADE, &[(0, 0.1)], 0.0),
        [0.9, 0.5, 0.3, 1.0],
    );
    close(
        render(renderer.as_mut(), ids::BLEND, &[(1, 0.0)], 0.0),
        [0.2, 0.6, 0.8, 1.0],
    );
    close(
        render(renderer.as_mut(), ids::BLEND, &[(1, 1.0)], 0.0),
        [1.0, 1.0, 1.0, 1.0],
    );
    close(
        render(renderer.as_mut(), ids::BLEND, &[(1, 2.0)], 0.0),
        [0.16, 0.24, 0.16, 1.0],
    );
    close(
        render(renderer.as_mut(), ids::BLEND, &[(1, 3.0)], 0.0),
        [0.84, 0.76, 0.84, 1.0],
    );
    close(
        render(renderer.as_mut(), ids::TRANSFORM, &[(0, 1.0)], 0.0),
        [0.0, 0.0, 0.0, 0.0],
    );
}
