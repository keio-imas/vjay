use super::*;
pub(super) struct Ancestor {
    pub node: NodeId,
    pub parent: Project,
}
impl Session {
    pub(super) fn root_snapshot(&self) -> PortResult<Project> {
        let mut p = self.project.clone();
        for a in self.ancestors.iter().rev() {
            let mut parent = a.parent.clone();
            let child = parent
                .subpatches
                .iter_mut()
                .find(|c| c.node == a.node)
                .ok_or("Missing parent subpatch")?;
            *child.project = p;
            p = parent;
        }
        self.snapshot_bookmarks(&mut p)?;
        p.refresh_interfaces(self.nodes.as_ref())?;
        Ok(p)
    }
    pub(super) fn snapshot_bookmarks(&self, p: &mut Project) -> PortResult<()> {
        for (id, bookmark) in self.timeline.bookmarks() {
            if let Some(owner) = p.owner_mut(id) {
                let mut settings = owner
                    .graph
                    .node(id)
                    .map_err(|e| e.to_string())?
                    .settings
                    .clone();
                settings.transport = Some(bookmark);
                owner
                    .graph
                    .set_settings(id, settings, self.nodes.as_ref())
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
    pub(super) fn owner(&self, id: NodeId) -> Option<&Project> {
        self.project
            .owner(id)
            .or_else(|| self.ancestors.iter().rev().find_map(|a| a.parent.owner(id)))
    }
    pub(super) fn owner_mut(&mut self, id: NodeId) -> Option<&mut Project> {
        if self.project.owner(id).is_some() {
            self.project.owner_mut(id)
        } else {
            self.ancestors
                .iter_mut()
                .rev()
                .find_map(|a| a.parent.owner_mut(id))
        }
    }
    pub(super) fn reserve_identities(&mut self) {
        let (n, a) = self.project.all_ids();
        self.next_node_id = n.iter().map(|n| n.0).max().unwrap_or(0).saturating_add(1);
        self.next_asset_id = a
            .iter()
            .map(|a| a.0)
            .max()
            .map_or(0, |a| a.saturating_add(1));
    }
    pub(super) fn used_identities(
        &self,
    ) -> (
        std::collections::HashSet<NodeId>,
        std::collections::HashSet<vjay_domain::node::AssetId>,
    ) {
        let (mut n, mut a) = self.project.all_ids();
        for p in &self.ancestors {
            let (pn, pa) = p.parent.all_ids();
            n.extend(pn);
            a.extend(pa);
        }
        (n, a)
    }
    pub fn is_subpatch(&self, id: NodeId) -> bool {
        self.project.subpatches.iter().any(|p| p.node == id)
    }
    pub fn patch_depth(&self) -> usize {
        self.ancestors.len()
    }
    pub fn root_output(&self) -> Option<NodeId> {
        self.ancestors
            .first()
            .map_or(self.project.output, |a| a.parent.output)
    }
    pub fn patch_breadcrumbs(&self) -> Vec<String> {
        let mut result = vec!["Project".into()];
        for a in &self.ancestors {
            let node = a.parent.graph.node(a.node).expect("parent instance");
            result.push(if node.settings.label.is_empty() {
                format!("Subpatch #{}", a.node.0)
            } else {
                node.settings.label.clone()
            });
        }
        result
    }
    pub fn enter_subpatch(&mut self, node: NodeId) -> PortResult<()> {
        if self.ancestors.len() >= 8 {
            return Err("Subpatch nesting exceeds eight levels".into());
        }
        let child = self
            .project
            .subpatches
            .iter_mut()
            .find(|p| p.node == node)
            .ok_or("Choose a subpatch node")?;
        let project = std::mem::replace(&mut child.project, Box::new(Project::blank()));
        let parent = std::mem::replace(&mut self.project, *project);
        self.ancestors.push(Ancestor { node, parent });
        self.midi.learning = None;
        Ok(())
    }
    pub fn leave_subpatch(&mut self) -> PortResult<()> {
        let Some(ancestor) = self.ancestors.last() else {
            return Ok(());
        };
        let mut parent = ancestor.parent.clone();
        let child = parent
            .subpatches
            .iter_mut()
            .find(|p| p.node == ancestor.node)
            .ok_or("Missing parent subpatch")?;
        *child.project = self.project.clone();
        parent.refresh_interfaces(self.nodes.as_ref())?;
        self.ancestors.pop();
        self.project = parent;
        self.invalidate(false);
        Ok(())
    }
    pub fn leave_to(&mut self, depth: usize) -> PortResult<()> {
        while self.ancestors.len() > depth {
            self.leave_subpatch()?;
        }
        Ok(())
    }
}
