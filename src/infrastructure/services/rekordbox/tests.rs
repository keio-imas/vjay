use super::*;
use rusqlite::Connection;
use std::{fs, path::Path, time::SystemTime};
fn fixture() -> PathBuf {
    std::env::temp_dir().join(format!(
        "vjay-history-test-{}-{}.db",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}
fn schema(db: &Connection) {
    db.execute_batch("CREATE TABLE djmdArtist(ID TEXT,Name TEXT);CREATE TABLE djmdAlbum(ID TEXT,Name TEXT);
    CREATE TABLE djmdGenre(ID TEXT,Name TEXT);CREATE TABLE djmdLabel(ID TEXT,Name TEXT);CREATE TABLE djmdKey(ID TEXT,ScaleName TEXT);
    CREATE TABLE djmdContent(ID TEXT,Title TEXT,ArtistID TEXT,AlbumID TEXT,GenreID TEXT,LabelID TEXT,KeyID TEXT,FolderPath TEXT,BPM INTEGER,Length INTEGER,rb_local_deleted INTEGER);
    CREATE TABLE djmdSongHistory(ID TEXT,ContentID TEXT,created_at TEXT,rb_local_deleted INTEGER);
    INSERT INTO djmdArtist VALUES('a','Test artist');
    INSERT INTO djmdContent VALUES('4294967295','Test title','a',NULL,NULL,NULL,NULL,'/test.mp3',12850,180,0);
    INSERT INTO djmdSongHistory VALUES('h','4294967295','2026-11-18 12:00:00',0);").unwrap();
}
fn bytes(path: &Path) -> (Vec<u8>, SystemTime) {
    (
        fs::read(path).unwrap(),
        fs::metadata(path).unwrap().modified().unwrap(),
    )
}
#[test]
fn plaintext_wal_snapshot_sees_commits_and_never_writes_source() {
    let path = fixture();
    let db = Connection::open(&path).unwrap();
    db.pragma_update(None, "journal_mode", "WAL").unwrap();
    db.pragma_update(None, "wal_autocheckpoint", 0).unwrap();
    schema(&db);
    let wal = PathBuf::from(format!("{}-wal", path.display()));
    let shm = PathBuf::from(format!("{}-shm", path.display()));
    let original = [bytes(&path), bytes(&wal), bytes(&shm)];
    let snapshot = snapshot::Snapshot::open(&path, None).unwrap();
    let track = query::latest(&snapshot.connection).unwrap().unwrap();
    assert_eq!(track.text[0].as_ref(), "Test title");
    assert_eq!(track.text[7].as_ref(), "4294967295");
    assert_eq!(track.bpm, 128.5);
    assert!(
        snapshot
            .connection
            .execute("DELETE FROM djmdContent", [])
            .is_err()
    );
    drop(snapshot);
    assert_eq!(original, [bytes(&path), bytes(&wal), bytes(&shm)]);
    db.execute(
        "INSERT INTO djmdSongHistory VALUES('h2','4294967295','later',0)",
        [],
    )
    .unwrap();
    let snapshot = snapshot::Snapshot::open(&path, None).unwrap();
    assert_eq!(query::latest(&snapshot.connection).unwrap().unwrap().row, 2);
    drop(snapshot);
    drop(db);
    fs::remove_file(path).unwrap();
}
#[test]
fn encrypted_snapshot_requires_key_and_preserves_source() {
    let path = fixture();
    let db = Connection::open(&path).unwrap();
    db.pragma_update(None, "key", "fixture-only-password")
        .unwrap();
    schema(&db);
    drop(db);
    let original = bytes(&path);
    assert!(snapshot::Snapshot::open(&path, Some("wrong-key")).is_err());
    let snapshot = snapshot::Snapshot::open(&path, Some("fixture-only-password")).unwrap();
    assert_eq!(
        query::latest(&snapshot.connection).unwrap().unwrap().text[1].as_ref(),
        "Test artist"
    );
    assert!(
        snapshot
            .connection
            .execute("PRAGMA user_version=123", [])
            .is_err()
    );
    drop(snapshot);
    assert_eq!(original, bytes(&path));
    fs::remove_file(path).unwrap();
}
#[test]
fn malformed_database_is_unchanged() {
    let path = fixture();
    fs::write(&path, b"not a database").unwrap();
    let original = bytes(&path);
    assert!(snapshot::Snapshot::open(&path, None).is_err());
    assert_eq!(original, bytes(&path));
    fs::remove_file(path).unwrap();
}
#[test]
#[ignore = "reads the installed rekordbox library without writing it"]
fn installed_library_read_only() {
    let paths = RekordboxPaths::discover();
    let (path, key) = config::credentials(&paths.options).unwrap();
    let original = bytes(&path);
    let snapshot = snapshot::Snapshot::open(&path, Some(&key)).unwrap();
    let _ = query::latest(&snapshot.connection).unwrap();
    drop(snapshot);
    assert_eq!(original, bytes(&path));
}

#[test]
fn history_worker_reports_latest_then_pulses_only_for_a_new_wal_row() {
    let path = fixture();
    let db = Connection::open(&path).unwrap();
    db.pragma_update(None, "journal_mode", "WAL").unwrap();
    schema(&db);
    let factory = RekordboxFactory {
        paths: RekordboxPaths {
            database: path.clone(),
            options: path.with_extension("missing-options"),
        },
    };
    let mut service = factory
        .create(&ServiceRequest {
            node: NodeId(8),
            service: "rekordbox-history",
            properties: std::collections::BTreeMap::from([("poll_seconds".into(), "0.25".into())]),
        })
        .unwrap();
    let wait = |service: &mut Box<dyn NodeService>, changed: f32| {
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        let mut events = Vec::new();
        loop {
            events.clear();
            service.poll(NodeId(8), &mut events);
            if events.iter().any(|e|matches!(e,ControlEvent::NodeValues{values,..} if values[3]==1.0 && values[2]==changed)) {break events;}
            assert!(
                std::time::Instant::now() < deadline,
                "history worker timeout: {events:?}"
            );
            thread::sleep(Duration::from_millis(10));
        }
    };
    let events = wait(&mut service, 0.0);
    assert!(events.iter().any(
        |e| matches!(e,ControlEvent::NodeText{values,..} if values[0].as_ref()=="Test title")
    ));
    db.execute_batch("UPDATE djmdContent SET Title='Next title'; INSERT INTO djmdSongHistory VALUES('h2','4294967295','later',0);").unwrap();
    let original = bytes(&path);
    let wal = PathBuf::from(format!("{}-wal", path.display()));
    let original_wal = bytes(&wal);
    let events = wait(&mut service, 1.0);
    assert!(events.iter().any(
        |e| matches!(e,ControlEvent::NodeText{values,..} if values[0].as_ref()=="Next title")
    ));
    let mut next = Vec::new();
    service.poll(NodeId(8), &mut next);
    assert!(
        next.iter()
            .any(|e| matches!(e,ControlEvent::NodeValues{values,..} if values[2]==0.0))
    );
    drop(service);
    assert_eq!(original, bytes(&path));
    assert_eq!(original_wal, bytes(&wal));
    drop(db);
    fs::remove_file(path).unwrap();
}

#[test]
fn artwork_column_is_optional_and_queries_never_write_source_database() {
    let path = fixture();
    let db = Connection::open(&path).unwrap();
    schema(&db);
    db.execute_batch("ALTER TABLE djmdContent ADD COLUMN ImagePath TEXT;UPDATE djmdContent SET ImagePath='share/PIONEER/Artwork/cover.jpg';").unwrap();
    drop(db);
    let original = bytes(&path);
    let snapshot = snapshot::Snapshot::open(&path, None).unwrap();
    let track = query::latest(&snapshot.connection).unwrap().unwrap();
    assert_eq!(
        track.cover.unwrap(),
        PathBuf::from("share/PIONEER/Artwork/cover.jpg")
    );
    drop(snapshot);
    assert_eq!(bytes(&path), original);
    fs::remove_file(path).unwrap();
}
