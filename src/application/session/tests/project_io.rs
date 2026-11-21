use super::*;
use std::sync::{Mutex, atomic::AtomicBool};
struct Files {
    doc: Mutex<ProjectDocument>,
    fail: AtomicBool,
    calls: AtomicUsize,
}
impl ProjectFiles for Files {
    fn read(&self, _: &Path) -> PortResult<ProjectDocument> {
        Ok(self.doc.lock().unwrap().clone())
    }
    fn write(&self, _: &Path, document: &ProjectDocument) -> PortResult<()> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        if self.fail.load(Ordering::Relaxed) {
            return Err("Simulated write failure".into());
        }
        *self.doc.lock().unwrap() = document.clone();
        Ok(())
    }
}
struct FileStore {
    path: PathBuf,
    files: Arc<Files>,
}
impl ProjectStore for FileStore {
    fn background_io(&self) -> Option<Arc<dyn ProjectFiles>> {
        Some(self.files.clone())
    }
    fn load(&self) -> PortResult<ProjectDocument> {
        self.files.read(&self.path)
    }
    fn save(&self, doc: &ProjectDocument) -> PortResult<()> {
        self.files.write(&self.path, doc)
    }
    fn location(&self) -> &Path {
        &self.path
    }
    fn set_location(&mut self, path: PathBuf) -> PortResult<()> {
        self.path = path;
        Ok(())
    }
}
fn session() -> (Session, Arc<Files>) {
    let doc = Project::demo(&builtin_registry()).unwrap().document();
    let files = Arc::new(Files {
        doc: Mutex::new(doc.clone()),
        fail: AtomicBool::new(false),
        calls: AtomicUsize::new(0),
    });
    let mut session = Session::new(
        Box::new(builtin_registry()),
        &Runtime(Rc::new(RefCell::new(Counts::default()))),
        Box::new(FileStore {
            path: "original.json".into(),
            files: files.clone(),
        }),
        Extent::new(320, 180).unwrap(),
    )
    .unwrap();
    session.restore(doc).unwrap();
    (session, files)
}
#[test]
fn background_save_preserves_edits_made_while_the_snapshot_is_written() {
    let (mut s, files) = session();
    s.set_parameter(NodeId(0), 0, 10.0).unwrap();
    let request = s.prepare_save(None).unwrap();
    assert_eq!(files.calls.load(Ordering::Relaxed), 0);
    s.set_parameter(NodeId(0), 0, 11.0).unwrap();
    let saved = std::thread::spawn(move || request.execute())
        .join()
        .unwrap()
        .unwrap();
    assert!(!s.finish_save(saved).unwrap());
    assert!(s.project_modified());
    assert_eq!(files.doc.lock().unwrap().nodes[0].parameters[0], 10.0);
    assert_eq!(
        s.project().graph.node(NodeId(0)).unwrap().parameters[0],
        11.0
    );
    let saved = s
        .prepare_save(Some("new.json".into()))
        .unwrap()
        .execute()
        .unwrap();
    assert!(s.finish_save(saved).unwrap());
    assert!(!s.project_modified());
    assert_eq!(s.store_location(), Path::new("new.json"));
    assert!(!s.is_untitled());
}
#[test]
fn failed_or_stale_background_operations_preserve_the_live_project_and_path() {
    let (mut s, files) = session();
    s.set_parameter(NodeId(0), 0, 10.0).unwrap();
    files.fail.store(true, Ordering::Relaxed);
    assert!(
        s.prepare_save(Some("failed.json".into()))
            .unwrap()
            .execute()
            .is_err()
    );
    assert_eq!(s.store_location(), Path::new("original.json"));
    assert!(s.project_modified());
    files.fail.store(false, Ordering::Relaxed);
    let open = s
        .prepare_open("other.json".into())
        .unwrap()
        .execute()
        .unwrap();
    s.set_parameter(NodeId(0), 0, 12.0).unwrap();
    assert!(s.finish_open(open).is_err());
    assert_eq!(
        s.project().graph.node(NodeId(0)).unwrap().parameters[0],
        12.0
    );
    let saved = s
        .prepare_save(Some("stale.json".into()))
        .unwrap()
        .execute()
        .unwrap();
    s.new_project().unwrap();
    assert!(!s.finish_save(saved).unwrap());
    assert!(s.is_untitled());
    assert_eq!(s.store_location(), Path::new("original.json"));
    s.finish_open(
        s.prepare_open("opened.json".into())
            .unwrap()
            .execute()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(s.store_location(), Path::new("opened.json"));
    assert!(!s.project_modified());
}
