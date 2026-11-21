use super::*;
use crate::media::MediaAsset;
use std::path::PathBuf;
use vjay_domain::node::AssetId;
impl Session {
    pub fn import_video(&mut self, path: PathBuf, target: Option<NodeId>) -> PortResult<NodeId> {
        self.import_asset(path, target, false)
    }
    pub fn import_image(&mut self, path: PathBuf, target: Option<NodeId>) -> PortResult<NodeId> {
        self.import_asset(path, target, true)
    }
    fn import_asset(
        &mut self,
        path: PathBuf,
        target: Option<NodeId>,
        still: bool,
    ) -> PortResult<NodeId> {
        if path.as_os_str().is_empty() {
            return Err("Choose a media file".into());
        }
        if let Some(id) = target {
            let p = self.owner(id).ok_or("Unknown media node")?;
            let n = p.graph.node(id).map_err(|e| e.to_string())?;
            if p.assets
                .iter()
                .any(|a| Some(a.id) == n.asset && a.path == path)
            {
                return Ok(id);
            }
            let caps = p.graph.schema(id).map_err(|e| e.to_string())?.capabilities;
            if if still {
                !caps.still_image
            } else {
                caps.media.is_none()
            } {
                return Err("Choose a compatible media node".into());
            }
        }
        let id = match target {
            Some(id) => id,
            None => {
                let ty = self
                    .catalog()
                    .iter()
                    .find(|d| {
                        if still {
                            d.capabilities.still_image
                        } else {
                            d.capabilities.media.is_some()
                        }
                    })
                    .ok_or("No media node registered")?
                    .type_id;
                self.add_node(ty, Point { x: 30.0, y: 80.0 })?
            }
        };
        let asset = AssetId(self.next_asset_id);
        self.next_asset_id = self
            .next_asset_id
            .checked_add(1)
            .ok_or("Asset IDs exhausted")?;
        let Self {
            project,
            ancestors,
            nodes,
            ..
        } = self;
        let owner = if project.owner(id).is_some() {
            project.owner_mut(id)
        } else {
            ancestors
                .iter_mut()
                .rev()
                .find_map(|a| a.parent.owner_mut(id))
        }
        .ok_or("Unknown media owner")?;
        if owner.assets.len() >= vjay_domain::graph::MAX_NODES {
            return Err("Media asset limit exceeded".into());
        }
        owner
            .graph
            .bind_asset(id, asset, nodes.as_ref())
            .map_err(|e| e.to_string())?;
        owner.assets.push(MediaAsset { id: asset, path });
        owner
            .assets
            .retain(|a| owner.graph.nodes().iter().any(|n| n.asset == Some(a.id)));
        self.invalidate(true);
        Ok(id)
    }
}
