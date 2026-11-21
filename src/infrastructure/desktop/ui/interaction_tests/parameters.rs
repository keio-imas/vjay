use super::*;
use vjay_application::project::WidgetKind;

fn value(view: &Entity<Editor>, cx: &VisualTestContext, node: NodeId, parameter: usize) -> f32 {
    cx.read(|cx| {
        view.read(cx)
            .session()
            .project()
            .graph
            .node(node)
            .unwrap()
            .parameters[parameter]
    })
}
fn near(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 0.00001,
        "{actual} != {expected}"
    );
}
fn modifiers(shift: bool, control: bool, platform: bool) -> Modifiers {
    let mut result = Modifiers::none();
    result.shift = shift;
    result.control = control;
    result.platform = platform;
    result
}

#[gpui::test]
fn parameter_drag_switches_precision_without_jumps_and_snaps_with_ctrl_or_cmd(
    cx: &mut TestAppContext,
) {
    let (view, cx) = cx.add_window_view(editor);
    cx.simulate_resize(size(px(1440.0), px(940.0)));
    let header = center(cx, "header-2");
    cx.simulate_click(header, Modifiers::none());
    let from = center(cx, "parameter-2-0-false");
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(
        from + point(px(20.0), px(0.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    near(value(&view, cx, NodeId(2), 0), 0.6);
    cx.simulate_mouse_move(
        from + point(px(40.0), px(0.0)),
        MouseButton::Left,
        modifiers(true, false, false),
    );
    near(value(&view, cx, NodeId(2), 0), 0.61);
    cx.simulate_mouse_move(
        from + point(px(40.0), px(0.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    near(value(&view, cx, NodeId(2), 0), 0.61);
    cx.simulate_mouse_move(
        from + point(px(51.0), px(0.0)),
        MouseButton::Left,
        modifiers(false, true, false),
    );
    near(value(&view, cx, NodeId(2), 0), 0.65);
    let end = from + point(px(60.0), px(0.0));
    cx.simulate_mouse_move(end, MouseButton::Left, modifiers(false, false, true));
    near(value(&view, cx, NodeId(2), 0), 0.7);
    cx.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    near(value(&view, cx, NodeId(2), 0), 0.7);
    cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    let number = center(cx, "number-2-0");
    let end = number - point(px(0.0), px(20.0));
    let shift = modifiers(true, false, false);
    cx.simulate_mouse_down(number, MouseButton::Left, shift);
    cx.simulate_mouse_move(end, MouseButton::Left, shift);
    cx.simulate_mouse_up(end, MouseButton::Left, shift);
    near(value(&view, cx, NodeId(2), 0), 0.71);
    assert!(cx.read(|cx| view.read(cx).configuration.is_none()));
    let number = center(cx, "number-2-0");
    cx.simulate_click(number, Modifiers::none());
    assert!(cx.read(|cx| view.read(cx).configuration.is_some()));
}

#[gpui::test]
fn slider_knob_and_number_widgets_share_precision_and_snap_behavior(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(editor);
    cx.simulate_resize(size(px(1440.0), px(940.0)));
    for widget in [WidgetKind::Slider, WidgetKind::Knob, WidgetKind::Number] {
        view.update(cx, |this, cx| {
            this.session_mut().set_parameter(NodeId(2), 0, 0.5).unwrap();
            let mut binding = this.session().project().controls[0].clone();
            binding.widget = widget;
            this.session_mut().configure_control(binding).unwrap();
            cx.notify();
        });
        let from = center(cx, "widget-0");
        let delta = if widget == WidgetKind::Slider {
            point(px(20.0), px(0.0))
        } else {
            point(px(0.0), px(-20.0))
        };
        let shift = modifiers(true, false, false);
        cx.simulate_mouse_down(from, MouseButton::Left, shift);
        cx.simulate_mouse_move(from + delta, MouseButton::Left, shift);
        cx.simulate_mouse_up(from + delta, MouseButton::Left, shift);
        near(value(&view, cx, NodeId(2), 0), 0.51);
        assert!(cx.read(|cx| view.read(cx).configuration.is_none()));
    }
}

#[gpui::test]
fn xy_pad_shift_drag_keeps_initial_values_and_modifies_both_axes(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(editor);
    cx.simulate_resize(size(px(1440.0), px(940.0)));
    let (control, node) = view.update(cx, |this, cx| {
        let ids = this
            .session()
            .project()
            .controls
            .iter()
            .map(|c| c.id)
            .collect::<Vec<_>>();
        for id in ids {
            this.session_mut().remove_control(id);
        }
        let id = this.session_mut().add_control(WidgetKind::XYPad).unwrap();
        this.session_mut().update_control_layout(
            id,
            Position { x: 12.0, y: 12.0 },
            Position { x: 200.0, y: 180.0 },
        );
        let node = this.session().project().controls[0].node;
        cx.notify();
        (id, node)
    });
    assert_eq!(control, 0);
    let pointer = center(cx, "widget-0");
    let bounds = cx.debug_bounds("widget-0").unwrap();
    let original = [value(&view, cx, node, 0), value(&view, cx, node, 1)];
    let shift = modifiers(true, false, false);
    cx.simulate_mouse_down(pointer, MouseButton::Left, shift);
    near(value(&view, cx, node, 0), original[0]);
    near(value(&view, cx, node, 1), original[1]);
    let end = pointer + point(px(10.0), px(-10.0));
    cx.simulate_mouse_move(end, MouseButton::Left, shift);
    cx.simulate_mouse_up(end, MouseButton::Left, shift);
    near(
        value(&view, cx, node, 0),
        original[0] + 1.0 / f32::from(bounds.size.width),
    );
    near(
        value(&view, cx, node, 1),
        original[1] + 1.0 / f32::from(bounds.size.height),
    );
}
