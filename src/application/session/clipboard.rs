use super::*;
use crate::clipboard::PatchFragment;
use std::collections::HashSet;
use vjay_domain::node::AssetId;
impl Session {
    pub fn copy_nodes(&self, ids: &[NodeId]) -> PortResult<PatchFragment> {
        let ids: HashSet<_> = ids.iter().copied().collect();
        if ids.is_empty() {
            return Err("Select nodes to copy".into());
        }
        let mut nodes = Vec::new();
        for id in &ids {
            nodes.push(
                self.project
                    .graph
                    .node(*id)
                    .map_err(|e| e.to_string())?
                    .clone(),
            );
        }
        nodes.sort_by_key(|n| n.id.0);
        let assets: HashSet<_> = nodes.iter().filter_map(|n| n.asset).collect();
        Ok(PatchFragment {
            subpatches: self
                .project
                .subpatches
                .iter()
                .filter(|s| ids.contains(&s.node))
                .map(|s| SubpatchDocument {
                    node: s.node,
                    source: s.source.clone(),
                    document: Box::new(s.project.document()),
                })
                .collect(),
            nodes,
            connections: self
                .project
                .graph
                .connections()
                .filter(|c| ids.contains(&c.from) && ids.contains(&c.to))
                .collect(),
            positions: self
                .project
                .positions
                .iter()
                .filter(|p| ids.contains(&p.node))
                .cloned()
                .collect(),
            assets: self
                .project
                .assets
                .iter()
                .filter(|a| assets.contains(&a.id))
                .cloned()
                .collect(),
        })
    }
    pub fn paste_nodes(
        &mut self,
        fragment: &PatchFragment,
        anchor: Point,
    ) -> PortResult<Vec<NodeId>> {
        if fragment.nodes.is_empty() || !anchor.x.is_finite() || !anchor.y.is_finite() {
            return Err("Invalid clipboard patch".into());
        }
        let mut document = ProjectDocument {
            version: 1,
            nodes: fragment.nodes.clone(),
            connections: fragment.connections.clone(),
            positions: fragment.positions.clone(),
            assets: fragment.assets.clone(),
            controls: Vec::new(),
            workspace: Default::default(),
            output: None,
            subpatches: fragment.subpatches.clone(),
        };
        let (mut used, mut assets) = self.used_identities();
        let copied = Project::restore(document.clone(), self.nodes.as_ref())?;
        let (n, a) = copied.all_ids();
        used.extend(n);
        assets.extend(a);
        used.insert(NodeId(self.next_node_id.saturating_sub(1)));
        assets.insert(AssetId(self.next_asset_id.saturating_sub(1)));
        crate::composition::identities::normalize(&mut document, &used, &assets)?;
        let mut pasted = Project::restore(document, self.nodes.as_ref())?;
        for asset in &mut pasted.assets {
            if let Some(existing) = self.project.assets.iter().find(|a| a.path == asset.path) {
                let old = asset.id;
                asset.id = existing.id;
                for record in pasted.graph.nodes().to_vec() {
                    if record.asset == Some(old) {
                        pasted
                            .graph
                            .bind_asset(record.id, existing.id, self.nodes.as_ref())
                            .map_err(|e| e.to_string())?;
                    }
                }
            }
        }
        for record in pasted.graph.nodes().to_vec() {
            if self
                .nodes
                .get(&record.type_id)?
                .boundary(&record.settings)
                .is_some()
            {
                let mut settings = record.settings.clone();
                settings
                    .properties
                    .insert("port.key".into(), format!("port-{}", record.id.0));
                pasted
                    .graph
                    .set_settings(record.id, settings, self.nodes.as_ref())
                    .map_err(|e| e.to_string())?;
            }
        }
        let left = pasted
            .positions
            .iter()
            .map(|p| p.position.x)
            .fold(f32::INFINITY, f32::min);
        let top = pasted
            .positions
            .iter()
            .map(|p| p.position.y)
            .fold(f32::INFINITY, f32::min);
        let mut candidate = self.project.clone();
        let result = pasted
            .graph
            .nodes()
            .iter()
            .map(|n| n.id)
            .collect::<Vec<_>>();
        for n in pasted.graph.nodes() {
            candidate
                .graph
                .insert(n.clone(), self.nodes.as_ref())
                .map_err(|e| e.to_string())?;
        }
        for e in pasted.graph.connections() {
            candidate.graph.connect(e).map_err(|e| e.to_string())?;
        }
        for mut p in pasted.positions {
            p.position = Point {
                x: p.position.x - left + anchor.x,
                y: p.position.y - top + anchor.y,
            };
            if p.position.x.abs() > 1e6 || p.position.y.abs() > 1e6 {
                return Err("Invalid clipboard layout".into());
            }
            candidate.positions.push(p);
        }
        for asset in pasted.assets {
            if !candidate.assets.iter().any(|a| a.id == asset.id) {
                candidate.assets.push(asset);
            }
        }
        candidate.subpatches.extend(pasted.subpatches);
        self.project = candidate;
        let (n, a) = self.used_identities();
        self.next_node_id = self
            .next_node_id
            .max(n.iter().map(|n| n.0).max().unwrap_or(0).saturating_add(1));
        self.next_asset_id = self.next_asset_id.max(
            a.iter()
                .map(|a| a.0)
                .max()
                .map_or(0, |a| a.saturating_add(1)),
        );
        self.invalidate(true);
        Ok(result)
    }
}
