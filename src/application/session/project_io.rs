use super::*;
use std::{path::PathBuf, sync::Arc};

pub struct SaveRequest {
    io: Arc<dyn ProjectFiles>,
    document: ProjectDocument,
    saved: SavedProject,
}
pub struct SavedProject {
    epoch: u64,
    revision: u64,
    path: PathBuf,
    save_as: bool,
}
impl SaveRequest {
    /// Execute on an injected executor, outside the UI/render thread.
    pub fn execute(self) -> PortResult<SavedProject> {
        self.io.write(&self.saved.path, &self.document)?;
        Ok(self.saved)
    }
}
pub struct OpenRequest {
    io: Arc<dyn ProjectFiles>,
    epoch: u64,
    revision: u64,
    path: PathBuf,
}
pub struct LoadedProject {
    epoch: u64,
    revision: u64,
    path: PathBuf,
    document: ProjectDocument,
}
impl OpenRequest {
    pub fn execute(self) -> PortResult<LoadedProject> {
        let document = self.io.read(&self.path)?;
        Ok(LoadedProject {
            epoch: self.epoch,
            revision: self.revision,
            path: self.path,
            document,
        })
    }
}
impl Session {
    pub(super) fn mark_modified(&self) {
        self.modified.set(true);
        self.revision.set(self.revision.get().wrapping_add(1));
    }
    pub fn prepare_save(&self, path: Option<PathBuf>) -> PortResult<SaveRequest> {
        let io = self
            .store
            .background_io()
            .ok_or("Project adapter has no background file interface")?;
        let save_as = path.is_some();
        let path = path.unwrap_or_else(|| self.store.location().to_owned());
        Ok(SaveRequest {
            io,
            document: self.root_snapshot()?.document(),
            saved: SavedProject {
                epoch: self.project_epoch,
                revision: self.revision.get(),
                path,
                save_as,
            },
        })
    }
    /// Returns whether this write includes all current edits. Edits made during
    /// the write stay dirty; a late result never adopts another project's path.
    pub fn finish_save(&mut self, saved: SavedProject) -> PortResult<bool> {
        if self.project_epoch != saved.epoch {
            return Ok(false);
        }
        if saved.save_as {
            self.store.set_location(saved.path)?;
            self.untitled = false;
        }
        let current = self.revision.get() == saved.revision;
        if current {
            self.modified.set(false);
        }
        Ok(current)
    }
    pub fn prepare_open(&self, path: PathBuf) -> PortResult<OpenRequest> {
        Ok(OpenRequest {
            io: self
                .store
                .background_io()
                .ok_or("Project adapter has no background file interface")?,
            epoch: self.project_epoch,
            revision: self.revision.get(),
            path,
        })
    }
    pub fn finish_open(&mut self, loaded: LoadedProject) -> PortResult<()> {
        if self.project_epoch != loaded.epoch || self.revision.get() != loaded.revision {
            return Err("Project changed while opening; current edits were preserved".into());
        }
        let candidate = Project::restore(loaded.document, self.nodes.as_ref())?;
        self.store.set_location(loaded.path)?;
        self.install_project(candidate);
        self.untitled = false;
        Ok(())
    }
}
