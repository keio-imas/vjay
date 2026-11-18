use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use vjay_application::{
    ports::{CapturedFrame, PortResult, ProjectStore},
    project::ProjectDocument,
};

pub struct JsonProjectStore {
    path: PathBuf,
}
impl JsonProjectStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl ProjectStore for JsonProjectStore {
    fn load_from(&self, path: &Path) -> PortResult<ProjectDocument> {
        Self::new(path.to_owned()).load()
    }
    fn save_to(&self, path: &Path, document: &ProjectDocument) -> PortResult<()> {
        Self::new(path.to_owned()).save(document)
    }
    fn set_location(&mut self, path: PathBuf) -> PortResult<()> {
        self.path = path;
        Ok(())
    }
    fn load(&self) -> PortResult<ProjectDocument> {
        let file = fs::File::open(&self.path).map_err(|e| e.to_string())?;
        let mut bytes = Vec::new();
        file.take(64 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > 64 * 1024 * 1024 {
            return Err("Project is larger than 64 MiB".into());
        }
        serde_json::from_slice(&bytes).map_err(|e| e.to_string())
    }
    fn save(&self, document: &ProjectDocument) -> PortResult<()> {
        let bytes = serde_json::to_vec_pretty(document).map_err(|e| e.to_string())?;
        if bytes.len() > 64 * 1024 * 1024 {
            return Err("Project is larger than 64 MiB".into());
        }
        atomic_write(&self.path, &bytes)
    }
    fn location(&self) -> &Path {
        &self.path
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> PortResult<()> {
    let name = path
        .file_name()
        .ok_or("Expected a file path")?
        .to_string_lossy();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let temp = path.with_file_name(format!(".{name}.{}.{nonce}.tmp", std::process::id()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result.map_err(|e: std::io::Error| e.to_string())
}

pub fn write_ppm(path: &Path, frame: &CapturedFrame) -> PortResult<()> {
    let mut bytes =
        format!("P6\n{} {}\n255\n", frame.extent.width, frame.extent.height).into_bytes();
    bytes.reserve(frame.rgba.len() / 4 * 3);
    for pixel in frame.rgba.as_chunks::<4>().0 {
        bytes.extend_from_slice(&pixel[..3]);
    }
    atomic_write(path, &bytes)
}
