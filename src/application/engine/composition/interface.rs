use super::*;
use vjay_domain::composition::{Boundary, Direction, Interface};
pub fn boundaries(
    project: &Project,
    factory: &dyn NodeFactory,
    direction: Direction,
) -> PortResult<Vec<(NodeId, Boundary)>> {
    let mut result = Vec::new();
    for node in project.graph.nodes() {
        if let Some(b) = factory.get(&node.type_id)?.boundary(&node.settings)
            && b.direction == direction
        {
            result.push((node.id, b));
        }
    }
    result.sort_by_key(|(id, b)| (b.order, id.0));
    if result.len() > 32 {
        return Err("Subpatch supports at most 32 ports per direction".into());
    }
    let mut keys = std::collections::HashSet::new();
    for (id, b) in &mut result {
        if b.port.key.is_empty() {
            b.port.key = format!("port-{}", id.0);
        }
        if !keys.insert(b.port.key.clone()) {
            return Err("Duplicate subpatch port identity".into());
        }
    }
    Ok(result)
}
pub fn of(project: &Project, factory: &dyn NodeFactory) -> PortResult<Interface> {
    let inputs = boundaries(project, factory, Direction::In)?
        .into_iter()
        .map(|(_, b)| b.port)
        .collect();
    let outputs = boundaries(project, factory, Direction::Out)?
        .into_iter()
        .map(|(id, mut b)| {
            b.port.delayed = b.port.signal == vjay_domain::port::SignalType::Scalar
                && project
                    .graph
                    .connections()
                    .find(|e| e.to == id && e.input == 0)
                    .is_some_and(|e| {
                        delayed_path(project, e.from, e.output, &mut Default::default())
                    });
            b.port
        })
        .collect();
    Ok(Interface { inputs, outputs })
}

fn delayed_path(
    project: &Project,
    node: NodeId,
    port: usize,
    seen: &mut std::collections::HashSet<(NodeId, usize)>,
) -> bool {
    if !seen.insert((node, port)) {
        return false;
    }
    if project
        .graph
        .schema(node)
        .ok()
        .and_then(|s| s.outputs.get(port))
        .is_some_and(|p| p.delayed)
    {
        return true;
    }
    project
        .graph
        .connections()
        .filter(|e| e.to == node)
        .any(|e| delayed_path(project, e.from, e.output, seen))
}
