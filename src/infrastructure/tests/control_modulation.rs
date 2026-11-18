use std::{cell::Cell, rc::Rc, sync::Arc};
use vjay_application::{ports::*, session::Session};
use vjay_domain::{
    builtins::{ids, registry},
    graph::{Connection, NodeId},
};
use vjay_infrastructure::{
    gpu::WgpuRuntimeFactory, media::FfmpegDecoderFactory, storage::JsonProjectStore,
};

struct LiveClock(Rc<Cell<f64>>);
impl Clock for LiveClock {
    fn seconds(&self) -> f64 {
        self.0.get()
    }
}
struct LiveRuntime {
    gpu: WgpuRuntimeFactory,
    now: Rc<Cell<f64>>,
}
impl RuntimeFactory for LiveRuntime {
    fn create_renderer(&self) -> PortResult<Box<dyn RenderBackend>> {
        self.gpu.create_renderer()
    }
    fn create_clock(&self) -> Box<dyn Clock> {
        Box::new(LiveClock(self.now.clone()))
    }
}
fn rotation(s: &Session, effect: NodeId) -> f32 {
    s.plan()
        .unwrap()
        .steps
        .iter()
        .find(|step| step.node == effect)
        .unwrap()
        .operation
        .parameters[1]
}

#[test]
#[ignore = "requires a native GPU"]
fn paused_checkerboard_rotates_on_button_trigger_then_returns_to_its_original_pixels() {
    let now = Rc::new(Cell::new(0.0));
    let gpu = WgpuRuntimeFactory::headless(
        Arc::new(FfmpegDecoderFactory::new("ffmpeg".into(), "ffprobe".into())),
        vjay_infrastructure::gpu::programs::builtin_programs(),
    )
    .unwrap();
    let runtime = LiveRuntime {
        gpu,
        now: now.clone(),
    };
    let mut s = Session::new(
        Box::new(registry()),
        &runtime,
        Box::new(JsonProjectStore::new(
            "/tmp/vjay-gpu-control-unused.json".into(),
        )),
        Extent::new(128, 80).unwrap(),
    )
    .unwrap();
    let out = s.project().output.unwrap();
    let mut add = |kind| {
        s.add_node(kind, vjay_application::project::Point { x: 0.0, y: 0.0 })
            .unwrap()
    };
    let checkerboard = add(ids::CHECKERBOARD);
    let effect = add(ids::KALEIDOSCOPE);
    let blur = add(ids::BLUR);
    let button = add(ids::BUTTON);
    let envelope = add("Envelope");
    let multiply = add(ids::MULTIPLY);
    s.set_parameter(envelope, 0, 0.001).unwrap();
    s.set_parameter(envelope, 1, 0.5).unwrap();
    s.set_parameter(multiply, 1, 5.0).unwrap();
    s.set_parameter(blur, 0, 6.12).unwrap();
    for (from, output, to, input) in [
        (checkerboard, 0, effect, 0),
        (effect, 0, blur, 0),
        (blur, 0, out, 0),
        (button, 1, envelope, 0),
        (envelope, 0, multiply, 0),
        (multiply, 0, effect, 2),
    ] {
        s.connect(Connection {
            from,
            output,
            to,
            input,
        })
        .unwrap();
    }
    s.paused = true;
    s.tick().unwrap();
    let original = s.capture().unwrap().rgba;
    s.set_parameter(button, 0, 1.0).unwrap();
    s.set_parameter(button, 0, 0.0).unwrap();
    now.set(0.016);
    s.tick().unwrap();
    assert_eq!(rotation(&s, effect), 5.0);
    let peak = s.capture().unwrap().rgba;
    assert_ne!(
        peak, original,
        "GPU output must rotate, not only its parameter label"
    );
    now.set(0.266);
    s.tick().unwrap();
    assert!((rotation(&s, effect) - 2.5).abs() < 1e-5);
    let halfway = s.capture().unwrap().rgba;
    assert_ne!(halfway, original);
    assert_ne!(halfway, peak);
    now.set(0.516);
    s.tick().unwrap();
    assert_eq!(rotation(&s, effect), 0.0);
    assert_eq!(s.capture().unwrap().rgba, original);
    assert_eq!(s.seconds(), 0.0);
    assert!(s.paused);
}
