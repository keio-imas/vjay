use super::*;
use crate::preferences::{Preferences, SettingsStore};
struct FailingStore;
impl SettingsStore for FailingStore {
    fn load(&self) -> PortResult<Preferences> {
        Ok(Preferences::default())
    }
    fn save(&self, _: &Preferences) -> PortResult<()> {
        Err("Read-only settings location".into())
    }
}
struct SettingsRuntime(Runtime);
impl RuntimeFactory for SettingsRuntime {
    fn create_renderer(&self) -> PortResult<Box<dyn RenderBackend>> {
        self.0.create_renderer()
    }
    fn create_clock(&self) -> Box<dyn Clock> {
        self.0.create_clock()
    }
    fn settings_store(&self) -> Arc<dyn SettingsStore> {
        Arc::new(FailingStore)
    }
}
#[test]
fn invalid_and_unsaved_settings_leave_live_configuration_unchanged() {
    let mut session = Session::new(
        Box::new(builtin_registry()),
        &SettingsRuntime(Runtime(Default::default())),
        Box::new(Store {
            doc: RefCell::new(Project::empty(&builtin_registry()).unwrap().document()),
            path: "test.json".into(),
        }),
        Extent::new(640, 360).unwrap(),
    )
    .unwrap();
    let initial = session.preferences().clone();
    let mut settings = initial.clone();
    settings.video.width = 0;
    assert!(session.apply_preferences(settings).is_err());
    assert_eq!(session.preferences(), &initial);
    let mut settings = initial.clone();
    settings.video.width = 1920;
    settings.audio.volume = 0.4;
    assert!(
        session
            .apply_preferences(settings)
            .unwrap_err()
            .contains("Read-only")
    );
    assert_eq!(session.preferences(), &initial);
    assert_eq!(session.extent(), Extent::new(640, 360).unwrap());
}
#[test]
fn settings_are_independent_of_project_files_and_validate_names() {
    let (mut s, _, _) = fixture(Project::demo(&builtin_registry()).unwrap().document());
    let doc = s.project().document();
    let mut p = Preferences::default();
    p.video.width = 1920;
    p.video.height = 1080;
    p.video.frame_rate = 30;
    p.default_project_name = "Stage A.vjay.json".into();
    s.apply_preferences(p.clone()).unwrap();
    assert_eq!(s.preferences(), &p);
    assert_eq!(s.extent(), Extent::new(1920, 1080).unwrap());
    assert_eq!(
        s.frame_interval(),
        std::time::Duration::from_secs_f64(1.0 / 30.0)
    );
    s.restore(doc).unwrap();
    assert_eq!(s.preferences(), &p);
    for name in ["", "../escape.json", "a/b", "a\\b", "a\nb"] {
        let mut invalid = p.clone();
        invalid.default_project_name = name.into();
        assert!(invalid.validate().is_err());
    }
}
