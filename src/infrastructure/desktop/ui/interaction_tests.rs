use super::*;
use crate::storage::JsonProjectStore;
use gpui::{Modifiers, TestAppContext, VisualTestContext};
use vjay_application::{compiler::RenderPlan, ports::*};
use vjay_domain::{builtins::registry as builtin_registry, graph::Connection};

mod modulation;

struct Runtime;
struct Renderer;
struct Clock;
impl vjay_application::ports::Clock for Clock {
    fn seconds(&self) -> f64 {
        0.0
    }
}
impl RuntimeFactory for Runtime {
    fn create_control_io(&self) -> Box<dyn vjay_application::control_io::ControlIo> {
        Box::new(midi::Inputs::default())
    }
    fn create_renderer(&self) -> PortResult<Box<dyn RenderBackend>> {
        Ok(Box::new(Renderer))
    }
    fn create_clock(&self) -> Box<dyn vjay_application::ports::Clock> {
        Box::new(Clock)
    }
}
impl RenderBackend for Renderer {
    fn media_duration(&self, _: vjay_domain::graph::NodeId) -> Option<f64> {
        Some(10.0)
    }
    fn prepare(&mut self, _: &RenderPlan, _: Extent) -> PortResult<()> {
        Ok(())
    }
    fn render(&mut self, _: &RenderPlan, _: FrameContext) -> PortResult<()> {
        Ok(())
    }
    fn preview(&self) -> Option<PreviewToken> {
        None
    }
    fn capture(&mut self) -> PortResult<CapturedFrame> {
        Err("unused".into())
    }
    fn description(&self) -> &str {
        "test renderer"
    }
}
fn editor(window: &mut Window, cx: &mut Context<Editor>) -> Editor {
    let mut session = Session::new(
        Box::new(builtin_registry()),
        &Runtime,
        Box::new(JsonProjectStore::new(
            "/tmp/vjay-ui-test-unused.json".into(),
        )),
        Extent::new(960, 540).unwrap(),
    )
    .unwrap();
    session
        .restore(
            crate::demo::project(&builtin_registry())
                .unwrap()
                .document(),
        )
        .unwrap();
    Editor::new(Ok((session, None, None)), window, cx, false)
}
fn center(cx: &mut VisualTestContext, selector: &'static str) -> Point<Pixels> {
    cx.refresh().unwrap();
    cx.run_until_parked();
    cx.debug_bounds(selector)
        .unwrap_or_else(|| panic!("Missing {selector}"))
        .center()
}
fn drag(cx: &mut VisualTestContext, from: Point<Pixels>, to: Point<Pixels>) {
    cx.simulate_mouse_move(from, None, Modifiers::none());
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
}
fn connection(
    view: &Entity<Editor>,
    cx: &VisualTestContext,
    node: u64,
    input: usize,
) -> Option<Connection> {
    cx.read(|cx| {
        view.read(cx)
            .session()
            .project()
            .graph
            .connections()
            .find(|edge| edge.to == NodeId(node) && edge.input == input)
    })
}

#[gpui::test]
fn mouse_cables_connect_reconnect_reverse_and_cancel(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(editor);
    cx.simulate_resize(size(px(1440.0), px(940.0)));
    let from = center(cx, "out-1");
    let to = center(cx, "in-3-0");
    drag(cx, from, to);
    assert_eq!(connection(&view, cx, 3, 0).unwrap().from, NodeId(1));
    let from = center(cx, "in-2-0");
    let to = center(cx, "in-3-0");
    drag(cx, from, to);
    assert!(connection(&view, cx, 2, 0).is_none());
    let preserved = connection(&view, cx, 3, 0).unwrap();
    assert_eq!(preserved.from, NodeId(0));
    let from = center(cx, "out-3");
    let to = center(cx, "in-3-0");
    drag(cx, from, to);
    assert_eq!(connection(&view, cx, 3, 0), Some(preserved));
    let from = center(cx, "in-2-0");
    let to = center(cx, "out-1");
    drag(cx, from, to);
    assert_eq!(connection(&view, cx, 2, 0).unwrap().from, NodeId(1));
    let from = center(cx, "in-3-0");
    let to = center(cx, "out-1");
    drag(cx, from, to);
    let preserved = connection(&view, cx, 3, 0).unwrap();
    assert_eq!(preserved.from, NodeId(1));
    let from = center(cx, "in-3-0");
    drag(cx, from, from + point(px(70.0), px(-70.0)));
    assert_eq!(connection(&view, cx, 3, 0), Some(preserved));
    let from = center(cx, "in-3-0");
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(
        from + point(px(50.0), px(40.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.simulate_keystrokes("escape");
    assert!(cx.read(|cx| view.read(cx).cable.is_none()));
    cx.simulate_mouse_up(
        from + point(px(50.0), px(40.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    assert_eq!(connection(&view, cx, 3, 0), Some(preserved));
}

#[gpui::test]
fn all_panel_dividers_resize_with_mouse(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(editor);
    cx.simulate_resize(size(px(1440.0), px(940.0)));
    let initial = layout(&view, cx);
    let from = center(cx, "library-divider");
    drag(cx, from, from + point(px(40.0), px(0.0)));
    assert_eq!(
        layout(&view, cx).library_width,
        initial.library_width + 40.0
    );
    let from = center(cx, "inspector-divider");
    drag(cx, from, from - point(px(40.0), px(0.0)));
    assert_eq!(
        layout(&view, cx).inspector_width,
        initial.inspector_width + 40.0
    );
    let from = center(cx, "patch-divider");
    drag(cx, from, from + point(px(0.0), px(40.0)));
    assert_eq!(layout(&view, cx).top_height, initial.top_height + 40.0);
    let from = center(cx, "preview-divider");
    drag(cx, from, from + point(px(60.0), px(0.0)));
    assert!(layout(&view, cx).preview_ratio > initial.preview_ratio);
    cx.simulate_resize(size(px(1000.0), px(700.0)));
    cx.refresh().unwrap();
    cx.run_until_parked();
    let fitted = layout(&view, cx);
    assert!(1000.0 - fitted.library_width - fitted.inspector_width - 18.0 >= 400.0);
}

fn layout(view: &Entity<Editor>, cx: &VisualTestContext) -> WorkspaceLayout {
    cx.read(|cx| view.read(cx).session().project().workspace)
}

#[gpui::test]
fn right_click_wire_opens_compatible_insert_menu_and_splices_only_selected_edge(
    cx: &mut TestAppContext,
) {
    let (view, cx) = cx.add_window_view(editor);
    cx.simulate_resize(size(px(1440.0), px(940.0)));
    view.update(cx, |this, cx| {
        this.session_mut()
            .move_node(NodeId(0), Position { x: 20.0, y: 40.0 });
        this.session_mut()
            .move_node(NodeId(2), Position { x: 420.0, y: 60.0 });
        cx.notify();
    });
    cx.refresh().unwrap();
    cx.run_until_parked();
    let a = center(cx, "out-0");
    let b = center(cx, "in-2-0");
    let pointer = point((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
    cx.simulate_mouse_down(pointer, MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(pointer, MouseButton::Right, Modifiers::none());
    cx.run_until_parked();
    assert!(cx.read(|cx| view.read(cx).insert_menu.is_some()));
    let button = center(cx, "insert-Transform");
    cx.simulate_click(button, Modifiers::none());
    cx.run_until_parked();
    cx.read(|cx| {
        let editor = view.read(cx);
        let g = &editor.session().project().graph;
        let inserted = g.nodes().last().unwrap();
        assert_eq!(inserted.type_id, vjay_domain::builtins::ids::TRANSFORM);
        assert_eq!(g.connection_count(), 5);
        assert!(
            g.connections()
                .any(|e| e.from == NodeId(0) && e.to == inserted.id)
        );
        assert!(
            g.connections()
                .any(|e| e.from == inserted.id && e.to == NodeId(2))
        );
    });
}

#[gpui::test]
fn search_text_input_filters_nodes_and_does_not_trigger_patch_keyboard(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(editor);
    cx.simulate_resize(size(px(1440.0), px(940.0)));
    let point = center(cx, "text-input");
    cx.simulate_click(point, Modifiers::none());
    cx.refresh().unwrap();
    cx.run_until_parked();
    cx.simulate_input("glsl");
    cx.run_until_parked();
    cx.read(|cx| {
        let editor = view.read(cx);
        assert_eq!(editor.search.as_ref().unwrap().read(cx).text, "glsl");
        assert_eq!(editor.session().project().graph.nodes().len(), 5);
    });
}

#[gpui::test]
fn multiline_shader_editor_handles_unicode_selection_and_vertical_navigation(
    cx: &mut TestAppContext,
) {
    let (view, cx) = cx.add_window_view(|window, cx| {
        let field = text_field::TextField::new("aあ\n123\nxy".into(), true, cx);
        window.focus(&field.focus, cx);
        field
    });
    cx.simulate_resize(size(px(600.0), px(400.0)));
    cx.refresh().unwrap();
    cx.run_until_parked();
    cx.simulate_keystrokes("up home shift-right shift-right");
    cx.simulate_input("b");
    assert_eq!(cx.read(|cx| view.read(cx).text.clone()), "aあ\nb3\nxy");
    cx.simulate_keystrokes("cmd-a");
    cx.simulate_input("valid");
    assert_eq!(cx.read(|cx| view.read(cx).text.clone()), "valid");
}

mod extensions;
mod keyboard;
mod midi;
mod parameters;
mod selection;

#[gpui::test]
fn node_browser_and_status_fit_small_window_without_expanding_the_canvas(cx: &mut TestAppContext) {
    let (_, cx) = cx.add_window_view(editor);
    cx.simulate_resize(size(px(1000.0), px(700.0)));
    cx.refresh().unwrap();
    cx.run_until_parked();
    let list = cx.debug_bounds("node-results").unwrap();
    let status = cx.debug_bounds("status-bar").unwrap();
    assert!(
        list.bottom() <= status.top(),
        "list viewport must leave room for status: {list:?} {status:?}"
    );
    assert!(status.bottom() <= px(700.0));
}

mod menus;
mod studio;

mod viewport;

mod settings;

mod readout;
mod subpatch;
