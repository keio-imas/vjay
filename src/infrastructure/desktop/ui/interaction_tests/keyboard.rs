use super::*;

#[gpui::test]
fn new_project_shortcut_preserves_unsaved_work_when_prompt_is_cancelled(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(editor);
    cx.simulate_resize(size(px(1440.0), px(940.0)));
    let header = center(cx, "header-0");
    cx.simulate_click(header, Modifiers::none());
    view.update(cx, |this, _| {
        this.session_mut()
            .add_node("Files", Position { x: 0.0, y: 0.0 })
            .unwrap();
    });
    cx.simulate_keystrokes("cmd-n");
    cx.run_until_parked();
    cx.simulate_prompt_answer("Cancel");
    cx.run_until_parked();
    assert_eq!(
        cx.read(|cx| view.read(cx).session().project().graph.nodes().len()),
        6
    );
    cx.simulate_keystrokes("ctrl-n");
    cx.run_until_parked();
    cx.simulate_prompt_answer("Discard");
    cx.run_until_parked();
    cx.read(|cx| {
        let s = view.read(cx).session();
        assert_eq!(s.project().graph.nodes().len(), 1);
        assert!(s.is_untitled());
    });
}

#[gpui::test]
fn backspace_deletes_clicked_node_and_its_connections_controls_and_layout(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(editor);
    cx.simulate_resize(size(px(1440.0), px(940.0)));
    let header = center(cx, "header-0");
    cx.simulate_click(header, Modifiers::none());
    cx.simulate_keystrokes("backspace");
    cx.read(|cx| {
        let editor = view.read(cx);
        let project = editor.session().project();
        assert!(project.graph.node(NodeId(0)).is_err());
        assert!(
            project
                .graph
                .connections()
                .all(|e| e.from != NodeId(0) && e.to != NodeId(0))
        );
        assert!(project.controls.iter().all(|c| c.node != NodeId(0)));
        assert!(project.positions.iter().all(|p| p.node != NodeId(0)));
        assert!(editor.selection.primary().is_none());
    });
}
#[gpui::test]
fn delete_removes_program_output_without_creating_a_replacement(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(editor);
    cx.simulate_resize(size(px(1440.0), px(940.0)));
    let header = center(cx, "header-4");
    cx.simulate_click(header, Modifiers::none());
    cx.simulate_keystrokes("delete");
    view.update(cx, |editor, _| {
        assert!(editor.session().project().output.is_none());
        assert_eq!(editor.session().project().graph.nodes().len(), 4);
        editor.session_mut().render_at(0.0).unwrap();
        assert!(!editor.session().plan().unwrap().program_active);
    });
}
#[gpui::test]
fn delete_and_backspace_edit_focused_text_and_preserve_selected_nodes(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(editor);
    cx.simulate_resize(size(px(1440.0), px(940.0)));
    let header = center(cx, "header-0");
    cx.simulate_click(header, Modifiers::none());
    let input = center(cx, "text-input");
    cx.simulate_click(input, Modifiers::none());
    cx.refresh().unwrap();
    cx.simulate_input("abc");
    cx.simulate_keystrokes("backspace delete");
    cx.read(|cx| {
        let editor = view.read(cx);
        assert_eq!(editor.search.as_ref().unwrap().read(cx).text, "ab");
        assert!(editor.session().project().graph.node(NodeId(0)).is_ok());
    });
    cx.update(|window, cx| {
        view.update(cx, |editor, cx| {
            editor.open_configuration(configuration::Target::Number(NodeId(0), 0), window, cx)
        })
    });
    cx.refresh().unwrap();
    cx.simulate_keystrokes("cmd-a backspace delete");
    cx.read(|cx| {
        assert!(
            view.read(cx)
                .session()
                .project()
                .graph
                .node(NodeId(0))
                .is_ok()
        )
    });
}
