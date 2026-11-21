pub mod tempo;
use crate::{
    execution::{ScalarKernel, SignalInputs},
    graph::NodeId,
    node::NodeDescriptor,
    port::SignalType,
    schema::{NodeSchema, PortInfo},
};
#[derive(Clone, Copy, Debug)]
pub struct TransportBinding {
    pub inputs: [usize; 8],
    pub outputs: [usize; 6],
}
impl TransportBinding {
    pub const fn at(offset: usize) -> Self {
        Self {
            outputs: [2, 3, 4, 5, 6, 7],
            inputs: [
                offset,
                offset + 1,
                offset + 2,
                offset + 3,
                offset + 4,
                offset + 5,
                offset + 6,
                offset + 7,
            ],
        }
    }
}
pub const INPUTS: [&str; 8] = [
    "Play",
    "Pause",
    "Toggle play",
    "Jog seconds",
    "Nudge backward",
    "Nudge forward",
    "Set cue",
    "Recall cue",
];
pub const OUTPUTS: [&str; 6] = [
    "Playhead",
    "Elapsed seconds",
    "Duration seconds",
    "Actual speed",
    "Playing",
    "Ready",
];
#[derive(Clone, Copy, Debug, Default)]
pub struct MediaState {
    pub position: f32,
    pub seconds: f64,
    pub duration: f64,
    pub speed: f32,
    pub playing: bool,
    pub ready: bool,
    pub cue: f32,
}
#[derive(Clone, Copy, Debug)]
pub enum TransportAction {
    Seek(f32),
    Jog(f64),
    Play,
    Pause,
    Toggle,
    SetCue,
    RecallCue,
}
impl TransportAction {
    pub fn valid(self) -> bool {
        match self {
            Self::Seek(v) => (0.0..=1.0).contains(&v),
            Self::Jog(v) => v.is_finite() && v.abs() <= 7200.0,
            _ => true,
        }
    }
}
pub fn schema(d: &'static NodeDescriptor) -> NodeSchema {
    let mut s = NodeSchema::new(d);
    s.inputs.extend(INPUTS.map(|name| PortInfo {
        name: name.into(),
        signal: SignalType::Scalar,
        optional: true,
        delayed: false,
    }));
    s.outputs.extend(OUTPUTS.map(|name| PortInfo {
        name: name.into(),
        signal: SignalType::Scalar,
        optional: false,
        delayed: true,
    }));
    s
}
pub fn feedback() -> ScalarKernel {
    ScalarKernel {
        state: Vec::new(),
        tap: None,
        evaluate: |c, _, o| {
            let s = c.host.media(c.node);
            o.copy_from_slice(&[
                s.position,
                s.seconds as f32,
                s.duration as f32,
                s.speed,
                f32::from(s.playing),
                f32::from(s.ready),
            ]);
        },
    }
}
pub fn state(host: &dyn SignalInputs, node: NodeId) -> MediaState {
    host.media(node)
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct Bookmark {
    pub asset: Option<crate::node::AssetId>,
    pub cue: f32,
}
