use super::*;
#[gpui::test]
fn encapsulate_open_and_breadcrumb_back_edit_the_child_and_keep_the_program(
    cx: &mut TestAppContext,
) {
    let (view, cx) = cx.add_window_view(editor);
    cx.simulate_resize(size(px(1600.0), px(1100.0)));
    view.update(cx, |this, cx| {
        this.selection.set(&[NodeId(2), NodeId(3)]);
        cx.notify();
    });
    cx.simulate_keystrokes("cmd-shift-e");
    cx.run_until_parked();
    let patch = cx.read(|cx| view.read(cx).selection.primary().unwrap());
    assert!(cx.read(|cx| view.read(cx).session().is_subpatch(patch)));
    let open = center(cx, "open-subpatch");
    cx.simulate_click(open, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(cx.read(|cx| view.read(cx).session().patch_depth()), 1);
    assert!(cx.debug_bounds("header-2").is_some());
    assert!(cx.debug_bounds("save-current-subpatch").is_some());
    let back = center(cx, "patch-depth-0");
    cx.simulate_click(back, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(cx.read(|cx| view.read(cx).session().patch_depth()), 0);
    assert!(cx.read(|cx| view.read(cx).session().project().output.is_some()));
}
#[gpui::test]
fn media_inspector_transport_is_available_without_a_controller_and_nudge_releases_outside(
    cx: &mut TestAppContext,
) {
    let (view, cx) = cx.add_window_view(editor);
    cx.simulate_resize(size(px(1800.0), px(1260.0)));
    let video = view.update(cx, |this, cx| {
        let video = this
            .session_mut()
            .import_video("/fixtures/clip.mov".into(), None)
            .unwrap();
        this.selection.only(video);
        this.session_mut().render_at(0.0).unwrap();
        cx.notify();
        video
    });
    assert!(
        cx.debug_bounds("deck-jog").is_some() || {
            cx.refresh().unwrap();
            cx.debug_bounds("deck-jog").is_some()
        }
    );
    let nudge = center(cx, "source-nudge-1");
    cx.simulate_mouse_down(nudge, MouseButton::Left, Modifiers::none());
    view.update(cx, |this, _| this.session_mut().render_at(0.1).unwrap());
    assert!(
        (cx.read(|cx| view.read(cx).session().media_state(video).unwrap().speed) - 1.02).abs()
            < 1e-6
    );
    cx.simulate_mouse_up(
        point(px(300.0), px(800.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    view.update(cx, |this, _| this.session_mut().render_at(0.2).unwrap());
    assert_eq!(
        cx.read(|cx| view.read(cx).session().media_state(video).unwrap().speed),
        1.0
    );
    let pause = center(cx, "deck-play");
    cx.simulate_click(pause, Modifiers::none());
    view.update(cx, |this, _| this.session_mut().render_at(0.3).unwrap());
    assert!(!cx.read(|cx| view.read(cx).session().media_state(video).unwrap().playing));
}
