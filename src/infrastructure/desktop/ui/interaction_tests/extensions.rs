use super::*;
use crate::desktop::open_node_tests::{Extension, TYPE_ID};

#[gpui::test]
fn browser_discovers_extension_categories_and_adds_nodes_from_registry(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|window, cx| {
        let mut nodes = vjay_domain::builtins::registry();
        nodes.register(Extension).unwrap();
        let session = Session::new(
            Box::new(nodes),
            &Runtime,
            Box::new(JsonProjectStore::new(
                "/tmp/vjay-extension-ui-unused.json".into(),
            )),
            Extent::new(64, 64).unwrap(),
        )
        .unwrap();
        Editor::new(Ok((session, None, None)), window, cx, false)
    });
    cx.simulate_resize(size(px(1440.0), px(940.0)));
    let input = center(cx, "text-input");
    cx.simulate_click(input, Modifiers::none());
    cx.simulate_input("Extensions");
    cx.run_until_parked();
    let row = center(cx, "browser-test.open-extension.v1");
    cx.simulate_click(row, Modifiers::none());
    cx.read(|cx| {
        let editor = view.read(cx);
        let project = editor.session().project();
        assert_eq!(project.graph.nodes().len(), 2);
        assert_eq!(project.graph.nodes()[1].type_id, TYPE_ID);
        assert!(project.graph.connection_count() == 0);
        assert_eq!(editor.session().descriptor(TYPE_ID).outputs.len(), 7);
        assert_eq!(editor.selection.primary(), Some(NodeId(1)));
    });
    let setting = center(cx, "node-property-channel");
    cx.simulate_click(setting, Modifiers::none());
    cx.simulate_keystrokes("cmd-a");
    cx.simulate_input("test.other");
    let apply = center(cx, "apply-configuration");
    cx.simulate_click(apply, Modifiers::none());
    cx.read(|cx| {
        let editor = view.read(cx);
        assert!(editor.configuration.is_none());
        assert_eq!(
            editor.session().project().graph.nodes()[1]
                .settings
                .properties["channel"],
            "test.other"
        );
    });
}
