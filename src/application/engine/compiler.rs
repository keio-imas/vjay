use crate::bindings::{OutputRef, PortBindings};
use crate::{ports::PortResult, signals::ValueRef};
use vjay_domain::{
    execution::ImageKernel,
    graph::{Graph, NodeId},
    node::NodeFactory,
    port::SignalType,
};

#[derive(Clone, Debug)]
pub struct MediaPlayback {
    pub speed: f32,
    pub position: f32,
    pub window: crate::media::PlaybackWindow,
}

#[derive(Clone, Debug)]
pub struct RenderStep {
    pub preview: Option<usize>,
    pub extra_previews: Vec<usize>,
    pub program_required: bool,
    pub node: NodeId,
    pub output: usize,
    pub operation: ImageKernel,
    pub inputs: [Option<usize>; 8],
    pub target: usize,
    pub asset: Option<vjay_domain::node::AssetId>,
    pub still_image: bool,
    pub controls: Vec<(usize, ValueRef)>,
    pub text_controls: Vec<(String, usize, Option<std::sync::Arc<str>>)>,
    pub prepared_revision: u64,
    pub prepared_controls: Vec<(String, ValueRef, Option<f32>, Option<f32>)>,
    pub uniform_controls: Vec<(usize, ValueRef)>,
    pub media_time: Option<f64>,
    pub playback: Option<MediaPlayback>,
}
impl RenderStep {
    pub fn external_source(&self) -> bool {
        self.operation.external_passthrough
            && (self.asset.is_some()
                || self.operation.prepared.is_some()
                || self.operation.stream.is_some())
    }
}
#[derive(Clone, Debug)]
pub struct RenderPlan {
    pub outputs: Vec<RenderOutput>,
    pub previews: Vec<RenderPreview>,
    pub program_active: bool,
    pub steps: Vec<RenderStep>,
    pub texture_count: usize,
    pub output_slot: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RenderOutput {
    pub node: NodeId,
    pub slot: usize,
    pub stream: Option<vjay_domain::execution::StreamRequest>,
}
#[derive(Clone, Debug)]
pub struct RenderPreview {
    pub node: NodeId,
    pub step: usize,
}
mod allocation;
mod kernels;
mod outputs;
mod previews;

/// Compile only the texture outputs reachable from Program. Scalar and audio
/// outputs are evaluated by their own plans; port count is a node contract.
pub fn compile(
    graph: &Graph,
    output: impl Into<Option<NodeId>>,
    factory: &dyn NodeFactory,
) -> PortResult<RenderPlan> {
    compile_plan(graph, output.into(), factory, false)
}
pub fn compile_with_previews(
    graph: &Graph,
    output: Option<NodeId>,
    factory: &dyn NodeFactory,
) -> PortResult<RenderPlan> {
    compile_plan(graph, output, factory, true)
}
fn compile_plan(
    graph: &Graph,
    output: Option<NodeId>,
    factory: &dyn NodeFactory,
    include_previews: bool,
) -> PortResult<RenderPlan> {
    let order = graph.topological_order().map_err(|e| e.to_string())?;
    let bindings = PortBindings::new(graph)?;
    let source = if let Some(output) = output {
        let index = graph.index(output).map_err(|e| e.to_string())?;
        let schema = graph.schema(output).map_err(|e| e.to_string())?;
        let input = schema
            .capabilities
            .program_output
            .ok_or("The designated output needs a Program output capability")?;
        bindings.source(index, input)
    } else {
        None
    };
    let output_roots = outputs::roots(graph, &bindings, factory, output)?;
    let roots = if include_previews {
        previews::roots(graph, &order, &bindings)?
    } else {
        Vec::new()
    };
    let mut preview_indices = vec![None; bindings.output_count()];
    for (index, &root) in roots.iter().enumerate() {
        preview_indices[bindings.output(root)] = Some(index);
    }
    let mut previews = roots
        .iter()
        .map(|r| RenderPreview {
            node: graph.nodes()[r.node].id,
            step: 0,
        })
        .collect::<Vec<_>>();
    let mut live = vec![false; bindings.output_count()];
    let mut program_live = vec![false; bindings.output_count()];
    let mut stack = roots.into_iter().map(|r| (r, false)).collect::<Vec<_>>();
    stack.extend(source.map(|r| (r, true)));
    stack.extend(output_roots.iter().map(|r| (r.source, true)));
    while let Some((key, required)) = stack.pop() {
        if required {
            program_live[bindings.output(key)] = true;
        }
        if std::mem::replace(&mut live[bindings.output(key)], true) {
            continue;
        }
        let record = &graph.nodes()[key.node];
        let descriptor = graph.schema(record.id).map_err(|e| e.to_string())?;
        if descriptor
            .outputs
            .get(key.port)
            .is_none_or(|port| port.signal != SignalType::Texture)
        {
            return Err("Program requires a texture output".into());
        }
        for (port, input) in descriptor.inputs.iter().enumerate() {
            if input.signal != SignalType::Texture {
                continue;
            }
            if let Some(dependency) = bindings.source(key.node, port) {
                stack.push((dependency, required));
            } else if !input.optional {
                return Err(format!(
                    "{} #{}: connect {}",
                    descriptor.name, record.id.0, input.name
                ));
            }
        }
    }
    let mut mapping = vec![None; bindings.output_count()];
    let mut dependencies = Vec::new();
    let mut steps = Vec::new();
    for index in order {
        let record = &graph.nodes()[index];
        let node = factory.get(&record.type_id)?;
        let descriptor = graph.schema(record.id).map_err(|e| e.to_string())?;
        for (output, port) in descriptor.outputs.iter().enumerate() {
            if port.signal != SignalType::Texture
                || !live[bindings.output(OutputRef {
                    node: index,
                    port: output,
                })]
            {
                continue;
            }
            if descriptor.capabilities.still_image
                && descriptor.capabilities.file_input.is_none()
                && record.asset.is_none()
            {
                return Err(format!("Video #{}: choose a video file", record.id.0));
            }
            let mut dependency = [None; 8];
            let mut texture = 0;
            for (input, port) in descriptor.inputs.iter().enumerate() {
                if port.signal != SignalType::Texture {
                    continue;
                }
                if texture >= 8 {
                    return Err("This GPU adapter supports eight texture inputs per pass".into());
                }
                dependency[texture] = bindings
                    .source(index, input)
                    .and_then(|key| mapping[bindings.output(key)]);
                texture += 1;
            }
            mapping[bindings.output(OutputRef {
                node: index,
                port: output,
            })] = Some(steps.len());
            let preview = preview_indices[bindings.output(OutputRef {
                node: index,
                port: output,
            })];
            if let Some(p) = preview {
                previews[p].step = steps.len();
            }
            dependencies.push(dependency);
            let operation = node
                .image(&record.parameters, &record.settings, output)
                .ok_or_else(|| {
                    format!(
                        "{} has no image kernel for output {}",
                        descriptor.name, output
                    )
                })?;
            kernels::validate(&operation, descriptor.parameters.len())?;
            steps.push(RenderStep {
                preview,
                extra_previews: Vec::new(),
                program_required: program_live[bindings.output(OutputRef {
                    node: index,
                    port: output,
                })],
                node: record.id,
                output,
                operation,
                inputs: [None; 8],
                target: 0,
                asset: record.asset,
                still_image: descriptor.capabilities.still_image && record.asset.is_some(),
                controls: Vec::new(),
                uniform_controls: Vec::new(),
                text_controls: Vec::new(),
                prepared_revision: 0,
                prepared_controls: Vec::new(),
                media_time: None,
                playback: descriptor.capabilities.media.map(|binding| MediaPlayback {
                    speed: record.parameters[binding.speed],
                    position: record.parameters[binding.position],
                    window: crate::media::PlaybackWindow {
                        looping: binding.looping.is_none_or(|i| record.parameters[i] >= 0.5),
                        start: binding.start.map_or(0.0, |i| record.parameters[i]),
                        end: binding.end.map_or(1.0, |i| record.parameters[i]),
                    },
                }),
            });
        }
    }
    let final_step = source
        .map(|s| mapping[bindings.output(s)].ok_or("Output has no renderable source"))
        .transpose()?;
    let retained = output_roots
        .iter()
        .filter_map(|r| mapping[bindings.output(r.source)])
        .collect::<Vec<_>>();
    let (count, output_slot) = allocation::assign(&mut steps, &dependencies, &retained, final_step);
    Ok(RenderPlan {
        outputs: output_roots
            .into_iter()
            .filter_map(|r| {
                mapping[bindings.output(r.source)].map(|i| RenderOutput {
                    node: r.node,
                    slot: steps[i].target,
                    stream: r.stream,
                })
            })
            .collect(),
        previews,
        program_active: final_step.is_some(),
        output_slot,
        steps,
        texture_count: count,
    })
}

#[cfg(test)]
mod compiler_tests;
