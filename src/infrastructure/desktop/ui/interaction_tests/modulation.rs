use super::*;
use std::{cell::Cell, rc::Rc};
use vjay_application::project::WidgetKind;
use vjay_domain::builtins::ids;

struct LiveClock(Rc<Cell<f64>>);
impl vjay_application::ports::Clock for LiveClock {
    fn seconds(&self) -> f64 {
        self.0.get()
    }
}
struct LiveRuntime(Rc<Cell<f64>>);
impl RuntimeFactory for LiveRuntime {
    fn create_renderer(&self) -> PortResult<Box<dyn RenderBackend>> {
        Ok(Box::new(Renderer))
    }
    fn create_clock(&self) -> Box<dyn vjay_application::ports::Clock> {
        Box::new(LiveClock(self.0.clone()))
    }
}

#[gpui::test]
fn control_panel_button_animates_rotation_while_transport_is_paused(cx: &mut TestAppContext) {
    let now = Rc::new(Cell::new(0.0));
    let clock = now.clone();
    let (view, cx) = cx.add_window_view(move |window, cx| {
        let mut s = Session::new(
            Box::new(builtin_registry()),
            &LiveRuntime(clock),
            Box::new(JsonProjectStore::new(
                "/tmp/vjay-ui-test-unused.json".into(),
            )),
            Extent::new(960, 540).unwrap(),
        )
        .unwrap();
        let out = s.project().output.unwrap();
        s.add_control(WidgetKind::Button).unwrap();
        let button = s.project().controls[0].node;
        let mut add = |kind| s.add_node(kind, Position { x: 20.0, y: 20.0 }).unwrap();
        let image = add(ids::CHECKERBOARD);
        let effect = add(ids::KALEIDOSCOPE);
        let envelope = add("Envelope");
        let multiply = add(ids::MULTIPLY);
        s.set_parameter(envelope, 0, 0.001).unwrap();
        s.set_parameter(envelope, 1, 0.5).unwrap();
        s.set_parameter(multiply, 1, 5.0).unwrap();
        for (from, output, to, input) in [
            (image, 0, effect, 0),
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
        s.paused = true;
        s.tick().unwrap();
        Editor::new(Ok((s, None, None)), window, cx, false)
    });
    cx.simulate_resize(size(px(1440.0), px(940.0)));
    let button = center(cx, "widget-0");
    cx.simulate_click(button, Modifiers::none());
    for (time, expected) in [(0.016, 5.0), (0.266, 2.5), (0.516, 0.0)] {
        now.set(time);
        view.update(cx, |editor, _| {
            let s = editor.session_mut();
            s.tick().unwrap();
            let step = s
                .plan()
                .unwrap()
                .steps
                .iter()
                .find(|step| step.operation.selector == 7.0)
                .unwrap();
            assert!((step.operation.parameters[1] - expected).abs() < 1e-5);
            assert_eq!(s.seconds(), 0.0);
            assert!(s.paused);
        });
    }
}
