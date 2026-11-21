use crate::{ports::PortResult, project::*};
use vjay_domain::{
    graph::{Graph, NodeId},
    node::NodeFactory,
};
mod expand;
pub mod identities;
pub mod interface;
pub use expand::{Expanded, expand};
impl Project {
    pub fn any_node(&self, predicate: &impl Fn(&vjay_domain::graph::NodeRecord) -> bool) -> bool {
        self.graph.nodes().iter().any(predicate)
            || self
                .subpatches
                .iter()
                .any(|s| s.project.any_node(predicate))
    }
    pub fn at(&self, path: &[NodeId]) -> Option<&Self> {
        if let Some((first, rest)) = path.split_first() {
            self.subpatches
                .iter()
                .find(|s| s.node == *first)?
                .project
                .at(rest)
        } else {
            Some(self)
        }
    }
    pub fn at_mut(&mut self, path: &[NodeId]) -> Option<&mut Self> {
        if let Some((first, rest)) = path.split_first() {
            self.subpatches
                .iter_mut()
                .find(|s| s.node == *first)?
                .project
                .at_mut(rest)
        } else {
            Some(self)
        }
    }
    pub fn owner(&self, node: NodeId) -> Option<&Self> {
        if self.graph.node(node).is_ok() {
            Some(self)
        } else {
            self.subpatches.iter().find_map(|s| s.project.owner(node))
        }
    }
    pub fn owner_mut(&mut self, node: NodeId) -> Option<&mut Self> {
        if self.graph.node(node).is_ok() {
            Some(self)
        } else {
            self.subpatches
                .iter_mut()
                .find_map(|s| s.project.owner_mut(node))
        }
    }
    pub fn all_ids(
        &self,
    ) -> (
        std::collections::HashSet<NodeId>,
        std::collections::HashSet<vjay_domain::node::AssetId>,
    ) {
        let mut nodes = self
            .graph
            .nodes()
            .iter()
            .map(|n| n.id)
            .collect::<std::collections::HashSet<_>>();
        let mut assets = self
            .assets
            .iter()
            .map(|a| a.id)
            .collect::<std::collections::HashSet<_>>();
        for s in &self.subpatches {
            let (n, a) = s.project.all_ids();
            nodes.extend(n);
            assets.extend(a);
        }
        (nodes, assets)
    }
    pub fn blank() -> Self {
        Self {
            graph: Graph::default(),
            output: None,
            positions: Vec::new(),
            controls: Vec::new(),
            assets: Vec::new(),
            workspace: Default::default(),
            subpatches: Vec::new(),
        }
    }
    pub fn refresh_interfaces(&mut self, factory: &dyn NodeFactory) -> PortResult<()> {
        for index in 0..self.subpatches.len() {
            let child = &mut self.subpatches[index];
            child.project.refresh_interfaces(factory)?;
            let id = child.node;
            let interface = interface::of(&child.project, factory)?;
            let old = self
                .graph
                .node(id)
                .map_err(|e| e.to_string())?
                .settings
                .interface
                .clone()
                .unwrap_or_default();
            if old == interface {
                continue;
            }
            let mut settings = self
                .graph
                .node(id)
                .map_err(|e| e.to_string())?
                .settings
                .clone();
            settings.interface = Some(interface.clone());
            let wires = self
                .graph
                .connections()
                .filter(|e| e.from == id || e.to == id)
                .collect::<Vec<_>>();
            for e in &wires {
                self.graph.disconnect(e.to, e.input);
            }
            self.graph
                .set_settings(id, settings, factory)
                .map_err(|e| e.to_string())?;
            for mut e in wires {
                let mapped = if e.from == id {
                    old.outputs
                        .get(e.output)
                        .and_then(|old| {
                            interface
                                .outputs
                                .iter()
                                .position(|p| p.key == old.key && p.signal == old.signal)
                        })
                        .map(|p| {
                            e.output = p;
                        })
                } else {
                    old.inputs
                        .get(e.input)
                        .and_then(|old| {
                            interface
                                .inputs
                                .iter()
                                .position(|p| p.key == old.key && p.signal == old.signal)
                        })
                        .map(|p| {
                            e.input = p;
                        })
                };
                if mapped.is_some() {
                    self.graph.connect(e).map_err(|e| e.to_string())?;
                }
            }
        }
        Ok(())
    }
}
