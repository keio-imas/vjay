use super::*;
use crate::clipboard::PatchFragment;
use crate::media::MediaAsset;
use std::collections::{HashMap, HashSet};
use vjay_domain::{graph::MAX_NODES, node::AssetId};
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
        if fragment.assets.len() > MAX_NODES
            || fragment.nodes.is_empty()
            || fragment.nodes.len() > MAX_NODES
            || fragment.positions.len() != fragment.nodes.len()
            || fragment.connections.len() > MAX_NODES * 80
            || !anchor.x.is_finite()
            || !anchor.y.is_finite()
        {
            return Err("Invalid clipboard patch".into());
        }
        let ids: HashSet<_> = fragment.nodes.iter().map(|n| n.id).collect();
        let positions: HashMap<_, _> = fragment
            .positions
            .iter()
            .map(|p| (p.node, p.position))
            .collect();
        if ids.len() != fragment.nodes.len()
            || positions.len() != ids.len()
            || !ids.iter().all(|n| positions.contains_key(n))
            || fragment
                .connections
                .iter()
                .any(|e| !ids.contains(&e.from) || !ids.contains(&e.to))
        {
            return Err("Invalid clipboard references".into());
        }
        let left = positions
            .values()
            .map(|p| p.x)
            .fold(f32::INFINITY, f32::min);
        let top = positions
            .values()
            .map(|p| p.y)
            .fold(f32::INFINITY, f32::min);
        let mut graph = self.project.graph.clone();
        let mut assets = self.project.assets.clone();
        let mut asset_map = HashMap::new();
        let needed: HashSet<_> = fragment.nodes.iter().filter_map(|n| n.asset).collect();
        let mut seen = HashSet::new();
        for asset in &fragment.assets {
            if !seen.insert(asset.id) || asset.path.as_os_str().is_empty() {
                return Err("Invalid clipboard assets".into());
            }
            if !needed.contains(&asset.id) {
                continue;
            }
            let id = if let Some(old) = assets.iter().find(|a| a.path == asset.path) {
                old.id
            } else {
                let id = AssetId(
                    assets
                        .iter()
                        .map(|a| a.id.0)
                        .max()
                        .map_or(Some(0), |n| n.checked_add(1))
                        .filter(|n| *n != u64::MAX)
                        .ok_or("Asset IDs exhausted")?,
                );
                assets.push(MediaAsset {
                    id,
                    path: asset.path.clone(),
                });
                id
            };
            asset_map.insert(asset.id, id);
        }
        let mut mapping = HashMap::new();
        let mut new_positions = Vec::new();
        let mut result = Vec::new();
        for record in &fragment.nodes {
            let mut node = record.clone();
            node.asset = node
                .asset
                .map(|a| {
                    asset_map
                        .get(&a)
                        .copied()
                        .ok_or("Clipboard asset is missing")
                })
                .transpose()?;
            let id = graph
                .insert_copy(node, self.nodes.as_ref())
                .map_err(|e| e.to_string())?;
            let p = positions[&record.id];
            let position = Point {
                x: p.x - left + anchor.x,
                y: p.y - top + anchor.y,
            };
            if !position.x.is_finite()
                || !position.y.is_finite()
                || position.x.abs() > 1e6
                || position.y.abs() > 1e6
            {
                return Err("Invalid clipboard layout".into());
            }
            mapping.insert(record.id, id);
            new_positions.push(NodePosition { node: id, position });
            result.push(id);
        }
        let migrations =
            crate::port_migrations::PortMigrations::new(&fragment.nodes, self.nodes.as_ref())?;
        for edge in &fragment.connections {
            let edge = migrations.connection(*edge)?;
            graph
                .connect(Connection {
                    from: mapping[&edge.from],
                    to: mapping[&edge.to],
                    ..edge
                })
                .map_err(|e| e.to_string())?;
        }
        self.project.graph = graph;
        self.project.assets = assets;
        self.project.positions.extend(new_positions);
        self.invalidate(true);
        Ok(result)
    }
}
