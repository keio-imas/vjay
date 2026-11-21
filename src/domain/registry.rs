use crate::node::{Node, NodeDescriptor, NodeFactory};
use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum NodeError {
    #[error("Node type is not registered: {0}")]
    Unknown(String),
    #[error("Node type is already registered: {0}")]
    Duplicate(String),
    #[error("Invalid node definition: {0}")]
    Invalid(String),
}
impl From<NodeError> for String {
    fn from(error: NodeError) -> Self {
        error.to_string()
    }
}
#[derive(Default)]
pub struct NodeRegistry {
    nodes: Vec<Box<dyn Node>>,
    descriptors: Vec<&'static NodeDescriptor>,
}
impl NodeRegistry {
    pub fn register(&mut self, node: impl Node + 'static) -> Result<(), NodeError> {
        self.register_boxed(Box::new(node))
    }
    pub fn register_boxed(&mut self, node: Box<dyn Node>) -> Result<(), NodeError> {
        let descriptor = node.descriptor();
        let id = descriptor.type_id;
        if self.descriptors.iter().any(|d| d.type_id == id) {
            return Err(NodeError::Duplicate(id.into()));
        }
        let schema = node.schema(&node.default_settings());
        let transport_valid = descriptor.capabilities.transport.is_none_or(|t| {
            descriptor.capabilities.media.is_some()
                && node.feedback().is_some()
                && t.inputs.iter().all(|&i| {
                    schema
                        .inputs
                        .get(i)
                        .is_some_and(|p| p.signal == crate::port::SignalType::Scalar)
                })
                && t.outputs.iter().all(|&i| {
                    schema
                        .outputs
                        .get(i)
                        .is_some_and(|p| p.signal == crate::port::SignalType::Scalar && p.delayed)
                })
        });
        if !transport_valid
            || !valid_capabilities(descriptor)
            || id.is_empty()
            || id.len() > 128
            || !id.is_ascii()
            || descriptor.parameters.iter().any(|p| {
                !p.min.is_finite()
                    || !p.max.is_finite()
                    || p.min >= p.max
                    || !p.default.is_finite()
                    || !(p.min..=p.max).contains(&p.default)
                    || (!p.choices.is_empty()
                        && (p.min.fract() != 0.0
                            || p.max.fract() != 0.0
                            || p.choices.len() != (p.max - p.min) as usize + 1
                            || p.choices.iter().any(|label| label.is_empty())))
                    || p.wrap.is_some_and(|period| {
                        !period.is_finite() || period <= 0.0 || period > p.max - p.min
                    })
            })
        {
            return Err(NodeError::Invalid(id.into()));
        }
        self.descriptors.push(descriptor);
        self.nodes.push(node);
        Ok(())
    }
}

fn valid_capabilities(d: &NodeDescriptor) -> bool {
    use crate::{capabilities::AudioRecipe, port::SignalType};
    let c = d.capabilities;
    let parameter = |index: usize| index < d.parameters.len();
    let audio_input = |index: usize| {
        d.inputs
            .get(index)
            .is_some_and(|p| p.signal == SignalType::Audio)
    };
    c.media.is_none_or(|m| {
        parameter(m.speed)
            && parameter(m.position)
            && [m.looping, m.start, m.end, m.seek]
                .into_iter()
                .all(|i| i.is_none_or(parameter))
    }) && c.program_output.is_none_or(|i| {
        d.inputs
            .get(i)
            .is_some_and(|p| p.signal == SignalType::Texture)
    }) && c.momentary_parameter.is_none_or(parameter)
        && c.asset_selection_parameter.is_none_or(parameter)
        && c.file_input.is_none_or(|p| {
            d.inputs
                .get(p)
                .is_some_and(|p| p.signal == SignalType::File)
        })
        && c.range_parameters
            .is_none_or(|[a, b]| parameter(a) && parameter(b))
        && c.value_control.is_none_or(|v| {
            parameter(v.parameter)
                && d.inputs
                    .get(v.feedback_input)
                    .is_some_and(|p| p.signal == SignalType::Scalar)
                && [v.request_output, v.display_output]
                    .into_iter()
                    .chain(v.request_trigger_output)
                    .all(|i| {
                        d.outputs
                            .get(i)
                            .is_some_and(|p| p.signal == SignalType::Scalar)
                    })
                && v.readout.is_none_or(|r| {
                    r.inputs.iter().all(|&i| {
                        d.inputs
                            .get(i)
                            .is_some_and(|p| p.signal == SignalType::Scalar)
                    }) && r.outputs.iter().all(|&i| {
                        d.outputs
                            .get(i)
                            .is_some_and(|p| p.signal == SignalType::Scalar)
                    })
                })
        })
        && c.rate.is_none_or(|r| {
            [r.mode, r.rate, r.range, r.clip_bpm, r.dj_bpm, r.trim]
                .into_iter()
                .all(parameter)
                && d.outputs
                    .get(r.speed_output)
                    .is_some_and(|p| p.signal == SignalType::Scalar)
        })
        && c.tempo_sync.is_none_or(|b| {
            let manual_valid = b.manual.is_none_or(|m| {
                [m.mode, m.rate, m.range, m.marker]
                    .into_iter()
                    .all(parameter)
            });
            manual_valid
                && [
                    b.clip_bpm,
                    b.dj_bpm,
                    b.trim,
                    b.nudge,
                    b.cue,
                    b.duration,
                    b.align,
                    b.nudge_step,
                ]
                .into_iter()
                .all(parameter)
                && b.outputs.into_iter().all(|p| {
                    d.outputs
                        .get(p)
                        .is_some_and(|p| p.signal == SignalType::Scalar)
                })
                && c.momentary_parameter == Some(b.align)
        })
        && (!c.audio_cue || matches!(c.audio, Some(AudioRecipe::Sink { .. })))
        && c.audio.is_none_or(|audio| match audio {
            AudioRecipe::Source => c.media.is_some(),
            AudioRecipe::Sink { input, gain } => audio_input(input) && parameter(gain),
            AudioRecipe::Mix(routes) => routes
                .iter()
                .all(|r| audio_input(r.input) && r.gain.is_none_or(parameter)),
        })
}
impl NodeFactory for NodeRegistry {
    fn get(&self, type_id: &str) -> Result<&dyn Node, NodeError> {
        self.descriptors
            .iter()
            .position(|d| d.type_id == type_id)
            .map(|index| self.nodes[index].as_ref())
            .ok_or_else(|| NodeError::Unknown(type_id.into()))
    }
    fn catalog(&self) -> &[&'static NodeDescriptor] {
        &self.descriptors
    }
}
