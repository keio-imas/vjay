use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};
use vjay_application::{ports::ProjectStore, project::Project};
use vjay_domain::builtins::registry as builtin_registry;
use vjay_infrastructure::storage::JsonProjectStore;

#[test]
fn json_roundtrip_and_atomic_replacement() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("vjay-test-{}-{nonce}", std::process::id()));
    fs::create_dir(&dir).unwrap();
    let store = JsonProjectStore::new(dir.join("project.json"));
    let project = vjay_infrastructure::demo::project(&builtin_registry()).unwrap();
    store.save(&project.document()).unwrap();
    let mut replacement = project.document();
    replacement.nodes[0].parameters[0] = 12.0;
    store.save(&replacement).unwrap();
    let loaded = store.load().unwrap();
    assert_eq!(loaded.nodes[0].parameters[0], 12.0);
    assert!(Project::restore(loaded, &builtin_registry()).is_ok());
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
    fs::write(store.location(), b"{ invalid json").unwrap();
    assert!(store.load().is_err());
    fs::remove_dir_all(dir).unwrap();
}
