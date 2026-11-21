use super::*;
use crate::{
    bindings::{OutputRef, PortBindings},
    ports::PortResult,
};
use vjay_domain::{graph::Graph, node::NodeFactory, port::SignalType};

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
        self.aliases.clear();
        self.parameters.clear();
        self.inputs.clear();
        self.input.media.retain(|n, _| live.contains(n));
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
            if descriptor.capabilities.transport.is_some() {
                self.input.media.entry(record.id).or_default();
            }
            for (p, input) in descriptor.inputs.iter().enumerate() {
                if input.signal == SignalType::Scalar {
                    self.inputs.insert(
                        (record.id, p),
                        ValueRef {
                            constant_slot: None,
                            source: bindings
                                .source(node_index, p)
                                .and_then(|key| slots[bindings.output(key)]),
                            value: 0.0,
                            min: -1e6,
                            max: 1e6,
                            wrap: None,
                        },
                    );
                }
            }

            for (index, parameter) in descriptor.parameters.iter().enumerate() {
                let constant_slot = self.values.len();
                self.values.push(record.parameters[index]);
                let source = bindings
                    .source(node_index, descriptor.inputs.len() + index)
                    .and_then(|key| slots[bindings.output(key)]);
                self.parameters.insert(
                    (record.id, index),
                    ValueRef {
                        constant_slot: Some(constant_slot),
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
            let feedback = node.feedback();
            let is_feedback = feedback.is_some();
            let Some(kernel) = feedback.or_else(|| node.scalar()) else {
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
                feedback: is_feedback,
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
        self.steps.sort_by_key(|step| !step.feedback);
        Ok(())
    }
}
