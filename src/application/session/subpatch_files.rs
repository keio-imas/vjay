use super::*;
use std::path::PathBuf;
impl Session {
    pub fn load_subpatch_file(&mut self, path: PathBuf, position: Point) -> PortResult<NodeId> {
        let mut document = self.store.load_from(&path)?;
        let (nodes, assets) = self.used_identities();
        crate::composition::identities::normalize(&mut document, &nodes, &assets)?;
        let child = Project::restore(document, self.nodes.as_ref())?;
        let interface = crate::composition::interface::of(&child, self.nodes.as_ref())?;
        let ty = self
            .catalog()
            .iter()
            .find(|d| self.nodes.get(d.type_id).is_ok_and(|n| n.composite()))
            .ok_or("No subpatch node registered")?
            .type_id;
        // The imported hierarchy reserves its IDs before creating its container.
        let (n, a) = child.all_ids();
        self.next_node_id = self
            .next_node_id
            .max(n.iter().map(|n| n.0).max().unwrap_or(0).saturating_add(1));
        self.next_asset_id = self.next_asset_id.max(
            a.iter()
                .map(|a| a.0)
                .max()
                .map_or(0, |a| a.saturating_add(1)),
        );
        let id = self.add_node(ty, position)?;
        let entry = self
            .project
            .subpatches
            .iter_mut()
            .find(|p| p.node == id)
            .expect("new instance");
        entry.source = Some(path.clone());
        *entry.project = child;
        let mut settings = self
            .project
            .graph
            .node(id)
            .map_err(|e| e.to_string())?
            .settings
            .clone();
        settings.interface = Some(interface);
        settings.label = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into();
        self.project
            .graph
            .set_settings(id, settings, self.nodes.as_ref())
            .map_err(|e| e.to_string())?;
        if self.project.any_node(&|n| {
            self.descriptor(&n.type_id)
                .capabilities
                .settings
                .contains(&vjay_domain::capabilities::SettingEditor::Midi)
        }) {
            self.ensure_midi();
        }
        self.invalidate(true);
        Ok(id)
    }
    pub fn save_subpatch_as(&mut self, node: Option<NodeId>, path: PathBuf) -> PortResult<()> {
        let document = if let Some(id) = node {
            self.project
                .subpatches
                .iter()
                .find(|p| p.node == id)
                .ok_or("Choose a subpatch")?
                .project
                .document()
        } else {
            if self.ancestors.is_empty() {
                return Err("Open a subpatch first".into());
            }
            let mut p = self.project.clone();
            p.refresh_interfaces(self.nodes.as_ref())?;
            p.document()
        };
        let mut snapshot = Project::restore(document, self.nodes.as_ref())?;
        self.snapshot_bookmarks(&mut snapshot)?;
        self.store.save_to(&path, &snapshot.document())?;
        if let Some(id) = node {
            self.project
                .subpatches
                .iter_mut()
                .find(|p| p.node == id)
                .expect("validated subpatch")
                .source = Some(path);
        } else if let Some(parent) = self.ancestors.last_mut() {
            parent
                .parent
                .subpatches
                .iter_mut()
                .find(|p| p.node == parent.node)
                .expect("parent instance")
                .source = Some(path);
        }
        self.mark_modified();
        Ok(())
    }
    pub fn subpatch_source(&self, node: NodeId) -> Option<&std::path::Path> {
        self.project
            .subpatches
            .iter()
            .find(|p| p.node == node)?
            .source
            .as_deref()
    }
    pub fn reload_subpatch(&mut self, node: NodeId) -> PortResult<()> {
        let path = self
            .subpatch_source(node)
            .ok_or("This subpatch has no saved file")?
            .to_owned();
        let mut document = self.store.load_from(&path)?;
        let (nodes, assets) = self.used_identities();
        crate::composition::identities::normalize(&mut document, &nodes, &assets)?;
        let project = Project::restore(document, self.nodes.as_ref())?;
        let mut candidate = self.project.clone();
        *candidate
            .subpatches
            .iter_mut()
            .find(|p| p.node == node)
            .ok_or("Missing subpatch")?
            .project = project;
        candidate.refresh_interfaces(self.nodes.as_ref())?;
        self.project = candidate;
        let (nodes, assets) = self.used_identities();
        self.next_node_id = self.next_node_id.max(
            nodes
                .iter()
                .map(|n| n.0)
                .max()
                .unwrap_or(0)
                .saturating_add(1),
        );
        self.next_asset_id = self.next_asset_id.max(
            assets
                .iter()
                .map(|a| a.0)
                .max()
                .map_or(0, |a| a.saturating_add(1)),
        );
        self.invalidate(true);
        Ok(())
    }
}
