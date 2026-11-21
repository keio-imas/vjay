use crate::{
    bindings::{OutputRef, PortBindings},
    ports::PortResult,
};
use std::{collections::HashMap, sync::Arc};
use vjay_domain::{
    execution::{SignalInputs, TextContext, TextEvaluate},
    graph::{Graph, NodeId},
    node::NodeFactory,
    port::SignalType,
};
struct Step {
    node: NodeId,
    inputs: Vec<Option<usize>>,
    outputs: Vec<usize>,
    evaluate: TextEvaluate,
    state: Vec<Arc<str>>,
}
#[derive(Default)]
pub(super) struct TextEngine {
    steps: Vec<Step>,
    values: Vec<Arc<str>>,
    outputs: HashMap<(NodeId, usize), usize>,
    inputs: Vec<Option<Arc<str>>>,
    input_slots: HashMap<(NodeId, usize), usize>,
    scratch: Vec<Arc<str>>,
}
impl TextEngine {
    pub fn rebuild(&mut self, graph: &Graph, nodes: &dyn NodeFactory) -> PortResult<()> {
        self.steps.clear();
        self.values.clear();
        self.outputs.clear();
        self.input_slots.clear();
        let bindings = PortBindings::new(graph)?;
        let mut slots = vec![None; bindings.output_count()];
        for (index, node) in graph.nodes().iter().enumerate() {
            for (port, p) in graph
                .schema(node.id)
                .map_err(|e| e.to_string())?
                .outputs
                .iter()
                .enumerate()
            {
                if p.signal == SignalType::Text {
                    let slot = self.values.len();
                    self.values.push(Arc::from(""));
                    slots[bindings.output(OutputRef { node: index, port })] = Some(slot);
                    self.outputs.insert((node.id, port), slot);
                }
            }
        }
        for index in graph.topological_order().map_err(|e| e.to_string())? {
            let record = &graph.nodes()[index];
            let node = nodes.get(&record.type_id)?;
            let d = graph.schema(record.id).map_err(|e| e.to_string())?;
            let inputs: Vec<_> = d
                .inputs
                .iter()
                .enumerate()
                .filter(|(_, p)| p.signal == SignalType::Text)
                .map(|(p, _)| {
                    bindings
                        .source(index, p)
                        .and_then(|r| slots[bindings.output(r)])
                })
                .collect();
            for (port, source) in d
                .inputs
                .iter()
                .enumerate()
                .filter(|(_, p)| p.signal == SignalType::Text)
                .map(|(p, _)| {
                    (
                        p,
                        bindings
                            .source(index, p)
                            .and_then(|r| slots[bindings.output(r)]),
                    )
                })
            {
                if let Some(source) = source {
                    self.input_slots.insert((record.id, port), source);
                }
            }
            let Some(kernel) = node.text(&record.settings) else {
                continue;
            };
            let outputs: Vec<_> = d
                .outputs
                .iter()
                .enumerate()
                .filter_map(|(port, _)| slots[bindings.output(OutputRef { node: index, port })])
                .collect();
            self.inputs
                .resize(self.inputs.len().max(inputs.len()), None);
            self.scratch
                .resize(self.scratch.len().max(outputs.len()), Arc::from(""));
            self.steps.push(Step {
                node: record.id,
                inputs,
                outputs,
                evaluate: kernel.evaluate,
                state: kernel.state,
            });
        }
        Ok(())
    }
    pub fn tick(&mut self, host: &dyn SignalInputs) {
        for step in &mut self.steps {
            for (value, source) in self.inputs.iter_mut().zip(&step.inputs) {
                *value = source.map(|i| self.values[i].clone());
            }
            let output = &mut self.scratch[..step.outputs.len()];
            output.fill(vjay_domain::execution::empty_text());
            (step.evaluate)(
                &TextContext {
                    node: step.node,
                    inputs: &self.inputs[..step.inputs.len()],
                    host,
                },
                &mut step.state,
                output,
            );
            for (&slot, value) in step.outputs.iter().zip(output) {
                self.values[slot] = value.clone();
            }
        }
    }
    pub fn input_slot(&self, node: NodeId, port: usize) -> Option<usize> {
        self.input_slots.get(&(node, port)).copied()
    }
    pub fn value(&self, slot: usize) -> &Arc<str> {
        &self.values[slot]
    }
    pub fn output(&self, node: NodeId, port: usize) -> Option<&str> {
        self.outputs
            .get(&(node, port))
            .map(|&i| self.values[i].as_ref())
    }
}
