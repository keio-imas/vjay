use super::*;
pub(super) struct Root {
    pub node: NodeId,
    pub source: OutputRef,
    pub stream: Option<vjay_domain::execution::StreamRequest>,
}
pub(super) fn roots(
    g: &Graph,
    bindings: &PortBindings,
    f: &dyn NodeFactory,
    primary: Option<NodeId>,
) -> PortResult<Vec<Root>> {
    let mut roots = Vec::new();
    for (index, n) in g.nodes().iter().enumerate() {
        if let Some(input) = g
            .schema(n.id)
            .map_err(|e| e.to_string())?
            .capabilities
            .program_output
            && let Some(source) = bindings.source(index, input)
        {
            let stream = f.get(&n.type_id)?.video_sink(&n.settings).map(|mut r| {
                if r.name.is_empty() {
                    r.name = format!("VJAY #{}", n.id.0);
                }
                r
            });
            if Some(n.id) != primary
                && stream
                    .as_ref()
                    .is_some_and(|r| r.options.get("enabled").is_some_and(|v| v != "On"))
            {
                continue;
            }
            roots.push(Root {
                node: n.id,
                source,
                stream,
            });
        }
    }
    Ok(roots)
}
