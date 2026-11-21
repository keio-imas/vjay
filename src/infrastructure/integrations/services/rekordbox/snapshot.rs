use rusqlite::{Connection, OpenFlags};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::SystemTime,
};
use vjay_application::ports::PortResult;
static NEXT: AtomicU64 = AtomicU64::new(0);
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Fingerprint(Vec<Option<(u64, SystemTime)>>);
pub(super) fn fingerprint(path: &Path) -> PortResult<Fingerprint> {
    let mut values = Vec::new();
    for source in [path.to_path_buf(), sidecar(path, "-wal")] {
        match fs::metadata(source) {
            Ok(m) => values.push(Some((m.len(), m.modified().map_err(|e| e.to_string())?))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => values.push(None),
            Err(e) => return Err(e.to_string()),
        }
    }
    if values[0].is_none() {
        return Err("rekordbox master.db was not found".into());
    }
    Ok(Fingerprint(values))
}
fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    name.into()
}
struct Directory(PathBuf);
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
pub(super) struct Snapshot {
    pub connection: Connection,
    _directory: Directory,
}
impl Snapshot {
    pub fn open(source: &Path, key: Option<&str>) -> PortResult<Self> {
        let before = fingerprint(source)?;
        let root = std::env::temp_dir().join(format!(
            "vjay-rekordbox-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&root).map_err(|e| e.to_string())?;
        let result = (|| {
            let target = root.join("master.db");
            copy_read_only(source, &target)?;
            let wal = sidecar(source, "-wal");
            if wal.is_file() {
                copy_read_only(&wal, &sidecar(&target, "-wal"))?;
            }
            if before != fingerprint(source)? {
                return Err("rekordbox database changed during snapshot; retrying".into());
            }
            let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
            let mut last = None;
            for compatibility in [4, 3] {
                let connection =
                    Connection::open_with_flags(&target, flags).map_err(|e| e.to_string())?;
                if let Some(key) = key {
                    connection
                        .pragma_update(None, "key", key)
                        .map_err(|_| "Cannot unlock database snapshot")?;
                    connection
                        .pragma_update(None, "cipher_compatibility", compatibility)
                        .map_err(|_| "SQLCipher compatibility unavailable")?;
                }
                match connection.query_row("SELECT count(*) FROM sqlite_master", [], |r| {
                    r.get::<_, i64>(0)
                }) {
                    Ok(_) => {
                        connection
                            .pragma_update(None, "query_only", true)
                            .map_err(|e| e.to_string())?;
                        connection
                            .pragma_update(None, "trusted_schema", false)
                            .map_err(|e| e.to_string())?;
                        return Ok(connection);
                    }
                    Err(error) => last = Some(error),
                }
            }
            Err(format!("Cannot read database snapshot: {}", last.unwrap()))
        })();
        match result {
            Ok(connection) => Ok(Self {
                connection,
                _directory: Directory(root),
            }),
            Err(error) => {
                let _ = fs::remove_dir_all(root);
                Err(error)
            }
        }
    }
}
fn copy_read_only(source: &Path, target: &Path) -> PortResult<()> {
    // The source is NEVER handed to SQLite, OpenOptions(write), or a repair API.
    let input = File::open(source).map_err(|e| e.to_string())?;
    if input.metadata().map_err(|e| e.to_string())?.len() > 1024 * 1024 * 1024 {
        return Err("Database snapshot exceeds the 1 GiB per-file limit".into());
    }
    let mut output = File::create_new(target).map_err(|e| e.to_string())?;
    std::io::copy(&mut input.take(1024 * 1024 * 1024 + 1), &mut output)
        .map_err(|e| e.to_string())?;
    output.flush().map_err(|e| e.to_string())
}
