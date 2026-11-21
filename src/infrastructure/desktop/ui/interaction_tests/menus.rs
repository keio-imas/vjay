use super::*;
#[gpui::test]
fn tab_palette_uses_cursor_and_adds_a_searched_node_without_connections(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(editor);
    cx.simulate_resize(size(px(1440.0), px(940.0)));
    let cursor = center(cx, "patch-canvas");
    cx.simulate_mouse_move(cursor, None, Modifiers::none());
    cx.simulate_keystrokes("tab");
    cx.refresh().unwrap();
    cx.run_until_parked();
    let bounds = cx
        .debug_bounds("node-palette")
        .expect("palette opens on Tab");
    assert!(bounds.left() >= cursor.x);
    cx.simulate_input("compare");
    cx.run_until_parked();
    cx.simulate_keystrokes("enter");
    cx.read(|cx| {
        let e = view.read(cx);
        assert!(e.palette.is_none());
        assert_eq!(
            e.session().project().graph.nodes().last().unwrap().type_id,
            "Compare"
        );
        assert_eq!(e.session().project().graph.connection_count(), 4);
    });
}
#[gpui::test]
fn context_menu_closes_when_a_node_consumes_the_outside_click(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(editor);
    cx.simulate_resize(size(px(1440.0), px(940.0)));
    let header = center(cx, "header-0");
    cx.simulate_mouse_down(header, MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(header, MouseButton::Right, Modifiers::none());
    cx.refresh().unwrap();
    cx.run_until_parked();
    assert!(cx.debug_bounds("patch-menu").is_some());
    let outside = center(cx, "header-2");
    assert!(
        !cx.debug_bounds("patch-menu").unwrap().contains(&outside),
        "outside node header is actually inside menu: {:?} {outside:?}",
        cx.debug_bounds("patch-menu")
    );
    cx.simulate_mouse_move(outside, None, Modifiers::none());
    cx.simulate_click(outside, Modifiers::none());
    cx.read(|cx| assert!(view.read(cx).patch_menu.is_none()));
}
#[gpui::test]
fn insert_menu_scroll_does_not_pan_the_patch_and_can_reach_its_last_item(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(editor);
    cx.simulate_resize(size(px(1440.0), px(940.0)));
    view.update(cx, |e, cx| {
        e.session_mut()
            .move_node(NodeId(0), Position { x: 20.0, y: 40.0 });
        e.session_mut()
            .move_node(NodeId(2), Position { x: 420.0, y: 60.0 });
        cx.notify();
    });
    let a = center(cx, "out-0");
    let b = center(cx, "in-2-0");
    let at = point((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
    cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(at, MouseButton::Right, Modifiers::none());
    let menu = center(cx, "insert-menu");
    let before = cx.read(|cx| view.read(cx).pan);
    cx.simulate_event(ScrollWheelEvent {
        position: menu,
        delta: gpui::ScrollDelta::Pixels(point(px(0.0), px(-100000.0))),
        ..Default::default()
    });
    cx.refresh().unwrap();
    cx.run_until_parked();
    cx.read(|cx| {
        let after = view.read(cx).pan;
        assert_eq!(before.x, after.x);
        assert_eq!(before.y, after.y);
    });
    let bounds = cx.debug_bounds("insert-menu").unwrap();
    let last = cx.debug_bounds("disconnect-wire").unwrap();
    assert!(
        last.bottom() <= bounds.bottom() + px(1.0),
        "menu must scroll internally: {bounds:?} {last:?}"
    );
    let outside = center(cx, "header-0");
    cx.simulate_click(outside, Modifiers::none());
    cx.read(|cx| assert!(view.read(cx).insert_menu.is_none()));
}
#[gpui::test]
fn compare_mode_uses_named_dropdown_and_inputs_are_in_the_top_bar(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(editor);
    cx.simulate_resize(size(px(1440.0), px(940.0)));
    let node = view.update(cx, |e, cx| {
        let id = e
            .session_mut()
            .add_node("Compare", Position { x: 30.0, y: 40.0 })
            .unwrap();
        e.selection.only(id);
        cx.notify();
        id
    });
    cx.refresh().unwrap();
    cx.run_until_parked();
    let field = cx
        .debug_bounds("choice-5-2")
        .expect("named dropdown")
        .center();
    cx.simulate_click(field, Modifiers::none());
    let option = center(cx, "option-4");
    cx.simulate_click(option, Modifiers::none());
    cx.read(|cx| {
        let e = view.read(cx);
        assert!(e.parameter_menu.is_none());
        assert_eq!(
            e.session().project().graph.node(node).unwrap().parameters[2],
            4.0
        );
    });
    cx.refresh().unwrap();
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("input-sources").unwrap().bottom()
            <= cx.debug_bounds("node-results").unwrap().top()
    );
    let field = center(cx, "choice-5-2");
    cx.simulate_click(field, Modifiers::none());
    cx.simulate_keystrokes("backspace");
    cx.read(|cx| {
        let e = view.read(cx);
        assert!(e.parameter_menu.is_none());
        assert!(e.session().project().graph.node(node).is_err());
    });
    cx.refresh().unwrap();
}

#[gpui::test]
fn text_alignment_uses_named_choices_and_accepts_numeric_input(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(editor);
    cx.simulate_resize(size(px(1440.0), px(1600.0)));
    let id = view.update(cx, |e, cx| {
        let id = e
            .session_mut()
            .add_node("Text", Position { x: 20.0, y: 30.0 })
            .unwrap();
        e.selection.only(id);
        cx.notify();
        id
    });
    let selector = Box::leak(format!("choice-{}-7", id.0).into_boxed_str());
    let field = center(cx, selector);
    cx.simulate_click(field, Modifiers::none());
    let left = center(cx, "option-0");
    cx.simulate_click(left, Modifiers::none());
    cx.read(|cx| {
        assert_eq!(
            view.read(cx)
                .session()
                .project()
                .graph
                .node(id)
                .unwrap()
                .parameters[7],
            0.0
        )
    });
}
