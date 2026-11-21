use super::*;
use std::collections::BTreeMap;
use vjay_domain::{composition::Direction, graph::Connection, port::SignalType};
type End = (NodeId, usize);
#[derive(Default)]
pub struct Expanded {
    pub graph: Graph,
    pub assets: Vec<crate::media::MediaAsset>,
    pub previews: Vec<(NodeId, End)>,
    pub routes: Vec<(End, End)>,
}
pub fn expand(project: &Project, factory: &dyn NodeFactory) -> PortResult<Expanded> {
    let mut out = Expanded::default();
    let mut aliases = BTreeMap::<(u64, usize), Option<End>>::new();
    let mut edges = Vec::new();
    let mut preview = Vec::new();
    collect(
        project,
        factory,
        &mut out,
        &mut aliases,
        &mut edges,
        &mut preview,
        0,
    )?;
    let resolve = |mut end: End| -> PortResult<Option<End>> {
        for _ in 0..=aliases.len() {
            match aliases.get(&(end.0.0, end.1)) {
                Some(Some(next)) => end = *next,
                Some(None) => return Ok(None),
                None => return Ok(Some(end)),
            }
        }
        Err("Subpatch routing contains an instantaneous cycle".into())
    };
    for mut edge in edges {
        if let Some((node, port)) = resolve((edge.from, edge.output))? {
            edge.from = node;
            edge.output = port;
            out.graph.connect(edge).map_err(|e| e.to_string())?;
        }
    }
    for &(node, port) in aliases.keys() {
        if let Some(real) = resolve((NodeId(node), port))? {
            out.routes.push(((NodeId(node), port), real));
        }
    }
    for (node, source) in preview {
        if let Some(end) = resolve(source)? {
            let record = out.graph.node(end.0).map_err(|e| e.to_string())?;
            if out.graph.schema(end.0).map_err(|e| e.to_string())?.outputs[end.1].signal
                == SignalType::Texture
            {
                let mut settings = record.settings.clone();
                settings.preview = true;
                out.graph
                    .set_settings(end.0, settings, factory)
                    .map_err(|e| e.to_string())?;
                out.previews.push((node, end));
            }
        }
    }
    Ok(out)
}
fn collect(
    p: &Project,
    f: &dyn NodeFactory,
    out: &mut Expanded,
    aliases: &mut BTreeMap<(u64, usize), Option<End>>,
    edges: &mut Vec<Connection>,
    preview: &mut Vec<(NodeId, End)>,
    depth: usize,
) -> PortResult<()> {
    if depth > 8 {
        return Err("Subpatch nesting exceeds limits".into());
    }
    out.assets.extend(p.assets.iter().cloned());
    for n in p.graph.nodes() {
        let definition = f.get(&n.type_id)?;
        if let Some(boundary) = definition.boundary(&n.settings) {
            if boundary.direction == Direction::In {
                aliases.entry((n.id.0, 0)).or_insert(None);
            }
            continue;
        }
        if definition.composite() {
            let child = p
                .subpatches
                .iter()
                .find(|s| s.node == n.id)
                .ok_or("Subpatch definition is missing")?;
            for (port, (id, _)) in interface::boundaries(&child.project, f, Direction::In)?
                .iter()
                .enumerate()
            {
                let source = p
                    .graph
                    .connections()
                    .find(|e| e.to == n.id && e.input == port)
                    .map(|e| (e.from, e.output));
                aliases.insert((id.0, 0), source);
            }
            for (port, (id, _)) in interface::boundaries(&child.project, f, Direction::Out)?
                .iter()
                .enumerate()
            {
                let source = child
                    .project
                    .graph
                    .connections()
                    .find(|e| e.to == *id && e.input == 0)
                    .map(|e| (e.from, e.output));
                aliases.insert((n.id.0, port), source);
                if n.settings.preview
                    && !preview.iter().any(|(id, _)| *id == n.id)
                    && p.graph.schema(n.id).map_err(|e| e.to_string())?.outputs[port].signal
                        == SignalType::Texture
                {
                    preview.push((n.id, (n.id, port)));
                }
            }
            collect(&child.project, f, out, aliases, edges, preview, depth + 1)?;
        } else {
            out.graph.insert(n.clone(), f).map_err(|e| e.to_string())?;
        }
    }
    for e in p.graph.connections() {
        let target = p.graph.node(e.to).map_err(|e| e.to_string())?;
        let def = f.get(&target.type_id)?;
        if !def.composite() && def.boundary(&target.settings).is_none() {
            edges.push(e);
        }
    }
    Ok(())
}
