use super::*;
use std::collections::HashSet;

fn world(view: &Entity<Editor>, cx: &mut VisualTestContext, x: f32, y: f32) -> Point<Pixels> {
    cx.refresh().unwrap();
    cx.run_until_parked();
    cx.read(|cx| {
        let editor = view.read(cx);
        editor.patch_bounds.get().unwrap().origin
            + point(px(x + editor.pan.x), px(y + editor.pan.y))
    })
}
fn selected(view: &Entity<Editor>, cx: &VisualTestContext) -> HashSet<NodeId> {
    cx.read(|cx| view.read(cx).selection.ids.clone())
}
fn gesture(
    cx: &mut VisualTestContext,
    from: Point<Pixels>,
    to: Point<Pixels>,
    modifiers: Modifiers,
) {
    cx.simulate_mouse_move(from, None, modifiers);
    cx.simulate_mouse_down(from, MouseButton::Left, modifiers);
    cx.simulate_mouse_move(to, MouseButton::Left, modifiers);
    cx.simulate_mouse_up(to, MouseButton::Left, modifiers);
    cx.run_until_parked();
}
fn pair() -> HashSet<NodeId> {
    [NodeId(0), NodeId(1)].into_iter().collect()
}
fn select_pair(view: &Entity<Editor>, cx: &mut VisualTestContext) {
    let a = world(view, cx, 15.0, 60.0);
    let b = world(view, cx, 245.0, 350.0);
    gesture(cx, a, b, Modifiers::none());
    assert_eq!(selected(view, cx), pair());
}

#[gpui::test]
fn marquee_moves_and_deletes_selected_nodes_together_without_changing_wires(
    cx: &mut TestAppContext,
) {
    let (view, cx) = cx.add_window_view(editor);
    cx.simulate_resize(size(px(1440.0), px(940.0)));
    select_pair(&view, cx);
    let before = cx.read(|cx| view.read(cx).session().project().positions.clone());
    let wires = cx.read(|cx| {
        view.read(cx)
            .session()
            .project()
            .graph
            .connections()
            .collect::<Vec<_>>()
    });
    let header = center(cx, "header-0");
    drag(cx, header, header + point(px(54.0), px(-25.0)));
    assert_eq!(selected(&view, cx), pair());
    cx.read(|cx| {
        for p in &view.read(cx).session().project().positions {
            let original = before.iter().find(|n| n.node == p.node).unwrap().position;
            let delta = if pair().contains(&p.node) {
                (54.0, -25.0)
            } else {
                (0.0, 0.0)
            };
            assert_eq!(
                (p.position.x, p.position.y),
                (original.x + delta.0, original.y + delta.1)
            );
        }
    });
    assert_eq!(
        cx.read(|cx| view
            .read(cx)
            .session()
            .project()
            .graph
            .connections()
            .collect::<Vec<_>>()),
        wires
    );
    cx.simulate_keystrokes("backspace");
    view.update(cx, |editor, _| {
        let project = editor.session().project();
        assert!(project.graph.node(NodeId(0)).is_err());
        assert!(project.graph.node(NodeId(1)).is_err());
        assert_eq!(project.graph.nodes().len(), 3);
        assert_eq!(project.controls.len(), 2);
        assert!(editor.session_mut().render_at(0.1).is_err());
    });
}
#[gpui::test]
fn shift_click_and_shift_marquee_add_or_remove_nodes_and_escape_restores_selection(
    cx: &mut TestAppContext,
) {
    let (view, cx) = cx.add_window_view(editor);
    cx.simulate_resize(size(px(1440.0), px(940.0)));
    select_pair(&view, cx);
    let mut shift = Modifiers::none();
    shift.shift = true;
    let header = center(cx, "header-2");
    cx.simulate_click(header, shift);
    assert_eq!(
        selected(&view, cx),
        [NodeId(0), NodeId(1), NodeId(2)].into_iter().collect()
    );
    let header = center(cx, "header-1");
    cx.simulate_click(header, shift);
    assert_eq!(
        selected(&view, cx),
        [NodeId(0), NodeId(2)].into_iter().collect()
    );
    let a = world(&view, cx, 480.0, 120.0);
    let b = world(&view, cx, 710.0, 255.0);
    gesture(cx, a, b, shift);
    let retained = selected(&view, cx);
    assert_eq!(
        retained,
        [NodeId(0), NodeId(2), NodeId(3)].into_iter().collect()
    );
    let a = world(&view, cx, 15.0, 60.0);
    let b = world(&view, cx, 245.0, 350.0);
    cx.simulate_mouse_down(a, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(b, MouseButton::Left, Modifiers::none());
    assert_eq!(selected(&view, cx), pair());
    cx.simulate_keystrokes("escape");
    assert_eq!(selected(&view, cx), retained);
    cx.simulate_mouse_up(b, MouseButton::Left, Modifiers::none());
}
#[gpui::test]
fn reverse_marquee_uses_panned_graph_coordinates_and_select_all_can_delete_every_node(
    cx: &mut TestAppContext,
) {
    let (view, cx) = cx.add_window_view(editor);
    cx.simulate_resize(size(px(1440.0), px(940.0)));
    let from = world(&view, cx, 800.0, 430.0);
    cx.simulate_mouse_down(from, MouseButton::Middle, Modifiers::none());
    cx.simulate_mouse_move(
        from + point(px(90.0), px(-45.0)),
        MouseButton::Middle,
        Modifiers::none(),
    );
    cx.simulate_mouse_up(
        from + point(px(90.0), px(-45.0)),
        MouseButton::Middle,
        Modifiers::none(),
    );
    let a = world(&view, cx, 245.0, 350.0);
    let b = world(&view, cx, 15.0, 60.0);
    gesture(cx, a, b, Modifiers::none());
    assert_eq!(selected(&view, cx), pair());
    let blank = world(&view, cx, 700.0, 450.0);
    cx.simulate_click(blank, Modifiers::none());
    assert!(selected(&view, cx).is_empty());
    cx.simulate_keystrokes("cmd-a delete");
    view.update(cx, |editor, _| {
        assert!(editor.session().project().graph.nodes().is_empty());
        assert!(editor.session().project().output.is_none());
        editor.session_mut().render_at(0.0).unwrap();
        assert!(editor.session().plan().unwrap().steps.is_empty());
    });
}

#[gpui::test]
fn marquee_finishes_on_socket_and_escape_restores_group_positions(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(editor);
    cx.simulate_resize(size(px(1440.0), px(940.0)));
    let from = world(&view, cx, 15.0, 60.0);
    let socket = center(cx, "in-1-2");
    gesture(cx, from, socket, Modifiers::none());
    assert!(cx.read(|cx| view.read(cx).marquee.is_none()));
    assert!(cx.read(|cx| view.read(cx).cable.is_none()));
    assert_eq!(selected(&view, cx), pair());
    let positions = cx.read(|cx| view.read(cx).session().project().positions.clone());
    let header = center(cx, "header-0");
    cx.simulate_mouse_down(header, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(
        header + point(px(25.0), px(35.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.simulate_keystrokes("escape");
    cx.simulate_mouse_up(
        header + point(px(25.0), px(35.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.read(|cx| {
        for (before, after) in positions
            .iter()
            .zip(&view.read(cx).session().project().positions)
        {
            assert_eq!(before.node, after.node);
            assert_eq!(
                (before.position.x, before.position.y),
                (after.position.x, after.position.y)
            );
        }
    });
}
