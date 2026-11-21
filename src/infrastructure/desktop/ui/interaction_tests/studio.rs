use super::*;
#[gpui::test]
fn alt_drag_copies_and_escape_removes_only_the_unfinished_copy(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(editor);
    cx.simulate_resize(size(px(1440.0), px(940.0)));
    let header = center(cx, "header-0");
    let modifiers = Modifiers {
        alt: true,
        ..Modifiers::none()
    };
    cx.simulate_mouse_down(header, MouseButton::Left, modifiers);
    cx.simulate_mouse_up(header, MouseButton::Left, modifiers);
    cx.read(|cx| assert_eq!(view.read(cx).session().project().graph.nodes().len(), 5));
    cx.simulate_mouse_down(header, MouseButton::Left, modifiers);
    cx.simulate_mouse_move(
        header + point(px(40.0), px(50.0)),
        MouseButton::Left,
        modifiers,
    );
    cx.read(|cx| {
        let e = view.read(cx);
        assert_eq!(e.session().project().graph.nodes().len(), 6);
        assert_eq!(e.selection.len(), 1);
        assert!(!e.selection.contains(NodeId(0)));
    });
    cx.simulate_keystrokes("escape");
    cx.simulate_mouse_up(header, MouseButton::Left, modifiers);
    cx.read(|cx| {
        let e = view.read(cx);
        assert_eq!(e.session().project().graph.nodes().len(), 5);
        assert!(e.selection.contains(NodeId(0)));
    });
}
#[gpui::test]
fn keyboard_and_context_menu_copy_paste_use_the_clipboard(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(editor);
    cx.simulate_resize(size(px(1440.0), px(940.0)));
    let header = center(cx, "header-0");
    cx.simulate_click(header, Modifiers::none());
    cx.simulate_keystrokes("cmd-c cmd-v");
    cx.read(|cx| assert_eq!(view.read(cx).session().project().graph.nodes().len(), 6));
    cx.simulate_keystrokes("ctrl-c ctrl-v");
    cx.read(|cx| assert_eq!(view.read(cx).session().project().graph.nodes().len(), 7));
    let header = center(cx, "header-0");
    cx.simulate_mouse_down(header, MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(header, MouseButton::Right, Modifiers::none());
    let copy = center(cx, "copy-nodes");
    cx.simulate_click(copy, Modifiers::none());
    cx.simulate_mouse_down(header, MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(header, MouseButton::Right, Modifiers::none());
    let paste = center(cx, "paste-nodes");
    cx.simulate_click(paste, Modifiers::none());
    cx.read(|cx| assert_eq!(view.read(cx).session().project().graph.nodes().len(), 8));
}
#[gpui::test]
fn node_preview_switch_and_uniform_editor_change_instance_schema(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(editor);
    cx.simulate_resize(size(px(1440.0), px(940.0)));
    let before = cx.read(|cx| {
        view.read(cx)
            .session()
            .project()
            .graph
            .node(NodeId(0))
            .unwrap()
            .settings
            .preview
    });
    let toggle = center(cx, "preview-0");
    cx.simulate_click(toggle, Modifiers::none());
    cx.read(|cx| {
        assert_eq!(
            view.read(cx)
                .session()
                .project()
                .graph
                .node(NodeId(0))
                .unwrap()
                .settings
                .preview,
            !before
        )
    });
    view.update(cx, |this, cx| {
        let id = this
            .session_mut()
            .add_node(
                vjay_domain::builtins::ids::GLSL,
                Position { x: 30.0, y: 20.0 },
            )
            .unwrap();
        this.selection.only(id);
        cx.notify();
    });
    let add = center(cx, "add-uniform");
    cx.simulate_click(add, Modifiers::none());
    cx.read(|cx| {
        let e = view.read(cx);
        assert_eq!(
            e.session()
                .schema(e.selection.primary().unwrap())
                .parameters
                .len(),
            5
        );
    });
}
#[gpui::test]
fn scrolling_the_browser_reveals_the_last_node_above_input_controls(cx: &mut TestAppContext) {
    let (_view, cx) = cx.add_window_view(editor);
    cx.simulate_resize(size(px(1000.0), px(700.0)));
    let list = center(cx, "node-results");
    cx.simulate_event(ScrollEvent {
        position: list,
        delta: gpui::ScrollDelta::Pixels(point(px(0.0), px(-100000.0))),
        ..Default::default()
    });
    cx.refresh().unwrap();
    cx.run_until_parked();
    let video = cx
        .debug_bounds("browser-LipSync")
        .expect("last catalog entry");
    let results = cx.debug_bounds("node-results").unwrap();
    assert!(
        video.bottom() <= results.bottom() + px(1.0),
        "last row is below its scroll viewport: video={video:?} results={results:?}"
    );
    assert!(video.top() >= results.top());
    assert!(cx.debug_bounds("input-sources").unwrap().bottom() <= results.top());
}

#[gpui::test]
fn transport_has_start_end_and_no_independent_video_import_button(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(editor);
    cx.simulate_resize(size(px(1440.0), px(940.0)));
    view.update(cx, |editor, _| {
        editor.session_mut().render_at(12.0).unwrap()
    });
    let end = center(cx, "transport-end");
    cx.simulate_click(end, Modifiers::none());
    cx.read(|cx| assert!(view.read(cx).session().paused));
    let start = center(cx, "transport-start");
    cx.simulate_click(start, Modifiers::none());
    cx.read(|cx| {
        let e = view.read(cx);
        assert_eq!(e.session().seconds(), 0.0);
        assert!(e.session().paused);
    });
    cx.refresh().unwrap();
    assert!(cx.debug_bounds("import").is_none());
}
