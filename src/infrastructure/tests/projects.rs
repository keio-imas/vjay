use vjay_application::{compiler::RenderPlan, ports::*, project::Point, session::Session};
use vjay_domain::builtins::registry;
use vjay_infrastructure::storage::JsonProjectStore;
struct Runtime;
struct Renderer;
struct Clock;
impl vjay_application::ports::Clock for Clock {
    fn seconds(&self) -> f64 {
        0.0
    }
}
impl RuntimeFactory for Runtime {
    fn create_renderer(&self) -> PortResult<Box<dyn RenderBackend>> {
        Ok(Box::new(Renderer))
    }
    fn create_clock(&self) -> Box<dyn vjay_application::ports::Clock> {
        Box::new(Clock)
    }
}
impl RenderBackend for Renderer {
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
        "test"
    }
}
#[test]
fn open_save_as_and_new_keep_separate_files_and_invalid_open_is_atomic() {
    let temp = tempfile::tempdir().unwrap();
    let a = temp.path().join("a.vjay.json");
    let b = temp.path().join("b.vjay.json");
    let bad = temp.path().join("invalid.vjay.json");
    let mut session = Session::new(
        Box::new(registry()),
        &Runtime,
        Box::new(JsonProjectStore::new(temp.path().join("untitled.json"))),
        Extent::new(64, 40).unwrap(),
    )
    .unwrap();
    assert!(session.is_untitled());
    session
        .add_node("Constant", Point { x: 0.0, y: 0.0 })
        .unwrap();
    assert!(session.project_modified());
    session.save_project_as(a.clone()).unwrap();
    assert!(!session.project_modified());
    let original = std::fs::read(&a).unwrap();
    session
        .add_node("TextValue", Point { x: 250.0, y: 0.0 })
        .unwrap();
    session.save_project_as(b.clone()).unwrap();
    assert_eq!(std::fs::read(&a).unwrap(), original);
    assert_eq!(session.store_location(), b);
    assert_eq!(session.project().graph.nodes().len(), 3);
    session.open_project(a.clone()).unwrap();
    assert_eq!(session.project().graph.nodes().len(), 2);
    assert_eq!(session.store_location(), a);
    assert!(!session.is_untitled());
    std::fs::write(&bad, b"invalid json").unwrap();
    assert!(session.open_project(bad).is_err());
    assert_eq!(session.store_location(), a);
    assert_eq!(session.project().graph.nodes().len(), 2);
    session.new_project().unwrap();
    assert!(session.is_untitled());
    assert_eq!(session.project().graph.nodes().len(), 1);
    assert_eq!(std::fs::read(&a).unwrap(), original);
}

use vjay_domain::graph::Connection;
#[test]
fn named_patches_save_reload_and_instantiate_without_writing_the_source_implicitly() {
    let temp = tempfile::tempdir().unwrap();
    let main = temp.path().join("main.vjay.json");
    let module = temp.path().join("module.vjay.patch.json");
    let mut s = Session::new(
        Box::new(registry()),
        &Runtime,
        Box::new(JsonProjectStore::new(main.clone())),
        Extent::new(64, 40).unwrap(),
    )
    .unwrap();
    let value = s.add_node("Constant", Point { x: 20.0, y: 20.0 }).unwrap();
    s.set_parameter(value, 0, 0.25).unwrap();
    let multiply = s.add_node("Multiply", Point { x: 280.0, y: 20.0 }).unwrap();
    s.set_parameter(multiply, 1, 2.0).unwrap();
    let sink = s.add_node("Add", Point { x: 540.0, y: 20.0 }).unwrap();
    for e in [
        Connection {
            from: value,
            output: 0,
            to: multiply,
            input: 0,
        },
        Connection {
            from: multiply,
            output: 0,
            to: sink,
            input: 0,
        },
    ] {
        s.connect(e).unwrap();
    }
    let patch = s.encapsulate(&[multiply]).unwrap();
    s.save_subpatch_as(Some(patch), module.clone()).unwrap();
    let original = std::fs::read(&module).unwrap();
    s.render_at(0.0).unwrap();
    assert_eq!(s.signal_value(sink, 0), Some(0.5));
    s.enter_subpatch(patch).unwrap();
    s.set_parameter(multiply, 1, 3.0).unwrap();
    s.save_project_as(main.clone()).unwrap();
    assert_eq!(std::fs::read(&module).unwrap(), original);
    s.leave_subpatch().unwrap();
    s.render_at(0.1).unwrap();
    assert_eq!(s.signal_value(sink, 0), Some(0.75));
    s.reload_subpatch(patch).unwrap();
    s.render_at(0.2).unwrap();
    assert_eq!(s.signal_value(sink, 0), Some(0.5));
    assert_eq!(std::fs::read(&module).unwrap(), original);
    let copy = s
        .load_subpatch_file(module.clone(), Point { x: 300.0, y: 300.0 })
        .unwrap();
    assert_ne!(patch, copy);
    s.open_project(main.clone()).unwrap();
    assert_eq!(s.project().subpatches.len(), 1);
    s.render_at(0.3).unwrap();
    assert_eq!(s.signal_value(sink, 0), Some(0.75));
    std::fs::write(&module, b"broken").unwrap();
    assert!(s.reload_subpatch(patch).is_err());
    s.render_at(0.4).unwrap();
    assert_eq!(s.signal_value(sink, 0), Some(0.75));
}
#[test]
fn standalone_patch_files_can_open_as_projects_without_an_unknown_inlet_source() {
    let temp = tempfile::tempdir().unwrap();
    let module = temp.path().join("standalone.vjay.patch.json");
    let mut s = Session::new(
        Box::new(registry()),
        &Runtime,
        Box::new(JsonProjectStore::new(temp.path().join("root.json"))),
        Extent::new(64, 40).unwrap(),
    )
    .unwrap();
    let a = s.add_node("Constant", Point { x: 0.0, y: 0.0 }).unwrap();
    let b = s.add_node("Multiply", Point { x: 250.0, y: 0.0 }).unwrap();
    let c = s.add_node("Add", Point { x: 500.0, y: 0.0 }).unwrap();
    s.connect(Connection {
        from: a,
        output: 0,
        to: b,
        input: 0,
    })
    .unwrap();
    s.connect(Connection {
        from: b,
        output: 0,
        to: c,
        input: 0,
    })
    .unwrap();
    let patch = s.encapsulate(&[b]).unwrap();
    s.save_subpatch_as(Some(patch), module.clone()).unwrap();
    s.open_project(module).unwrap();
    s.render_at(0.0).unwrap();
    assert!(s.project().graph.nodes().iter().any(|n| n.type_id == "In"));
}
