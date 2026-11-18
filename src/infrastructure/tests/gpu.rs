use vjay_application::{compiler::compile, ports::*};
use vjay_domain::{builtins::registry as builtin_registry, graph::NodeId};
use vjay_infrastructure::gpu::WgpuRuntimeFactory;

#[test]
#[ignore = "requires a native WebGPU adapter; run cargo test -p vjay-infrastructure --test gpu -- --ignored"]
fn webgpu_graph_readback() {
    let f = builtin_registry();
    let media = std::sync::Arc::new(vjay_infrastructure::media::FfmpegDecoderFactory::new(
        "ffmpeg".into(),
        "ffprobe".into(),
    ));
    let runtime = WgpuRuntimeFactory::headless(
        media,
        vjay_infrastructure::gpu::programs::builtin_programs(),
    )
    .expect("GPU adapter");
    let mut renderer = runtime.create_renderer().unwrap();
    let mut project = vjay_infrastructure::demo::project(&f).unwrap();
    let mut extent = Extent::new(65, 37).unwrap();
    let plan = compile(&project.graph, project.output, &f).unwrap();
    renderer.prepare(&plan, extent).unwrap();
    renderer
        .render(
            &plan,
            FrameContext {
                seconds: 0.0,
                timeline_seconds: 0.0,
                extent,
                delivery: vjay_application::media::FrameDelivery::Exact,
            },
        )
        .unwrap();
    let first = renderer.capture().unwrap();
    assert_eq!(first.rgba.len(), 65 * 37 * 4);
    assert!(first.rgba.as_chunks::<4>().0.iter().all(|p| p[3] == 255));
    assert!(first.rgba.as_chunks::<4>().0.iter().any(|p| p[0] != p[1]));
    renderer
        .render(
            &plan,
            FrameContext {
                seconds: 0.75,
                timeline_seconds: 0.75,
                extent,
                delivery: vjay_application::media::FrameDelivery::Exact,
            },
        )
        .unwrap();
    assert_ne!(first.rgba, renderer.capture().unwrap().rgba);

    project.graph.set_parameter(NodeId(3), 2, 0.0, &f).unwrap();
    let black = compile(&project.graph, project.output, &f).unwrap();
    // Only uniforms change: the existing bind groups and textures are reused.
    renderer
        .render(
            &black,
            FrameContext {
                seconds: 1.0,
                timeline_seconds: 1.0,
                extent,
                delivery: vjay_application::media::FrameDelivery::Exact,
            },
        )
        .unwrap();
    assert!(
        renderer
            .capture()
            .unwrap()
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| *p == [0, 0, 0, 255])
    );
    extent = Extent::new(127, 51).unwrap();
    renderer.prepare(&black, extent).unwrap();
    renderer
        .render(
            &black,
            FrameContext {
                seconds: 1.0,
                timeline_seconds: 1.0,
                extent,
                delivery: vjay_application::media::FrameDelivery::Exact,
            },
        )
        .unwrap();
    assert_eq!(renderer.capture().unwrap().rgba.len(), 127 * 51 * 4);
}
