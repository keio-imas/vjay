use crate::{media::MediaAsset, project::NodePosition};
use serde::{Deserialize, Serialize};
use vjay_domain::graph::{Connection, NodeRecord};
#[derive(Clone, Serialize, Deserialize)]
pub struct PatchFragment {
    pub nodes: Vec<NodeRecord>,
    pub connections: Vec<Connection>,
    pub positions: Vec<NodePosition>,
    pub assets: Vec<MediaAsset>,
}
