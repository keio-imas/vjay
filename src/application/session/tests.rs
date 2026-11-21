use crate::{compiler::RenderPlan, ports::*, project::*, session::Session};
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use vjay_domain::builtins::ids;
use vjay_domain::{
    builtins::registry as builtin_registry,
    graph::NodeId,
    node::{Node, NodeFactory},
};

#[derive(Default)]
struct Counts {
    prepares: usize,
    duration: Option<f64>,
    frames: usize,
    fail_prepare: bool,
}
struct Renderer(Rc<RefCell<Counts>>);
impl RenderBackend for Renderer {
    fn media_duration(&self, _: NodeId) -> Option<f64> {
        self.0.borrow().duration
    }

    fn prepare(&mut self, _: &RenderPlan, _: Extent) -> PortResult<()> {
        let mut counts = self.0.borrow_mut();
        counts.prepares += 1;
        if counts.fail_prepare {
            return Err("Simulated GPU preparation failure".into());
        }
        Ok(())
    }
    fn render(&mut self, _: &RenderPlan, _: FrameContext) -> PortResult<()> {
        self.0.borrow_mut().frames += 1;
        Ok(())
    }
    fn preview(&self) -> Option<PreviewToken> {
        None
    }
    fn capture(&mut self) -> PortResult<CapturedFrame> {
        Err("test renderer".into())
    }
    fn description(&self) -> &str {
        "test"
    }
}
struct FixedClock;
impl Clock for FixedClock {
    fn seconds(&self) -> f64 {
        0.0
    }
}
struct Runtime(Rc<RefCell<Counts>>);
impl RuntimeFactory for Runtime {
    fn create_renderer(&self) -> PortResult<Box<dyn RenderBackend>> {
        Ok(Box::new(Renderer(self.0.clone())))
    }
    fn create_clock(&self) -> Box<dyn Clock> {
        Box::new(FixedClock)
    }
}
struct CountingFactory(Arc<AtomicUsize>, vjay_domain::registry::NodeRegistry);
impl NodeFactory for CountingFactory {
    fn get(&self, type_id: &str) -> Result<&dyn Node, vjay_domain::registry::NodeError> {
        self.0.fetch_add(1, Ordering::Relaxed);
        self.1.get(type_id)
    }
    fn catalog(&self) -> &[&'static vjay_domain::node::NodeDescriptor] {
        self.1.catalog()
    }
}
struct Store {
    doc: RefCell<ProjectDocument>,
    path: PathBuf,
}
impl ProjectStore for Store {
    fn load(&self) -> PortResult<ProjectDocument> {
        Ok(self.doc.borrow().clone())
    }
    fn save(&self, doc: &ProjectDocument) -> PortResult<()> {
        *self.doc.borrow_mut() = doc.clone();
        Ok(())
    }
    fn location(&self) -> &Path {
        &self.path
    }
}
fn fixture(doc: ProjectDocument) -> (Session, Rc<RefCell<Counts>>, Arc<AtomicUsize>) {
    let counts = Rc::new(RefCell::new(Counts::default()));
    let calls = Arc::new(AtomicUsize::new(0));
    let mut session = Session::new(
        Box::new(CountingFactory(calls.clone(), builtin_registry())),
        &Runtime(counts.clone()),
        Box::new(Store {
            doc: RefCell::new(doc),
            path: "test.json".into(),
        }),
        Extent::new(640, 360).unwrap(),
    )
    .unwrap();
    session
        .restore(Project::demo(&builtin_registry()).unwrap().document())
        .unwrap();
    (session, counts, calls)
}

#[test]
fn steady_frames_do_not_traverse_nodes_and_parameter_edits_do_not_reallocate() {
    let doc = Project::demo(&builtin_registry()).unwrap().document();
    let (mut session, counts, calls) = fixture(doc);
    session.render_at(0.0).unwrap();
    let initial = calls.load(Ordering::Relaxed);
    session.render_at(0.5).unwrap();
    session.render_at(1.0).unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), initial);
    session.set_parameter(NodeId(2), 0, 0.75).unwrap();
    let after_edit = calls.load(Ordering::Relaxed);
    session.render_at(1.5).unwrap();
    assert_eq!(
        calls.load(Ordering::Relaxed),
        after_edit,
        "parameter edits must not rebuild schedules"
    );
    assert_eq!(counts.borrow().prepares, 1);
    assert_eq!(counts.borrow().frames, 4);
    session.set_extent(Extent::new(1280, 720).unwrap()).unwrap();
    session.render_at(2.0).unwrap();
    assert_eq!(counts.borrow().prepares, 2);
}

#[test]
fn incomplete_patch_freezes_output_and_recovers_after_reconnection() {
    let doc = Project::demo(&builtin_registry()).unwrap().document();
    let (mut session, counts, calls) = fixture(doc.clone());
    session.render_at(0.0).unwrap();
    session.disconnect(NodeId(2), 0);
    assert!(session.render_at(0.1).is_err());
    let failed_calls = calls.load(Ordering::Relaxed);
    assert!(session.render_at(0.2).is_err());
    assert_eq!(calls.load(Ordering::Relaxed), failed_calls);
    assert_eq!(counts.borrow().frames, 1);
    session.connect(doc.connections[0]).unwrap();
    session.render_at(0.3).unwrap();
    assert_eq!(counts.borrow().frames, 2);
}

#[test]
fn invalid_load_does_not_replace_live_project() {
    let mut doc = Project::demo(&builtin_registry()).unwrap().document();
    doc.nodes[0].parameters[0] = -999.0;
    let (mut session, _, _) = fixture(doc);
    session.set_parameter(NodeId(0), 0, 20.0).unwrap();
    assert!(session.load().is_err());
    assert_eq!(
        session.project().graph.node(NodeId(0)).unwrap().parameters[0],
        20.0
    );
    session.render_at(0.0).unwrap();
}

#[test]
fn video_import_stays_disconnected_and_replacement_preserves_connections() {
    let doc = Project::demo(&builtin_registry()).unwrap().document();
    let (mut session, _, _) = fixture(doc);
    let id = session.import_video("/clips/a.mp4".into(), None).unwrap();
    assert_eq!(
        session.project().graph.node(id).unwrap().type_id,
        ids::VIDEO
    );
    assert_eq!(session.project().assets.len(), 1);
    let connections = session.project().graph.connections().collect::<Vec<_>>();
    assert!(!connections.iter().any(|e| e.from == id || e.to == id));
    assert_eq!(connections.len(), 4);
    session
        .import_video("/clips/b.mp4".into(), Some(id))
        .unwrap();
    assert!(session.project().graph.connections().eq(connections));
    assert_eq!(session.project().assets.len(), 1);
    assert_eq!(
        session.project().assets[0].path,
        PathBuf::from("/clips/b.mp4")
    );
    let document = session.project().document();
    assert!(Project::restore(document, &builtin_registry()).is_ok());
    session.remove_node(id).unwrap();
    assert!(session.project().assets.is_empty());
}

#[test]
fn new_session_nodes_and_widgets_never_create_connections_and_bindings_roundtrip() {
    let (mut session, _, _) = fixture(Project::empty(&builtin_registry()).unwrap().document());
    session.load().unwrap();
    assert!(session.project().graph.connection_count() == 0);
    assert!(session.project().controls.is_empty());
    for widget in WidgetKind::ALL {
        session.add_control(widget).unwrap();
    }
    let noise = session
        .add_node(ids::NOISE, Point { x: 10.0, y: 20.0 })
        .unwrap();
    session.import_video("/clips/a.mp4".into(), None).unwrap();
    assert!(session.project().graph.connection_count() == 0);
    let mut control = session.project().controls[0].clone();
    control.node = noise;
    control.parameter = 1;
    control.label = "Movement".into();
    control.range = Some([0.0, 5.0]);
    session.configure_control(control).unwrap();
    session.save().unwrap();
    session.load().unwrap();
    assert_eq!(session.project().controls.len(), WidgetKind::ALL.len());
    assert_eq!(session.project().controls[0].node, noise);
    assert_eq!(session.project().controls[0].label, "Movement");
    assert!(session.project().graph.connection_count() == 0);
}
#[test]
fn explicit_wire_insertion_is_atomic_and_preserves_the_original_output_port() {
    let (mut session, _, _) = fixture(Project::demo(&builtin_registry()).unwrap().document());
    let edge = session.project().graph.connections().next().unwrap();
    assert!(
        session
            .insert_node(edge, ids::AUDIO_GAIN, Point { x: 0.0, y: 0.0 })
            .is_err()
    );
    assert!(session.project().graph.has_connection(edge));
    let id = session
        .insert_node(edge, ids::INVERT, Point { x: 100.0, y: 100.0 })
        .unwrap();
    let connections = session.project().graph.connections().collect::<Vec<_>>();
    assert!(!connections.contains(&edge));
    assert!(
        connections
            .iter()
            .any(|e| e.from == edge.from && e.output == edge.output && e.to == id)
    );
    assert!(
        connections
            .iter()
            .any(|e| e.from == id && e.to == edge.to && e.input == edge.input)
    );
    session.render_at(0.0).unwrap();
}

#[test]
fn unavailable_optional_audio_does_not_interrupt_program_rendering() {
    let (mut session, counts, _) = fixture(Project::demo(&builtin_registry()).unwrap().document());
    let video = session
        .import_video("/fixtures/clip.mov".into(), None)
        .unwrap();
    let out = session
        .add_node(ids::AUDIO_OUT, Point { x: 0.0, y: 0.0 })
        .unwrap();
    session
        .connect(vjay_domain::graph::Connection {
            from: video,
            output: 1,
            to: out,
            input: 0,
        })
        .unwrap();
    session.render_at(0.0).unwrap();
    assert!(session.audio_error.is_some());
    assert_eq!(counts.borrow().frames, 1);
}

mod deletion;

mod modulation;
mod studio;

mod workflow;

mod deck;
mod preferences;

mod anime_club;
mod readout;
mod subpatch;
mod transport;
