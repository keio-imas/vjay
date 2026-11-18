use crate::{
    bindings::{OutputRef, PortBindings},
    ports::PortResult,
};
use std::collections::HashMap;
use vjay_domain::{
    execution::{FileContext, FileEvaluate, FileValue, SignalInputs},
    graph::{Graph, NodeId},
    node::NodeFactory,
    port::SignalType,
};
struct Step {
    node: NodeId,
    inputs: Vec<Option<usize>>,
    outputs: Vec<usize>,
    evaluate: FileEvaluate,
}
#[derive(Default)]
pub(super) struct FileEngine {
    steps: Vec<Step>,
    values: Vec<Option<FileValue>>,
    inputs: HashMap<(NodeId, usize), usize>,
    outputs: HashMap<(NodeId, usize), usize>,
    input_scratch: Vec<Option<FileValue>>,
    output_scratch: Vec<Option<FileValue>>,
}
impl FileEngine {
    pub fn rebuild(&mut self, g: &Graph, f: &dyn NodeFactory) -> PortResult<()> {
        self.steps.clear();
        self.values.clear();
        self.inputs.clear();
        self.outputs.clear();
        let bindings = PortBindings::new(g)?;
        let mut slots = vec![None; bindings.output_count()];
        for (index, n) in g.nodes().iter().enumerate() {
            for (port, info) in g
                .schema(n.id)
                .map_err(|e| e.to_string())?
                .outputs
                .iter()
                .enumerate()
            {
                if info.signal == SignalType::File {
                    let slot = self.values.len();
                    self.values.push(None);
                    slots[bindings.output(OutputRef { node: index, port })] = Some(slot);
                    self.outputs.insert((n.id, port), slot);
                }
            }
        }
        for index in g.topological_order().map_err(|e| e.to_string())? {
            let n = &g.nodes()[index];
            let schema = g.schema(n.id).map_err(|e| e.to_string())?;
            let mut inputs = Vec::new();
            for (p, _) in schema
                .inputs
                .iter()
                .enumerate()
                .filter(|(_, p)| p.signal == SignalType::File)
            {
                let source = bindings
                    .source(index, p)
                    .and_then(|s| slots[bindings.output(s)]);
                if let Some(slot) = source {
                    self.inputs.insert((n.id, p), slot);
                }
                inputs.push(source);
            }
            let Some(evaluate) = f.get(&n.type_id)?.file() else {
                continue;
            };
            let outputs: Vec<_> = schema
                .outputs
                .iter()
                .enumerate()
                .filter_map(|(p, _)| {
                    slots[bindings.output(OutputRef {
                        node: index,
                        port: p,
                    })]
                })
                .collect();
            self.input_scratch
                .resize(self.input_scratch.len().max(inputs.len()), None);
            self.output_scratch
                .resize(self.output_scratch.len().max(outputs.len()), None);
            self.steps.push(Step {
                node: n.id,
                inputs,
                outputs,
                evaluate,
            });
        }
        Ok(())
    }
    pub fn tick(&mut self, host: &dyn SignalInputs) {
        for step in &self.steps {
            for (v, s) in self.input_scratch.iter_mut().zip(&step.inputs) {
                *v = s.and_then(|s| self.values[s].clone());
            }
            let out = &mut self.output_scratch[..step.outputs.len()];
            out.fill(None);
            (step.evaluate)(
                &FileContext {
                    node: step.node,
                    inputs: &self.input_scratch[..step.inputs.len()],
                    host,
                },
                out,
            );
            for (&s, v) in step.outputs.iter().zip(out) {
                self.values[s] = v.clone();
            }
        }
    }
    pub fn input(&self, node: NodeId, p: usize) -> Option<usize> {
        self.inputs.get(&(node, p)).copied()
    }
    pub fn value(&self, slot: usize) -> Option<&FileValue> {
        self.values.get(slot).and_then(Option::as_ref)
    }
    pub fn output(&self, node: NodeId, p: usize) -> Option<&FileValue> {
        self.outputs.get(&(node, p)).and_then(|s| self.value(*s))
    }
}
