use crate::{
    bindings::{OutputRef, PortBindings},
    control_io::*,
    ports::PortResult,
};
use std::collections::HashMap;
use vjay_domain::{
    execution::{ScalarContext, ScalarEvaluate, ScalarTap},
    graph::{Graph, NodeId},
    node::NodeFactory,
    port::SignalType,
    settings::NodeSettings,
};
mod files;
mod input;
mod render;
mod text;

#[derive(Clone, Copy, Debug)]
pub struct ValueRef {
    pub source: Option<usize>,
    pub value: f32,
    pub min: f32,
    pub max: f32,
    pub wrap: Option<f32>,
}
impl ValueRef {
    pub fn read(self, values: &[f32]) -> f32 {
        let value = self
            .source
            .and_then(|i| values.get(i))
            .copied()
            .unwrap_or(self.value);
        if value.is_finite() {
            self.wrap.map_or(value.clamp(self.min, self.max), |period| {
                ((value as f64 - self.min as f64).rem_euclid(period as f64) + self.min as f64)
                    as f32
            })
        } else {
            self.value
        }
    }
}
struct SignalStep {
    node: NodeId,
    type_id: String,
    settings: NodeSettings,
    parameters: Vec<ValueRef>,
    inputs: Vec<Option<usize>>,
    outputs: Vec<usize>,
    state: std::ops::Range<usize>,
    evaluate: ScalarEvaluate,
    tap: Option<ScalarTap>,
}
#[derive(Default)]
pub struct SignalEngine {
    text: text::TextEngine,
    files: files::FileEngine,
    text_revision: u64,
    steps: Vec<SignalStep>,
    states: Vec<f64>,
    pub values: Vec<f32>,
    outputs: HashMap<(NodeId, usize), usize>,
    parameters: HashMap<(NodeId, usize), ValueRef>,
    capabilities: HashMap<NodeId, vjay_domain::capabilities::Capabilities>,
    input: input::InputState,
    pub events: Vec<ControlEvent>,
    last_time: Option<f64>,
    parameter_scratch: Vec<f32>,
    input_scratch: Vec<Option<f32>>,
    output_scratch: Vec<f32>,
}
impl SignalEngine {
    pub fn rebuild(&mut self, graph: &Graph, factory: &dyn NodeFactory) -> PortResult<()> {
        self.text.rebuild(graph, factory)?;
        self.files.rebuild(graph, factory)?;
        self.text_revision = self.text_revision.wrapping_add(1);
        let live: std::collections::HashSet<_> = graph.nodes().iter().map(|n| n.id).collect();
        self.input.values.retain(|n, _| live.contains(n));
        self.input.files.retain(|n, _| live.contains(n));
        self.input.text.retain(|n, _| live.contains(n));
        let old_states: HashMap<_, _> = self
            .steps
            .iter()
            .map(|step| {
                (
                    (step.node, step.type_id.clone()),
                    self.states[step.state.clone()].to_vec(),
                )
            })
            .collect();
        self.steps.clear();
        self.states.clear();
        self.outputs.clear();
        self.parameters.clear();
        self.capabilities.clear();
        let bindings = PortBindings::new(graph)?;
        let mut slots = vec![None; bindings.output_count()];
        let mut count = 0;
        for (index, record) in graph.nodes().iter().enumerate() {
            for (port, descriptor) in graph
                .schema(record.id)
                .map_err(|e| e.to_string())?
                .outputs
                .iter()
                .enumerate()
            {
                if descriptor.signal == SignalType::Scalar {
                    slots[bindings.output(OutputRef { node: index, port })] = Some(count);
                    self.outputs.insert((record.id, port), count);
                    count += 1;
                }
            }
        }
        self.values.clear();
        self.values.resize(count, 0.0);
        for (node_index, record) in graph.nodes().iter().enumerate() {
            let descriptor = graph.schema(record.id).map_err(|e| e.to_string())?;
            self.capabilities.insert(record.id, descriptor.capabilities);
            for (index, parameter) in descriptor.parameters.iter().enumerate() {
                let source = bindings
                    .source(node_index, descriptor.inputs.len() + index)
                    .and_then(|key| slots[bindings.output(key)]);
                self.parameters.insert(
                    (record.id, index),
                    ValueRef {
                        source,
                        value: record.parameters[index],
                        min: parameter.min,
                        max: parameter.max,
                        wrap: parameter.wrap,
                    },
                );
            }
        }
        for index in graph.topological_order().map_err(|e| e.to_string())? {
            let record = &graph.nodes()[index];
            let node = factory.get(&record.type_id)?;
            let Some(kernel) = node.scalar() else {
                continue;
            };
            let descriptor = graph.schema(record.id).map_err(|e| e.to_string())?;
            let start = self.states.len();
            let previous = old_states
                .get(&(record.id, record.type_id.to_string()))
                .filter(|s| s.len() == kernel.state.len());
            self.states
                .extend_from_slice(previous.map_or(&kernel.state, |s| s));
            self.parameter_scratch.resize(
                self.parameter_scratch
                    .len()
                    .max(descriptor.parameters.len()),
                0.0,
            );
            self.input_scratch
                .resize(self.input_scratch.len().max(descriptor.inputs.len()), None);
            self.output_scratch
                .resize(self.output_scratch.len().max(descriptor.outputs.len()), 0.0);
            self.steps.push(SignalStep {
                node: record.id,
                type_id: record.type_id.to_string(),
                settings: record.settings.clone(),
                parameters: (0..descriptor.parameters.len())
                    .map(|p| self.parameters[&(record.id, p)])
                    .collect(),
                inputs: (0..descriptor.inputs.len())
                    .map(|p| {
                        bindings
                            .source(index, p)
                            .and_then(|key| slots[bindings.output(key)])
                    })
                    .collect(),
                outputs: (0..descriptor.outputs.len())
                    .filter_map(|p| {
                        slots[bindings.output(OutputRef {
                            node: index,
                            port: p,
                        })]
                    })
                    .collect(),
                state: start..self.states.len(),
                evaluate: kernel.evaluate,
                tap: kernel.tap,
            });
        }
        Ok(())
    }
    pub fn capabilities(&self, node: NodeId) -> Option<vjay_domain::capabilities::Capabilities> {
        self.capabilities.get(&node).copied()
    }
    pub fn triggered(&self, node: NodeId) -> bool {
        self.input.triggers.contains(&node)
    }
    pub fn clear_midi(&mut self) {
        self.input.midi = Default::default();
        self.input.sources.clear();
    }
    pub fn tick(&mut self, seconds: f64) {
        let delta = self
            .last_time
            .map_or(0.0, |last| (seconds - last).clamp(0.0, 0.25));
        self.last_time = Some(seconds);
        self.input.begin_frame();
        for event in self.events.drain(..) {
            if let ControlEvent::Tap { node, seconds } = event {
                for step in &self.steps {
                    if let Some(tap) = step.tap
                        && node.is_none_or(|id| id == step.node)
                    {
                        tap(&mut self.states[step.state.clone()], seconds);
                    }
                }
            } else {
                self.input.apply(event);
            }
        }
        self.text.tick(&self.input);
        self.files.tick(&self.input);
        for step in &self.steps {
            for (index, value) in step.parameters.iter().enumerate() {
                self.parameter_scratch[index] = value.read(&self.values);
            }
            for (index, value) in step.inputs.iter().enumerate() {
                self.input_scratch[index] = value.map(|i| self.values[i]);
            }
            let output = &mut self.output_scratch[..step.outputs.len()];
            output.fill(0.0);
            (step.evaluate)(
                &ScalarContext {
                    node: step.node,
                    delta,
                    settings: &step.settings,
                    parameters: &self.parameter_scratch[..step.parameters.len()],
                    inputs: &self.input_scratch[..step.inputs.len()],
                    host: &self.input,
                },
                &mut self.states[step.state.clone()],
                output,
            );
            for (&index, &value) in step.outputs.iter().zip(output.iter()) {
                self.values[index] = if value.is_finite() { value } else { 0.0 };
            }
        }
    }
    pub fn file_input_slot(&self, node: NodeId, p: usize) -> Option<usize> {
        self.files.input(node, p)
    }
    pub fn file_value(&self, slot: usize) -> Option<&vjay_domain::execution::FileValue> {
        self.files.value(slot)
    }
    pub fn file_output(
        &self,
        node: NodeId,
        p: usize,
    ) -> Option<&vjay_domain::execution::FileValue> {
        self.files.output(node, p)
    }
    pub fn text_output(&self, node: NodeId, port: usize) -> Option<&str> {
        self.text.output(node, port)
    }
    pub fn output(&self, node: NodeId, port: usize) -> Option<f32> {
        self.outputs.get(&(node, port)).map(|&i| self.values[i])
    }
    pub fn parameter(&self, node: NodeId, parameter: usize) -> Option<ValueRef> {
        self.parameters.get(&(node, parameter)).copied()
    }
    pub fn value(&self, node: NodeId, parameter: usize) -> Option<f32> {
        self.parameter(node, parameter)
            .map(|v| v.read(&self.values))
    }
}
#[cfg(test)]
mod new_nodes_tests;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod text_tests;
