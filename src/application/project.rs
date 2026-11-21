pub mod clipboard;
#[cfg(test)]
mod tests;
pub mod value_display;
pub mod workspace;
use crate::ports::PortResult;
use serde::{Deserialize, Serialize};
use vjay_domain::{
    graph::{Connection, Graph, MAX_NODES, NodeId, NodeRecord},
    node::NodeFactory,
};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodePosition {
    pub node: NodeId,
    pub position: Point,
}

mod controls;
#[cfg(test)]
mod demo;
pub use controls::{ControlBinding, ControlTarget, WidgetKind};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectDocument {
    pub version: u32,
    pub nodes: Vec<NodeRecord>,
    pub connections: Vec<Connection>,
    pub output: Option<NodeId>,
    pub positions: Vec<NodePosition>,
    pub controls: Vec<ControlBinding>,
    #[serde(default)]
    pub assets: Vec<crate::media::MediaAsset>,
    #[serde(default)]
    pub workspace: crate::workspace::WorkspaceLayout,
    #[serde(default)]
    pub subpatches: Vec<SubpatchDocument>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubpatchDocument {
    pub node: NodeId,
    #[serde(default)]
    pub source: Option<std::path::PathBuf>,
    pub document: Box<ProjectDocument>,
}
#[derive(Clone)]
pub struct Subpatch {
    pub node: NodeId,
    pub source: Option<std::path::PathBuf>,
    pub project: Box<Project>,
}
#[derive(Clone)]
pub struct Project {
    pub graph: Graph,
    pub output: Option<NodeId>,
    pub positions: Vec<NodePosition>,
    pub controls: Vec<ControlBinding>,
    pub assets: Vec<crate::media::MediaAsset>,
    pub workspace: crate::workspace::WorkspaceLayout,
    pub subpatches: Vec<Subpatch>,
}

impl Project {
    pub fn empty(factory: &dyn NodeFactory) -> PortResult<Self> {
        let mut graph = Graph::default();
        let output = factory
            .catalog()
            .iter()
            .find(|d| d.capabilities.program_output.is_some())
            .map(|d| graph.add(d.type_id, factory))
            .transpose()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            graph,
            output,
            positions: output
                .map(|node| NodePosition {
                    node,
                    position: Point { x: 280.0, y: 80.0 },
                })
                .into_iter()
                .collect(),
            controls: Vec::new(),
            assets: Vec::new(),
            workspace: Default::default(),
            subpatches: Vec::new(),
        })
    }
    pub fn document(&self) -> ProjectDocument {
        ProjectDocument {
            version: 1,
            nodes: self.graph.nodes().to_vec(),
            connections: self.graph.connections().collect::<Vec<_>>(),
            output: self.output,
            positions: self.positions.clone(),
            controls: self.controls.clone(),
            assets: self.assets.clone(),
            workspace: self.workspace,
            subpatches: self
                .subpatches
                .iter()
                .map(|p| SubpatchDocument {
                    node: p.node,
                    source: p.source.clone(),
                    document: Box::new(p.project.document()),
                })
                .collect(),
        }
    }

    pub fn restore(mut document: ProjectDocument, factory: &dyn NodeFactory) -> PortResult<Self> {
        crate::composition::identities::normalize(
            &mut document,
            &Default::default(),
            &Default::default(),
        )?;
        Self::restore_scoped(document, factory, 0)
    }
    pub(crate) fn restore_scoped(
        mut document: ProjectDocument,
        factory: &dyn NodeFactory,
        depth: usize,
    ) -> PortResult<Self> {
        if depth > 8 {
            return Err("Subpatch nesting exceeds eight levels".into());
        }
        let mut subpatches = Vec::new();
        let mut children = std::collections::HashSet::new();
        for child in std::mem::take(&mut document.subpatches) {
            if !children.insert(child.node) {
                return Err("Duplicate subpatch instance".into());
            }
            let project = Self::restore_scoped(*child.document, factory, depth + 1)?;
            let record = document
                .nodes
                .iter_mut()
                .find(|n| n.id == child.node)
                .ok_or("Subpatch has no container node")?;
            if !factory.get(&record.type_id)?.composite() {
                return Err("Node does not accept a subpatch".into());
            }
            record.settings.interface = Some(crate::composition::interface::of(&project, factory)?);
            subpatches.push(Subpatch {
                node: child.node,
                source: child.source,
                project: Box::new(project),
            });
        }
        if document.nodes.iter().any(|n| {
            factory.get(&n.type_id).is_ok_and(|d| d.composite()) && !children.contains(&n.id)
        }) {
            return Err("Subpatch definition is missing".into());
        }

        if document.version != 1 {
            return Err("Unsupported project version".into());
        }
        if !document.workspace.is_valid() {
            return Err("Invalid workspace layout".into());
        }
        if document.nodes.len() > MAX_NODES
            || document.connections.len() > MAX_NODES * 80
            || document.positions.len() > MAX_NODES
            || document.controls.len() > MAX_NODES * 3
        {
            return Err("Project exceeds limits".into());
        }
        if document.assets.len() > MAX_NODES {
            return Err("Too many media assets".into());
        }
        let mut asset_ids = std::collections::HashSet::new();
        for asset in &document.assets {
            if asset.path.as_os_str().is_empty()
                || asset.id.0 == u64::MAX
                || !asset_ids.insert(asset.id)
            {
                return Err("Invalid media asset".into());
            }
        }
        let mut graph = Graph::default();
        let migrations = crate::port_migrations::PortMigrations::new(&document.nodes, factory)?;
        for node in document.nodes {
            if let Some(asset) = node.asset
                && !asset_ids.contains(&asset)
            {
                return Err("Invalid media reference".into());
            }
            graph.insert(node, factory).map_err(|e| e.to_string())?;
        }
        let mut occupied = std::collections::HashSet::new();
        for edge in document.connections {
            let edge = migrations.connection(edge)?;
            if !occupied.insert((edge.to, edge.input)) {
                return Err("Duplicate input connection".into());
            }
            graph.connect(edge).map_err(|e| e.to_string())?;
        }
        crate::transport_migrations::cues(&mut graph, factory)?;
        if let Some(output) = document.output
            && factory
                .get(&graph.node(output).map_err(|e| e.to_string())?.type_id)?
                .descriptor()
                .capabilities
                .program_output
                .is_none()
        {
            return Err("Invalid Program output".into());
        }
        let finite = |p: Point| {
            p.x.is_finite()
                && p.y.is_finite()
                && p.x.abs() <= 1_000_000.0
                && p.y.abs() <= 1_000_000.0
        };
        let mut positioned = std::collections::HashSet::new();
        for p in &document.positions {
            if graph.node(p.node).is_err() || !finite(p.position) || !positioned.insert(p.node) {
                return Err("Invalid node layout".into());
            }
        }
        if positioned.len() != graph.nodes().len() {
            return Err("Every node must have a layout position".into());
        }
        let mut ids = std::collections::HashSet::new();
        for control in &document.controls {
            let node = graph.node(control.node).map_err(|e| e.to_string())?;
            if !valid_control(control, node, factory)
                || !finite(control.position)
                || !finite(control.size)
                || control.size.x < 80.0
                || control.size.y < 30.0
                || control.id == u64::MAX
                || !ids.insert(control.id)
            {
                return Err("Invalid control binding or layout".into());
            }
        }
        Ok(Self {
            graph,
            output: document.output,
            positions: document.positions,
            controls: document.controls,
            assets: document.assets,
            workspace: document.workspace,
            subpatches,
        })
    }
}

pub use controls::valid_control;
