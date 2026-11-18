use crate::ports::PortResult;
use std::collections::HashMap;
use vjay_domain::{
    graph::{Connection, NodeId, NodeRecord},
    node::{Node, NodeFactory},
};
pub(crate) struct PortMigrations<'a>(HashMap<NodeId, (&'a dyn Node, u32)>);
impl<'a> PortMigrations<'a> {
    pub fn new(records: &[NodeRecord], nodes: &'a dyn NodeFactory) -> PortResult<Self> {
        let mut entries = HashMap::with_capacity(records.len());
        for record in records {
            entries.insert(
                record.id,
                (nodes.get(&record.type_id)?, record.settings.port_version),
            );
        }
        Ok(Self(entries))
    }
    pub fn connection(&self, mut edge: Connection) -> PortResult<Connection> {
        let (from, version) = self.0.get(&edge.from).ok_or("Unknown connection source")?;
        edge.output = from.migrate_output_port(edge.output, *version);
        let (to, version) = self.0.get(&edge.to).ok_or("Unknown connection target")?;
        edge.input = to.migrate_input_port(edge.input, *version);
        Ok(edge)
    }
}
