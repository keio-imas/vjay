use super::NodeId;
use crate::{port::SignalType, schema::NodeSchema};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct InputPortRef {
    pub node: NodeId,
    pub port: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OutputPortRef {
    pub node: NodeId,
    pub port: usize,
}

/// An output owns its registrations. Input-side bindings are compiled data.
#[derive(Clone, Debug)]
pub struct OutputPort {
    pub(super) signal: SignalType,
    pub(super) destinations: Vec<InputPortRef>,
}
impl OutputPort {
    pub fn signal(&self) -> SignalType {
        self.signal
    }
    pub fn destinations(&self) -> &[InputPortRef] {
        &self.destinations
    }
}
#[derive(Clone, Debug)]
pub struct NodePorts {
    pub(super) schema: NodeSchema,
    pub(super) inputs: Vec<SignalType>,
    pub(super) outputs: Vec<OutputPort>,
}
impl NodePorts {
    pub(super) fn new(descriptor: NodeSchema) -> Self {
        Self {
            inputs: (0..descriptor.input_count())
                .map(|i| descriptor.input(i).expect("declared input").signal)
                .collect(),
            outputs: descriptor
                .outputs
                .iter()
                .map(|port| OutputPort {
                    signal: port.signal,
                    destinations: Vec::new(),
                })
                .collect(),
            schema: descriptor,
        }
    }
    pub fn inputs(&self) -> &[SignalType] {
        &self.inputs
    }
    pub fn outputs(&self) -> &[OutputPort] {
        &self.outputs
    }
}

impl super::Graph {
    pub fn schema(&self, id: NodeId) -> Result<&NodeSchema, super::GraphError> {
        Ok(&self.ports[self.index(id)?].schema)
    }
    /// Port instances are in the same dense order as `nodes()`.
    pub fn ports(&self) -> &[NodePorts] {
        &self.ports
    }
    pub fn input(&self, node: NodeId, port: usize) -> Result<InputPortRef, super::GraphError> {
        self.ports[self.index(node)?]
            .inputs
            .get(port)
            .ok_or(super::GraphError::InvalidPort)?;
        Ok(InputPortRef { node, port })
    }
    pub fn output(&self, node: NodeId, port: usize) -> Result<OutputPortRef, super::GraphError> {
        self.ports[self.index(node)?]
            .outputs
            .get(port)
            .ok_or(super::GraphError::InvalidPort)?;
        Ok(OutputPortRef { node, port })
    }
    pub fn destinations(
        &self,
        output: OutputPortRef,
    ) -> Result<&[InputPortRef], super::GraphError> {
        self.ports[self.index(output.node)?]
            .outputs
            .get(output.port)
            .map(|port| port.destinations())
            .ok_or(super::GraphError::InvalidPort)
    }
    pub(super) fn prune_registrations(&mut self, removed: impl Fn(NodeId) -> bool) {
        self.connection_count = 0;
        for ports in &mut self.ports {
            for output in &mut ports.outputs {
                output.destinations.retain(|input| !removed(input.node));
                self.connection_count += output.destinations.len();
            }
        }
    }
}
