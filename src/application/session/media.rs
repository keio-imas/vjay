use super::Session;
use crate::{media::MediaAsset, ports::PortResult, project::Point};
use std::path::PathBuf;
use vjay_domain::{
    graph::{MAX_NODES, NodeId},
    node::AssetId,
};

impl Session {
    /// Import a disconnected clip. Users own every connection in the patch.
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
            return Err("Choose a video file".into());
        }
        if let Some(target) = target
            && let Ok(n) = self.project.graph.node(target)
            && self
                .project
                .assets
                .iter()
                .any(|a| Some(a.id) == n.asset && a.path == path)
        {
            return Ok(target);
        }
        if self.project.assets.len() >= MAX_NODES {
            return Err("Media asset limit exceeded".into());
        }
        if let Some(id) = target {
            let caps = self
                .project
                .graph
                .schema(id)
                .map_err(|e| e.to_string())?
                .capabilities;
            if if still {
                !caps.still_image
            } else {
                caps.media.is_none()
            } {
                return Err("Choose a compatible media node".into());
            }
        }
        let asset = AssetId(match self.project.assets.iter().map(|a| a.id.0).max() {
            Some(id) => id
                .checked_add(1)
                .filter(|id| *id != u64::MAX)
                .ok_or("Media IDs exhausted")?,
            None => 0,
        });
        let id = match target {
            Some(id) => id,
            None => {
                let type_id = self
                    .catalog()
                    .iter()
                    .find(|d| {
                        if still {
                            d.capabilities.still_image
                        } else {
                            d.capabilities.media.is_some()
                        }
                    })
                    .map(|d| d.type_id)
                    .ok_or("No media node is registered")?;
                self.add_node(type_id, Point { x: 30.0, y: 80.0 })?
            }
        };
        self.project
            .graph
            .bind_asset(id, asset, self.nodes.as_ref())
            .map_err(|e| e.to_string())?;
        self.project.assets.push(MediaAsset { id: asset, path });
        self.project.assets.retain(|asset| {
            self.project
                .graph
                .nodes()
                .iter()
                .any(|node| node.asset == Some(asset.id))
        });
        self.invalidate(true);
        Ok(id)
    }
}
