use crate::{ports::PortResult, signals::SignalEngine};
use vjay_domain::{
    execution::FileValue,
    graph::{Graph, NodeId},
};
struct Binding {
    node: NodeId,
    slot: usize,
    still: bool,
    last: Option<FileValue>,
    initialized: bool,
}
#[derive(Default)]
pub(crate) struct FileMedia {
    bindings: Vec<Binding>,
}
impl FileMedia {
    pub fn rebuild(&mut self, g: &Graph, s: &SignalEngine) -> PortResult<()> {
        let mut previous = std::mem::take(&mut self.bindings);
        for n in g.nodes() {
            let schema = g.schema(n.id).map_err(|e| e.to_string())?;
            if let Some(slot) = schema
                .capabilities
                .file_input
                .and_then(|p| s.file_input_slot(n.id, p))
            {
                let old = previous.iter_mut().find(|b| b.node == n.id);
                let initialized = old.as_ref().is_some_and(|b| b.initialized);
                let last = old.and_then(|b| b.last.take());
                self.bindings.push(Binding {
                    node: n.id,
                    slot,
                    still: schema.capabilities.still_image,
                    last,
                    initialized,
                });
            }
        }
        Ok(())
    }
    pub fn changes(&mut self, s: &SignalEngine) -> Vec<(NodeId, Option<std::path::PathBuf>, bool)> {
        let mut changes = Vec::new();
        for b in &mut self.bindings {
            let value = s.file_value(b.slot);
            if !b.initialized || value != b.last.as_ref() {
                b.initialized = true;
                changes.push((b.node, value.map(|v| v.to_path_buf()), b.still));
                b.last = value.cloned();
            }
        }
        changes
    }
}
