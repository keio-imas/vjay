use super::*;
use std::cell::Cell;
use vjay_domain::graph::Connection;

fn chain() -> (Session, NodeId, NodeId) {
    chain_with_runtime(&Runtime(Rc::new(RefCell::new(Counts::default()))))
}
fn chain_with_runtime(runtime: &dyn RuntimeFactory) -> (Session, NodeId, NodeId) {
    let f = builtin_registry();
    let mut s = Session::new(
        Box::new(f),
        runtime,
        Box::new(Store {
            doc: RefCell::new(Project::empty(&builtin_registry()).unwrap().document()),
            path: "unused.json".into(),
        }),
        Extent::new(640, 360).unwrap(),
    )
    .unwrap();
    let out = s.project().output.unwrap();
    let mut add = |kind| s.add_node(kind, Point { x: 0.0, y: 0.0 }).unwrap();
    let pattern = add(ids::PATTERN);
    let effect = add(ids::KALEIDOSCOPE);
    let button = add(ids::BUTTON);
    let envelope = add("Envelope");
    let multiply = add(ids::MULTIPLY);
    s.set_parameter(envelope, 0, 0.001).unwrap();
    s.set_parameter(envelope, 1, 0.5).unwrap();
    s.set_parameter(multiply, 1, 5.0).unwrap();
    for (from, output, to, input) in [
        (pattern, 0, effect, 0),
        (effect, 0, out, 0),
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
    (s, button, effect)
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
fn short_button_click_drives_kaleidoscope_rotation_through_envelope_and_multiply() {
    let (mut s, button, effect) = chain();
    s.render_at(0.0).unwrap();
    s.set_parameter(button, 0, 1.0).unwrap();
    s.set_parameter(button, 0, 0.0).unwrap();
    s.render_at(1.0 / 60.0).unwrap();
    assert_eq!(rotation(&s, effect), 5.0);
    s.render_at(0.25 + 1.0 / 60.0).unwrap();
    assert!((rotation(&s, effect) - 2.5).abs() < 1e-5);
    s.render_at(0.5 + 1.0 / 60.0).unwrap();
    assert_eq!(rotation(&s, effect), 0.0);
}

#[test]
fn button_trigger_is_not_lost_when_the_first_evaluation_has_no_elapsed_time() {
    let (mut s, button, effect) = chain();
    s.set_parameter(button, 0, 1.0).unwrap();
    s.set_parameter(button, 0, 0.0).unwrap();
    s.render_at(0.0).unwrap();
    s.render_at(1.0 / 60.0).unwrap();
    assert!(rotation(&s, effect) > 0.0, "button trigger was lost");
}

struct ControlClock(Rc<Cell<f64>>);
impl Clock for ControlClock {
    fn seconds(&self) -> f64 {
        self.0.get()
    }
}
struct LiveRuntime(Rc<Cell<f64>>);
impl RuntimeFactory for LiveRuntime {
    fn create_renderer(&self) -> PortResult<Box<dyn RenderBackend>> {
        Runtime(Rc::new(RefCell::new(Counts::default()))).create_renderer()
    }
    fn create_clock(&self) -> Box<dyn Clock> {
        Box::new(ControlClock(self.0.clone()))
    }
}

#[test]
fn control_envelope_runs_and_retriggers_while_media_is_paused() {
    let now = Rc::new(Cell::new(0.0));
    let (mut s, button, effect) = chain_with_runtime(&LiveRuntime(now.clone()));
    s.paused = true;
    s.tick().unwrap();
    for offset in [0.0, 1.0] {
        s.set_parameter(button, 0, 1.0).unwrap();
        s.set_parameter(button, 0, 0.0).unwrap();
        now.set(offset + 0.016);
        s.tick().unwrap();
        assert_eq!(rotation(&s, effect), 5.0);
        for (elapsed, expected) in [(0.25, 2.5), (0.5, 0.0)] {
            now.set(offset + 0.016 + elapsed);
            s.tick().unwrap();
            assert!((rotation(&s, effect) - expected).abs() < 1e-5);
            assert_eq!(s.seconds(), 0.0, "media must remain paused");
        }
    }
}
