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
