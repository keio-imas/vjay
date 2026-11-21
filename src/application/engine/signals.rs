use crate::control_io::*;
use std::collections::HashMap;
use vjay_domain::{
    execution::{ScalarContext, ScalarEvaluate, ScalarTap},
    graph::NodeId,
    settings::NodeSettings,
};
mod compilation;
mod files;
mod input;
mod parameters;
mod render;
mod text;

#[derive(Clone, Copy, Debug)]
pub struct ValueRef {
    pub source: Option<usize>,
    pub constant_slot: Option<usize>,
    pub value: f32,
    pub min: f32,
    pub max: f32,
    pub wrap: Option<f32>,
}
impl ValueRef {
    pub fn read(self, values: &[f32]) -> f32 {
        let value = self
            .source
            .or(self.constant_slot)
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
    feedback: bool,
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
    aliases: Vec<((NodeId, usize), (NodeId, usize))>,
    text: text::TextEngine,
    files: files::FileEngine,
    text_revision: u64,
    steps: Vec<SignalStep>,
    states: Vec<f64>,
    pub values: Vec<f32>,
    outputs: HashMap<(NodeId, usize), usize>,
    inputs: HashMap<(NodeId, usize), ValueRef>,
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
    pub fn set_media_feedback(&mut self, node: NodeId, state: vjay_domain::transport::MediaState) {
        if let Some(value) = self.input.media.get_mut(&node) {
            *value = state;
        }
    }
    pub fn input(&self, node: NodeId, port: usize) -> Option<ValueRef> {
        self.inputs.get(&(node, port)).copied()
    }
    pub fn alias(&mut self, alias: (NodeId, usize), source: (NodeId, usize)) {
        if let Some(slot) = self.outputs.get(&source).copied() {
            self.outputs.insert(alias, slot);
        }
        self.aliases.push((alias, source));
    }
    fn source_alias(&self, node: NodeId, port: usize) -> (NodeId, usize) {
        self.aliases
            .iter()
            .find(|(a, _)| *a == (node, port))
            .map_or((node, port), |(_, s)| *s)
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
        let delta = self.last_time.map_or(0.0, |last| (seconds - last).max(0.0));
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
        let (node, p) = self.source_alias(node, p);
        self.files.output(node, p)
    }
    pub fn text_output(&self, node: NodeId, port: usize) -> Option<&str> {
        let (node, port) = self.source_alias(node, port);
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
