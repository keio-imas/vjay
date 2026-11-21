use super::*;
#[gpui::test]
fn modifier_wheel_zooms_at_cursor_and_preserves_drag_and_socket_hitboxes(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(editor);
    cx.simulate_resize(size(px(1440.0), px(940.0)));
    let anchor = center(cx, "header-0");
    let world = cx.read(|cx| view.read(cx).world_pointer(position(anchor)).unwrap());
    cx.simulate_event(ScrollEvent {
        position: anchor,
        delta: gpui::ScrollDelta::Pixels(point(px(0.0), px(2.0_f32.ln() / 0.002))),
        modifiers: Modifiers {
            platform: true,
            ..Default::default()
        },
        ..Default::default()
    });
    cx.refresh().unwrap();
    cx.run_until_parked();
    cx.read(|cx| {
        let e = view.read(cx);
        let after = e.world_pointer(position(anchor)).unwrap();
        assert!((after.x - world.x).abs() < 0.01 && (after.y - world.y).abs() < 0.01);
        assert!((e.zoom - 2.0).abs() < 0.001);
        assert_eq!(e.session().project().workspace.patch_zoom, e.zoom);
    });
    let width = f32::from(cx.debug_bounds("header-0").unwrap().size.width);
    assert!((width - 398.0).abs() < 3.0);
    // Zoom back out so all original sockets remain in the viewport.
    cx.simulate_event(ScrollEvent {
        position: anchor,
        delta: gpui::ScrollDelta::Pixels(point(px(0.0), px(0.25_f32.ln() / 0.002))),
        modifiers: Modifiers {
            control: true,
            ..Default::default()
        },
        ..Default::default()
    });
    cx.refresh().unwrap();
    cx.run_until_parked();
    let header = center(cx, "header-0");
    let before = cx.read(|cx| {
        view.read(cx)
            .session()
            .project()
            .positions
            .iter()
            .find(|p| p.node == NodeId(0))
            .unwrap()
            .position
    });
    drag(cx, header, header + point(px(-24.0), px(12.0)));
    cx.read(|cx| {
        let e = view.read(cx);
        let after = e
            .session()
            .project()
            .positions
            .iter()
            .find(|p| p.node == NodeId(0))
            .unwrap()
            .position;
        assert!((after.x - before.x + 48.0).abs() < 0.01);
        assert!((after.y - before.y - 24.0).abs() < 0.01);
    });
    let from = center(cx, "out-0");
    let to = center(cx, "in-3-0");
    drag(cx, from, to);
    assert_eq!(connection(&view, cx, 3, 0).unwrap().from, NodeId(0));
}
#[gpui::test]
fn control_inspector_only_shows_current_binding_and_searches_targets_on_demand(
    cx: &mut TestAppContext,
) {
    let (view, cx) = cx.add_window_view(editor);
    cx.simulate_resize(size(px(1440.0), px(940.0)));
    view.update(cx, |e, cx| {
        e.selected_control = Some(0);
        cx.notify();
    });
    center(cx, "change-control-binding");
    let before = cx.debug_bounds("current-control-binding").unwrap();
    view.update(cx, |e, cx| {
        for _ in 0..30 {
            e.session_mut()
                .add_node(
                    "LFO",
                    Position {
                        x: 1000.0,
                        y: 1000.0,
                    },
                )
                .unwrap();
        }
        cx.notify();
    });
    let change = center(cx, "change-control-binding");
    assert_eq!(
        cx.debug_bounds("current-control-binding").unwrap().size,
        before.size
    );
    cx.simulate_click(change, Modifiers::none());
    center(cx, "binding-picker");
    cx.simulate_input("lfo");
    cx.refresh().unwrap();
    cx.run_until_parked();
    cx.read(|cx| assert!(view.read(cx).binding_picker.is_some()));
    let candidate = center(cx, "binding-candidate-0");
    cx.simulate_click(candidate, Modifiers::none());
    cx.read(|cx| {
        let e = view.read(cx);
        assert!(e.binding_picker.is_none());
        let c = e
            .session()
            .project()
            .controls
            .iter()
            .find(|c| c.id == 0)
            .unwrap();
        assert_eq!(
            e.session().project().graph.node(c.node).unwrap().type_id,
            "LFO"
        );
    });
}
