use crate::{
    bindings::{OutputRef, PortBindings},
    ports::PortResult,
};
use vjay_domain::{graph::Graph, port::SignalType};
/// One topological edit-time pass filters incomplete preview branches. A missing
/// file/input in a disconnected node must not invalidate a working Program.
pub(super) fn roots(
    graph: &Graph,
    order: &[usize],
    bindings: &PortBindings,
) -> PortResult<Vec<OutputRef>> {
    let mut ready = vec![false; graph.nodes().len()];
    let mut result = Vec::new();
    for &index in order {
        let node = &graph.nodes()[index];
        let schema = graph.schema(node.id).map_err(|e| e.to_string())?;
        let complete = (!(schema.capabilities.media.is_some() || schema.capabilities.still_image)
            || node.asset.is_some())
            && schema
                .inputs
                .iter()
                .enumerate()
                .filter(|(_, p)| p.signal == SignalType::Texture)
                .all(|(p, port)| {
                    bindings
                        .source(index, p)
                        .map_or(port.optional, |s| ready[s.node])
                });
        ready[index] = complete;
        if complete
            && node.settings.preview
            && let Some(port) = schema
                .outputs
                .iter()
                .position(|p| p.signal == SignalType::Texture)
        {
            result.push(OutputRef { node: index, port });
        }
    }
    Ok(result)
}
