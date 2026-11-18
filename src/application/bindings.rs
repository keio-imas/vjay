use crate::ports::PortResult;
use vjay_domain::graph::Graph;

/// Dense addresses only exist within one graph compilation, never in projects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct OutputRef {
    pub node: usize,
    pub port: usize,
}

#[cfg(test)]
mod tests;
pub(crate) struct PortBindings {
    input_offsets: Vec<usize>,
    output_offsets: Vec<usize>,
    sources: Vec<Option<OutputRef>>,
}
impl PortBindings {
    pub fn new(graph: &Graph) -> PortResult<Self> {
        let mut input_offsets = Vec::with_capacity(graph.nodes().len() + 1);
        let mut output_offsets = Vec::with_capacity(graph.nodes().len() + 1);
        input_offsets.push(0);
        output_offsets.push(0);
        for ports in graph.ports() {
            input_offsets.push(input_offsets.last().unwrap() + ports.inputs().len());
            output_offsets.push(output_offsets.last().unwrap() + ports.outputs().len());
        }
        let mut sources = vec![None; *input_offsets.last().unwrap()];
        for (node, ports) in graph.ports().iter().enumerate() {
            for (port, output) in ports.outputs().iter().enumerate() {
                for input in output.destinations() {
                    let target = graph.index(input.node).map_err(|e| e.to_string())?;
                    sources[input_offsets[target] + input.port] = Some(OutputRef { node, port });
                }
            }
        }
        Ok(Self {
            input_offsets,
            output_offsets,
            sources,
        })
    }
    pub fn source(&self, node: usize, input: usize) -> Option<OutputRef> {
        let start = *self.input_offsets.get(node)?;
        let end = *self.input_offsets.get(node + 1)?;
        (input < end - start)
            .then(|| self.sources[start + input])
            .flatten()
    }
    pub fn output(&self, output: OutputRef) -> usize {
        self.output_offsets[output.node] + output.port
    }
    pub fn output_count(&self) -> usize {
        *self.output_offsets.last().unwrap()
    }
}
