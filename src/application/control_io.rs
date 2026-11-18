use crate::ports::PortResult;
use vjay_domain::graph::NodeId;

#[derive(Clone, Debug)]
pub enum ControlEvent {
    NodeFile {
        node: vjay_domain::graph::NodeId,
        values: Vec<Option<vjay_domain::execution::FileValue>>,
    },
    NodeValues {
        node: NodeId,
        values: [f32; 16],
    },
    NodeText {
        node: NodeId,
        values: Vec<std::sync::Arc<str>>,
    },
    External {
        channel: String,
        values: Vec<f32>,
    },
    Key {
        key: String,
        down: bool,
    },
    Midi {
        channel: u8,
        message: MidiMessage,
    },
    MidiFrom {
        source: String,
        channel: u8,
        message: MidiMessage,
    },
    Osc {
        address: String,
        value: f32,
    },
    Tap {
        node: Option<NodeId>,
        seconds: f64,
    },
    Trigger(NodeId),
}
#[derive(Clone, Copy, Debug)]
pub enum MidiMessage {
    Cc { controller: u8, value: u8 },
    Note { note: u8, velocity: u8 },
}
impl ControlEvent {
    pub fn midi(&self) -> Option<(Option<&str>, u8, MidiMessage)> {
        match self {
            Self::Midi { channel, message } => Some((None, *channel, *message)),
            Self::MidiFrom {
                source,
                channel,
                message,
            } => Some((Some(source.as_str()), *channel, *message)),
            _ => None,
        }
    }
}
#[derive(Clone, Debug)]
pub struct MidiPort {
    pub id: String,
    pub name: String,
    pub error: Option<String>,
    pub virtual_input: bool,
}
#[derive(Clone, Debug)]
pub struct InputSettings {
    pub midi: bool,
    pub osc_port: Option<u16>,
    pub osc_host: String,
}

impl Default for InputSettings {
    fn default() -> Self {
        Self {
            midi: false,
            osc_port: None,
            osc_host: "0.0.0.0".into(),
        }
    }
}

pub trait ControlIo {
    fn prepare_services(
        &mut self,
        _requests: &[crate::services::ServiceRequest],
    ) -> PortResult<()> {
        Ok(())
    }
    fn update_services(&mut self, _controls: &[crate::services::ServiceControls]) {}
    fn configure(&mut self, settings: &InputSettings) -> PortResult<String>;
    fn poll(&mut self, events: &mut Vec<ControlEvent>) -> PortResult<()>;
    fn midi_ports(&self) -> &[MidiPort] {
        &[]
    }
    fn refresh_midi(&mut self) -> PortResult<()> {
        Ok(())
    }
}
pub struct DisabledControlIo;
impl ControlIo for DisabledControlIo {
    fn configure(&mut self, _: &InputSettings) -> PortResult<String> {
        Err("Control input adapter unavailable".into())
    }
    fn poll(&mut self, _: &mut Vec<ControlEvent>) -> PortResult<()> {
        Ok(())
    }
}
