use super::{NodeService, ServiceFactory};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use vjay_application::{control_io::ControlEvent, ports::PortResult, services::ServiceRequest};
use vjay_domain::graph::NodeId;
mod config;
mod query;
mod snapshot;
pub use config::RekordboxPaths;
pub struct RekordboxFactory {
    pub paths: RekordboxPaths,
}
impl ServiceFactory for RekordboxFactory {
    fn id(&self) -> &'static str {
        "rekordbox-history"
    }
    fn create(&self, request: &ServiceRequest) -> PortResult<Box<dyn NodeService>> {
        let path = |name: &str, fallback: &PathBuf| {
            request
                .properties
                .get(name)
                .filter(|v| !v.is_empty() && v.as_str() != "auto")
                .map_or_else(|| fallback.clone(), PathBuf::from)
        };
        let database = path("database", &self.paths.database);
        let options = path("options_file", &self.paths.options);
        let explicit = request
            .properties
            .get("database")
            .is_some_and(|v| !v.is_empty() && v != "auto");
        let interval = request
            .properties
            .get("poll_seconds")
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(2.0)
            .clamp(0.25, 60.0);
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let mailbox = Arc::new(Mutex::new(None));
        let pending = mailbox.clone();
        let worker = thread::Builder::new()
            .name("vjay-rekordbox-read".into())
            .spawn(move || {
                let mut previous = None;
                let mut cursor = None;
                while !stopping.load(Ordering::Acquire) {
                    let mut read = || -> PortResult<_> {
                        let configured = config::credentials(&options);
                        let database = if explicit {
                            database.clone()
                        } else {
                            configured
                                .as_ref()
                                .map_or_else(|_| database.clone(), |(path, _)| path.clone())
                        };
                        let encrypted = {
                            use std::io::Read;
                            let mut bytes = [0; 16];
                            std::fs::File::open(&database)
                                .map_err(|e| e.to_string())?
                                .read_exact(&mut bytes)
                                .map_err(|e| e.to_string())?;
                            &bytes != b"SQLite format 3\0"
                        };
                        let key = if encrypted { Some(configured?.1) } else { None };
                        let signature = snapshot::fingerprint(&database)?;
                        if previous.as_ref() == Some(&signature) {
                            return Ok(None);
                        }
                        let snapshot = snapshot::Snapshot::open(&database, key.as_deref())?;
                        let mut track = query::latest(&snapshot.connection)?;
                        if let Some(track) = &mut track {
                            track.cover = track.cover.take().and_then(|p| {
                                let root = database.parent()?;
                                [
                                    if p.is_absolute() {
                                        p.clone()
                                    } else {
                                        root.join(&p)
                                    },
                                    root.join(p.strip_prefix("/").unwrap_or(&p)),
                                    root.join("share").join(&p),
                                ]
                                .into_iter()
                                .find(|p| p.is_file())
                            });
                        }
                        previous = Some(signature);
                        Ok(Some(track))
                    };
                    let update = match read() {
                        Ok(Some(track)) => {
                            let changed = track
                                .as_ref()
                                .is_some_and(|t| cursor.is_some_and(|c| t.row > c));
                            cursor = track.as_ref().map(|t| t.row);
                            Some(Update {
                                track,
                                changed,
                                error: None,
                            })
                        }
                        Ok(None) => None,
                        Err(error) => Some(Update {
                            track: None,
                            changed: false,
                            error: Some(error),
                        }),
                    };
                    if let Some(mut update) = update
                        && let Ok(mut latest) = pending.lock()
                    {
                        update.changed |= latest.as_ref().is_some_and(|u: &Update| u.changed);
                        *latest = Some(update);
                    }
                    let mut elapsed = 0.0;
                    while elapsed < interval && !stopping.load(Ordering::Acquire) {
                        thread::sleep(Duration::from_millis(25));
                        elapsed += 0.025;
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Box::new(HistoryService {
            stop,
            worker: Some(worker),
            mailbox,
            values: [0.0; 16],
            text: vec![Arc::from(""); 10],
            cover: None,
        }))
    }
}
struct Update {
    track: Option<query::Track>,
    changed: bool,
    error: Option<String>,
}
struct HistoryService {
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    mailbox: Arc<Mutex<Option<Update>>>,
    values: [f32; 16],
    text: Vec<Arc<str>>,
    cover: Option<vjay_domain::execution::FileValue>,
}
impl NodeService for HistoryService {
    fn update(&mut self, _: &[f32]) {}
    fn poll(&mut self, node: NodeId, events: &mut Vec<ControlEvent>) {
        self.values[2] = 0.0;
        let mut text_changed = false;
        let update = self
            .mailbox
            .try_lock()
            .ok()
            .and_then(|mut pending| pending.take());
        if let Some(update) = update {
            if let Some(error) = update.error {
                self.values[3] = 0.0;
                self.text[9] = Arc::from(error);
                self.cover = None;
            } else if let Some(track) = update.track {
                self.values[0] = track.bpm;
                self.values[1] = track.duration;
                self.values[2] = f32::from(update.changed);
                self.values[3] = 1.0;
                self.text = track.text;
                self.cover = track.cover.map(|p| Arc::from(p.as_path()));
            } else {
                self.values[3] = 1.0;
                self.text[9] = Arc::from("No play history entries");
                self.cover = None;
            }
            text_changed = true;
        }
        self.values[4] = f32::from(self.cover.is_some());
        events.push(ControlEvent::NodeValues {
            node,
            values: self.values,
        });
        if text_changed {
            events.push(ControlEvent::NodeFile {
                node,
                values: vec![self.cover.clone()],
            });
            events.push(ControlEvent::NodeText {
                node,
                values: self.text.clone(),
            });
        }
    }
}
impl Drop for HistoryService {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
#[cfg(test)]
mod tests;
