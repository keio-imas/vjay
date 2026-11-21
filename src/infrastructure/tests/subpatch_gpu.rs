use std::sync::Arc;
use vjay_application::{ports::*, session::Session};
use vjay_domain::{builtins::registry, graph::NodeId};
use vjay_infrastructure::{
    gpu::WgpuRuntimeFactory, media::FfmpegDecoderFactory, storage::JsonProjectStore,
};
#[test]
#[ignore = "requires a native GPU; compares nested expansion, live internal edits and alias previews"]
fn nested_subpatches_render_the_same_pixels_without_adding_processing_passes() {
    let runtime = WgpuRuntimeFactory::headless(
        Arc::new(FfmpegDecoderFactory::new("ffmpeg".into(), "ffprobe".into())),
        vjay_infrastructure::gpu::programs::builtin_programs(),
    )
    .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let mut s = Session::new(
        Box::new(registry()),
        &runtime,
        Box::new(JsonProjectStore::new(temp.path().join("scope.json"))),
        Extent::new(96, 64).unwrap(),
    )
    .unwrap();
    s.restore(
        vjay_infrastructure::demo::project(&registry())
            .unwrap()
            .document(),
    )
    .unwrap();
    s.render_at(0.5).unwrap();
    let before = s.capture().unwrap().rgba;
    let passes = s.plan().unwrap().steps.len();
    let patch = s.encapsulate(&[NodeId(2), NodeId(3)]).unwrap();
    s.render_at(0.5).unwrap();
    assert_eq!(s.capture().unwrap().rgba, before);
    assert_eq!(s.plan().unwrap().steps.len(), passes);
    assert!(s.plan().unwrap().previews.iter().any(|p| p.node == patch));
    s.enter_subpatch(patch).unwrap();
    let nested = s.encapsulate(&[NodeId(2)]).unwrap();
    s.enter_subpatch(nested).unwrap();
    s.render_at(0.5).unwrap();
    assert_eq!(s.capture().unwrap().rgba, before);
    s.set_parameter(NodeId(2), 0, 0.9).unwrap();
    s.render_at(0.5).unwrap();
    assert_ne!(s.capture().unwrap().rgba, before);
    s.save().unwrap();
    s.leave_to(0).unwrap();
    s.load().unwrap();
    s.render_at(0.5).unwrap();
    assert_eq!(s.plan().unwrap().steps.len(), passes);
}
