use super::*;
use std::path::{Path, PathBuf};
impl Session {
    pub fn project_modified(&self) -> bool {
        self.modified.get()
    }
    pub fn is_untitled(&self) -> bool {
        self.untitled
    }
    pub fn new_project(&mut self) -> PortResult<()> {
        let document = Project::empty(self.nodes.as_ref())?.document();
        self.restore(document)?;
        self.untitled = true;
        Ok(())
    }
    pub fn open_project(&mut self, path: PathBuf) -> PortResult<()> {
        let doc = self.store.load_from(&path)?;
        let _ = Project::restore(doc.clone(), self.nodes.as_ref())?;
        self.store.set_location(path)?;
        self.restore(doc)?;
        self.untitled = false;
        self.modified.set(false);
        Ok(())
    }
    pub fn save_project_as(&mut self, path: PathBuf) -> PortResult<()> {
        self.store.save_to(&path, &self.project.document())?;
        self.store.set_location(path)?;
        self.untitled = false;
        self.modified.set(false);
        Ok(())
    }
    pub fn project_name(&self) -> String {
        if self.untitled {
            self.preferences.default_project_name.clone()
        } else {
            self.store
                .location()
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        }
    }
    pub fn project_directory(&self) -> &Path {
        self.store
            .location()
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
    }
}
