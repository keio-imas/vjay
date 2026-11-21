use crate::ports::PortResult;
use vjay_domain::{graph::Graph, node::NodeFactory, transport::Bookmark};
/// Legacy document migration only; playback never discovers or mutates a peer.
pub(crate) fn cues(graph: &mut Graph, factory: &dyn NodeFactory) -> PortResult<()> {
    let mut changes = Vec::new();
    for video in graph.nodes() {
        if video.settings.transport.is_some() {
            continue;
        }
        let schema = graph.schema(video.id).map_err(|e| e.to_string())?;
        let Some(media) = schema.capabilities.media else {
            continue;
        };
        let offset = schema.inputs.len();
        let Some(edge) = graph
            .connections()
            .find(|e| e.to == video.id && e.input == offset + media.speed)
        else {
            continue;
        };
        let source = graph.node(edge.from).map_err(|e| e.to_string())?;
        let Some(sync) = graph
            .schema(source.id)
            .map_err(|e| e.to_string())?
            .capabilities
            .tempo_sync
        else {
            continue;
        };
        let Some(manual) = sync.manual else {
            continue;
        };
        if edge.output != sync.outputs[0]
            || ![
                (sync.outputs[1], Some(media.position)),
                (sync.outputs[2], media.seek),
            ]
            .iter()
            .all(|(output, input)| {
                input.is_some_and(|input| {
                    graph.connections().any(|e| {
                        e.from == source.id
                            && e.output == *output
                            && e.to == video.id
                            && e.input == offset + input
                    })
                })
            })
        {
            continue;
        }
        let mut settings = video.settings.clone();
        settings.transport = Some(Bookmark {
            asset: video.asset,
            cue: source.parameters[manual.marker],
        });
        changes.push((video.id, settings));
    }
    for (id, settings) in changes {
        graph
            .set_settings(id, settings, factory)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
