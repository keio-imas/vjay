pub mod composition;
use crate::node::{AssetId, NodeFactory, NodeTypeId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
mod connections;
mod ordering;
mod ports;
mod settings;
pub use ports::{InputPortRef, NodePorts, OutputPort, OutputPortRef};
use thiserror::Error;

pub const MAX_NODES: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub u64);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeRecord {
    pub id: NodeId,
    #[serde(rename = "kind", alias = "type_id")]
    pub type_id: NodeTypeId,
    pub parameters: Vec<f32>,
    #[serde(default)]
    pub asset: Option<AssetId>,
    #[serde(default)]
    pub settings: crate::settings::NodeSettings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Connection {
    #[serde(default)]
    pub output: usize,
    pub from: NodeId,
    pub to: NodeId,
    pub input: usize,
}

#[derive(Debug, Error, Clone, PartialEq)]
pub enum GraphError {
    #[error("Unknown node: {0:?}")]
    UnknownNode(NodeId),
    #[error("Duplicate node ID or node limit ({MAX_NODES}) exceeded")]
    InvalidId,
    #[error(transparent)]
    Definition(#[from] crate::registry::NodeError),
    #[error("Invalid input port")]
    InvalidPort,
    #[error("Incompatible port types")]
    PortType,
    #[error("Output nodes cannot be used as a source")]
    TerminalSource,
    #[error("This connection would create a cycle")]
    Cycle,
    #[error("Invalid parameter value or index")]
    InvalidParameter,
}

/// Dense node storage for traversal, with constant-time ID lookup.
#[derive(Clone, Default)]
pub struct Graph {
    nodes: Vec<NodeRecord>,
    indices: HashMap<NodeId, usize>,
    ports: Vec<NodePorts>,
    connection_count: usize,
    next_id: u64,
}

impl Graph {
    pub fn reserve_ids_until(&mut self, next: u64) {
        self.next_id = self.next_id.max(next);
    }
    pub fn clear_asset(&mut self, id: NodeId) -> Result<(), GraphError> {
        let index = self.index(id)?;
        self.nodes[index].asset = None;
        Ok(())
    }
    pub fn nodes(&self) -> &[NodeRecord] {
        &self.nodes
    }
    pub fn index(&self, id: NodeId) -> Result<usize, GraphError> {
        self.indices
            .get(&id)
            .copied()
            .ok_or(GraphError::UnknownNode(id))
    }
    pub fn node(&self, id: NodeId) -> Result<&NodeRecord, GraphError> {
        Ok(&self.nodes[self.index(id)?])
    }
    pub fn add(
        &mut self,
        type_id: impl Into<NodeTypeId>,
        factory: &dyn NodeFactory,
    ) -> Result<NodeId, GraphError> {
        let type_id = type_id.into();
        let node = factory.get(&type_id)?;
        let id = NodeId(self.next_id);
        let mut settings = node.default_settings();
        settings.port_version = node.port_version();
        let parameters = node
            .schema(&settings)
            .parameters
            .iter()
            .map(|p| p.default)
            .collect();
        self.insert(
            NodeRecord {
                id,
                type_id,
                parameters,
                asset: None,
                settings,
            },
            factory,
        )?;
        Ok(id)
    }
    pub fn insert_copy(
        &mut self,
        mut node: NodeRecord,
        factory: &dyn NodeFactory,
    ) -> Result<NodeId, GraphError> {
        node.id = NodeId(self.next_id);
        let id = node.id;
        self.insert(node, factory)?;
        Ok(id)
    }
    pub fn insert(
        &mut self,
        mut record: NodeRecord,
        factory: &dyn NodeFactory,
    ) -> Result<(), GraphError> {
        if self.nodes.len() >= MAX_NODES
            || self.indices.contains_key(&record.id)
            || record.id.0 == u64::MAX
        {
            return Err(GraphError::InvalidId);
        }
        let definition = factory.get(&record.type_id)?;
        if record.settings.port_version > definition.port_version() {
            return Err(GraphError::InvalidParameter);
        }
        record.settings.port_version = definition.port_version();
        definition.migrate_parameters_with_settings(&mut record.parameters, &record.settings);
        let schema = definition.schema(&record.settings);
        for (parameter, value) in schema.parameters.iter().zip(&mut record.parameters) {
            *value = parameter
                .normalize(*value)
                .ok_or(GraphError::InvalidParameter)?;
        }
        if !record.settings.is_valid()
            || !definition.validate_settings(&record.settings)
            || (record.asset.is_some()
                && definition.descriptor().capabilities.media.is_none()
                && !definition.descriptor().capabilities.still_image)
            || !definition.validate_parameters(&record.parameters, &record.settings)
        {
            return Err(GraphError::InvalidParameter);
        }
        self.next_id = self.next_id.max(record.id.0 + 1);
        self.indices.insert(record.id, self.nodes.len());
        self.ports.push(NodePorts::new(schema));
        self.nodes.push(record);
        Ok(())
    }
    pub fn remove(&mut self, id: NodeId) -> Result<(), GraphError> {
        let index = self.index(id)?;
        self.nodes.swap_remove(index);
        self.ports.swap_remove(index);
        self.indices.remove(&id);
        if let Some(moved) = self.nodes.get(index) {
            self.indices.insert(moved.id, index);
        }
        self.prune_registrations(|node| node == id);
        Ok(())
    }
    pub fn remove_many(
        &mut self,
        ids: &std::collections::HashSet<NodeId>,
    ) -> Result<(), GraphError> {
        for &id in ids {
            self.index(id)?;
        }
        let mut index = 0;
        self.ports.retain(|_| {
            let keep = !ids.contains(&self.nodes[index].id);
            index += 1;
            keep
        });
        self.nodes.retain(|node| !ids.contains(&node.id));
        self.indices.clear();
        for (index, node) in self.nodes.iter().enumerate() {
            self.indices.insert(node.id, index);
        }
        self.prune_registrations(|node| ids.contains(&node));
        Ok(())
    }
    pub fn bind_asset(
        &mut self,
        id: NodeId,
        asset: AssetId,
        factory: &dyn NodeFactory,
    ) -> Result<(), GraphError> {
        let index = self.index(id)?;
        let capabilities = factory
            .get(&self.nodes[index].type_id)?
            .descriptor()
            .capabilities;
        if capabilities.media.is_none() && !capabilities.still_image {
            return Err(GraphError::InvalidParameter);
        }
        self.nodes[index].asset = Some(asset);
        Ok(())
    }
    pub fn set_parameter(
        &mut self,
        id: NodeId,
        parameter: usize,
        value: f32,
        factory: &dyn NodeFactory,
    ) -> Result<(), GraphError> {
        let index = self.index(id)?;
        let node = &mut self.nodes[index];
        let definition = factory.get(&node.type_id)?;
        let descriptor = &self.ports[index].schema;
        let p = descriptor
            .parameters
            .get(parameter)
            .ok_or(GraphError::InvalidParameter)?;
        let value = p.normalize(value).ok_or(GraphError::InvalidParameter)?;
        let previous = std::mem::replace(&mut node.parameters[parameter], value);
        if !definition.validate_parameters(&node.parameters, &node.settings) {
            node.parameters[parameter] = previous;
            return Err(GraphError::InvalidParameter);
        }
        Ok(())
    }
}

#[cfg(test)]
mod registration_tests;
#[cfg(test)]
mod tests;
