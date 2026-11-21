use super::*;
#[gpui::test]
fn settings_shortcut_edits_cancel_apply_and_dropdown_escape(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(editor);
    cx.simulate_resize(size(px(1440.0), px(940.0)));
    cx.simulate_keystrokes("cmd-,");
    cx.run_until_parked();
    assert!(cx.read(|cx| view.read(cx).settings.is_some()));
    let device = center(cx, "setting-device");
    cx.simulate_click(device, Modifiers::none());
    cx.run_until_parked();
    assert!(cx.read(|cx| view.read(cx).settings.as_ref().unwrap().menu_open()));
    cx.simulate_keystrokes("escape");
    assert!(cx.read(|cx| view.read(cx).settings.is_some()));
    assert!(!cx.read(|cx| view.read(cx).settings.as_ref().unwrap().menu_open()));
    let tab = center(cx, "settings-tab-2");
    cx.simulate_click(tab, Modifiers::none());
    let name = center(cx, "settings-input-name");
    cx.simulate_click(name, Modifiers::none());
    cx.refresh().unwrap();
    cx.run_until_parked();
    cx.simulate_keystrokes("cmd-a");
    cx.simulate_input("Cancelled.vjay.json");
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(cx.read(|cx| view.read(cx).settings.is_none()));
    assert_eq!(
        cx.read(|cx| view
            .read(cx)
            .session()
            .preferences()
            .default_project_name
            .clone()),
        "Untitled.vjay.json"
    );
    cx.simulate_keystrokes("cmd-,");
    let tab = center(cx, "settings-tab-2");
    cx.simulate_click(tab, Modifiers::none());
    let name = center(cx, "settings-input-name");
    cx.simulate_click(name, Modifiers::none());
    cx.refresh().unwrap();
    cx.run_until_parked();
    cx.simulate_keystrokes("cmd-a");
    cx.simulate_input("Live Set.vjay.json");
    assert_eq!(
        cx.read(|cx| view.read(cx).settings.as_ref().unwrap().value("name", cx)),
        "Live Set.vjay.json"
    );
    let apply = center(cx, "apply-settings");
    cx.simulate_click(apply, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        cx.read(|cx| view
            .read(cx)
            .session()
            .preferences()
            .default_project_name
            .clone()),
        "Live Set.vjay.json",
        "{}",
        cx.read(|cx| view.read(cx).settings.as_ref().unwrap().error.clone())
    );
    assert_eq!(
        cx.read(|cx| view.read(cx).session().project().graph.nodes().len()),
        5
    );
}
