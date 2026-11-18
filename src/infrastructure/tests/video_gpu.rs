use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use vjay_application::{compiler::compile, media::*, ports::*};
use vjay_domain::builtins::ids;
use vjay_domain::{
    builtins::registry as builtin_registry,
    graph::{Connection, Graph},
    node::AssetId,
};
use vjay_infrastructure::gpu::WgpuRuntimeFactory;

struct FakeFactory(Arc<AtomicUsize>);
struct FakeDecoder(Extent);
impl VideoDecoderFactory for FakeFactory {
    fn open(&self, _: &Path, extent: Extent) -> PortResult<Box<dyn VideoDecoder>> {
        self.0.fetch_add(1, Ordering::Relaxed);
        Ok(Box::new(FakeDecoder(extent)))
    }
}
impl VideoDecoder for FakeDecoder {
    fn duration(&self) -> f64 {
        2.0
    }
    fn frame_at(&mut self, seconds: f64, _: FrameDelivery) -> PortResult<Option<VideoFrame>> {
        let pixel = [(seconds * 100.0).round() as u8, 32, 64, 255];
        Ok(Some(VideoFrame {
            rgba: pixel.repeat((self.0.width * self.0.height) as usize),
            seconds,
        }))
    }
    fn recycle(&mut self, _: VideoFrame) {}
}

#[test]
#[ignore = "requires a native GPU"]
fn video_upload_pause_and_topology_reuse() {
    let opens = Arc::new(AtomicUsize::new(0));
    let runtime = WgpuRuntimeFactory::headless(
        Arc::new(FakeFactory(opens.clone())),
        vjay_infrastructure::gpu::programs::builtin_programs(),
    )
    .unwrap();
    let mut renderer = runtime.create_renderer().unwrap();
    let f = builtin_registry();
    let mut graph = Graph::default();
    let video = graph.add(ids::VIDEO, &f).unwrap();
    graph.bind_asset(video, AssetId(0), &f).unwrap();
    let output = graph.add(ids::OUTPUT, &f).unwrap();
    graph
        .connect(Connection {
            output: 0,
            from: video,
            to: output,
            input: 0,
        })
        .unwrap();
    renderer.set_assets(&[MediaAsset {
        id: AssetId(0),
        path: "/fake.mp4".into(),
    }]);
    let extent = Extent::new(17, 11).unwrap();
    let plan = compile(&graph, output, &f).unwrap();
    renderer.prepare(&plan, extent).unwrap();
    let frame = |seconds| FrameContext {
        seconds: seconds as f32,
        timeline_seconds: seconds,
        extent,
        delivery: FrameDelivery::Exact,
    };
    renderer.render(&plan, frame(0.0)).unwrap();
    renderer.render(&plan, frame(0.5)).unwrap();
    assert_eq!(renderer.capture().unwrap().rgba[0], 50);
    graph.set_parameter(video, 0, 0.0, &f).unwrap();
    let paused = compile(&graph, output, &f).unwrap();
    renderer.render(&paused, frame(1.0)).unwrap();
    assert_eq!(renderer.capture().unwrap().rgba[0], 50);
    graph.set_parameter(video, 0, 1.0, &f).unwrap();
    let grade = graph.add(ids::GRADE, &f).unwrap();
    graph
        .connect(Connection {
            output: 0,
            from: video,
            to: grade,
            input: 0,
        })
        .unwrap();
    graph
        .connect(Connection {
            output: 0,
            from: grade,
            to: output,
            input: 0,
        })
        .unwrap();
    let graded = compile(&graph, output, &f).unwrap();
    renderer.prepare(&graded, extent).unwrap();
    assert_eq!(
        opens.load(Ordering::Relaxed),
        1,
        "topology edits must not restart video playback"
    );
    renderer.render(&graded, frame(1.5)).unwrap();
    assert_eq!(renderer.capture().unwrap().rgba[0], 100);
}

#[test]
#[ignore = "requires a native GPU"]
fn large_rotation_and_playback_range_keep_the_video_decoder() {
    let opens = Arc::new(AtomicUsize::new(0));
    let runtime = WgpuRuntimeFactory::headless(
        Arc::new(FakeFactory(opens.clone())),
        vjay_infrastructure::gpu::programs::builtin_programs(),
    )
    .unwrap();
    let mut renderer = runtime.create_renderer().unwrap();
    let f = builtin_registry();
    let mut graph = Graph::default();
    let video = graph.add(ids::VIDEO, &f).unwrap();
    graph.bind_asset(video, AssetId(0), &f).unwrap();
    let transform = graph.add(ids::TRANSFORM, &f).unwrap();
    let output = graph.add(ids::OUTPUT, &f).unwrap();
    for (from, to) in [(video, transform), (transform, output)] {
        graph
            .connect(Connection {
                from,
                to,
                output: 0,
                input: 0,
            })
            .unwrap();
    }
    renderer.set_assets(&[MediaAsset {
        id: AssetId(0),
        path: "/fake.mp4".into(),
    }]);
    let extent = Extent::new(32, 32).unwrap();
    graph.set_parameter(video, 3, 0.25, &f).unwrap();
    graph.set_parameter(video, 4, 0.75, &f).unwrap();
    let render = |renderer: &mut Box<dyn RenderBackend>, graph: &Graph, base: f64| {
        let mut plan = compile(graph, output, &f).unwrap();
        plan.steps[0].media_time = Some(base);
        renderer.prepare(&plan, extent).unwrap();
        renderer
            .render(
                &plan,
                FrameContext {
                    seconds: base as f32,
                    timeline_seconds: base,
                    extent,
                    delivery: FrameDelivery::Exact,
                },
            )
            .unwrap();
        renderer.capture().unwrap().rgba[0]
    };
    assert_eq!(render(&mut renderer, &graph, 0.0), 50);
    graph.set_parameter(transform, 3, 36_000_000.0, &f).unwrap();
    assert_eq!(render(&mut renderer, &graph, 1.5), 100);
    graph.set_parameter(video, 2, 0.0, &f).unwrap();
    assert_eq!(render(&mut renderer, &graph, 1.5), 150);
    assert_eq!(opens.load(Ordering::Relaxed), 1);
}
