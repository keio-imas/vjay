use crate::{
    control_io::ControlIo,
    ports::PortResult,
    signals::{SignalEngine, ValueRef},
};
use std::collections::BTreeMap;
use vjay_domain::{
    graph::{Graph, NodeId},
    node::NodeFactory,
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServiceRequest {
    pub node: NodeId,
    pub service: &'static str,
    pub properties: BTreeMap<String, String>,
}
pub struct ServiceControls {
    pub node: NodeId,
    pub values: Vec<f32>,
}
#[derive(Default)]
pub(crate) struct ServicePlan {
    controls: Vec<ServiceControls>,
    bindings: Vec<Vec<ValueRef>>,
}
impl ServicePlan {
    pub fn rebuild(
        &mut self,
        graph: &Graph,
        nodes: &dyn NodeFactory,
        signals: &SignalEngine,
        io: &mut dyn ControlIo,
    ) -> PortResult<()> {
        let mut requests = Vec::new();
        self.controls.clear();
        self.bindings.clear();
        for node in graph.nodes() {
            if let Some(service) = nodes.get(&node.type_id)?.service() {
                requests.push(ServiceRequest {
                    node: node.id,
                    service,
                    properties: node.settings.properties.clone(),
                });
                self.controls.push(ServiceControls {
                    node: node.id,
                    values: node.parameters.clone(),
                });
                self.bindings.push(
                    (0..node.parameters.len())
                        .map(|i| signals.parameter(node.id, i).expect("compiled parameter"))
                        .collect(),
                );
            }
        }
        io.prepare_services(&requests)
    }
    pub fn update(&mut self, signals: &SignalEngine, io: &mut dyn ControlIo) {
        for (control, bindings) in self.controls.iter_mut().zip(&self.bindings) {
            for (value, binding) in control.values.iter_mut().zip(bindings) {
                *value = binding.read(&signals.values);
            }
        }
        io.update_services(&self.controls);
    }
}
