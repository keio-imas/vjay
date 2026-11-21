use crate::control_io::{ControlEvent, MidiMessage};
use std::collections::{HashMap, HashSet};
use vjay_domain::graph::NodeId;

#[derive(Default)]
pub(super) struct InputState {
    pub media: HashMap<NodeId, vjay_domain::transport::MediaState>,
    pub values: HashMap<NodeId, [f32; 16]>,
    pub files: HashMap<NodeId, Vec<Option<vjay_domain::execution::FileValue>>>,
    pub text: HashMap<NodeId, Vec<std::sync::Arc<str>>>,
    pub keys: HashSet<String>,
    pub pressed: HashSet<String>,
    pub triggers: HashSet<NodeId>,
    pub midi: MidiState,
    pub sources: HashMap<String, MidiState>,
    pub osc: HashMap<String, f32>,
    pub osc_changed: HashSet<String>,
    pub external: HashMap<String, Vec<f32>>,
}
impl InputState {
    pub fn midi_state(&self, source: Option<&str>) -> &MidiState {
        static SILENT: MidiState = MidiState {
            cc: [0; 2048],
            notes: [0; 2048],
            last_note: [0; 16],
        };
        source.map_or(&self.midi, |id| self.sources.get(id).unwrap_or(&SILENT))
    }
    pub fn begin_frame(&mut self) {
        self.pressed.clear();
        self.triggers.clear();
        self.osc_changed.clear();
    }
    pub fn apply(&mut self, event: ControlEvent) {
        match event {
            ControlEvent::NodeFile { node, values } => {
                if values.len() <= 64
                    && (self.files.len() < vjay_domain::graph::MAX_NODES
                        || self.files.contains_key(&node))
                {
                    self.files.insert(node, values);
                }
            }
            ControlEvent::NodeValues { node, values } => {
                if values.iter().all(|v| v.is_finite())
                    && (self.values.len() < vjay_domain::graph::MAX_NODES
                        || self.values.contains_key(&node))
                {
                    self.values.insert(node, values);
                }
            }
            ControlEvent::NodeText { node, values } => {
                if values.len() <= 64
                    && values.iter().all(|v| v.len() <= 16384)
                    && (self.text.len() < vjay_domain::graph::MAX_NODES
                        || self.text.contains_key(&node))
                {
                    self.text.insert(node, values);
                }
            }
            ControlEvent::Key { key, down } => {
                if down {
                    if self.keys.insert(key.clone()) {
                        self.pressed.insert(key);
                    }
                } else {
                    self.keys.remove(&key);
                }
            }
            ControlEvent::Trigger(node) => {
                self.triggers.insert(node);
            }
            ControlEvent::Midi { channel, message } => self.midi.apply(channel, message),
            ControlEvent::MidiFrom {
                source,
                channel,
                message,
            } => {
                self.midi.apply(channel, message);
                if source.len() <= 1024
                    && (self.sources.len() < 64 || self.sources.contains_key(&source))
                {
                    self.sources
                        .entry(source)
                        .or_default()
                        .apply(channel, message);
                }
            }
            ControlEvent::Osc { address, value } => {
                if value.is_finite()
                    && address.len() <= 256
                    && (self.osc.len() < 4096 || self.osc.contains_key(&address))
                {
                    self.osc_changed.insert(address.clone());
                    self.osc.insert(address, value);
                }
            }
            ControlEvent::Tap { .. } => (),
            ControlEvent::External { channel, values } => {
                if channel.len() <= 256
                    && values.len() <= 64
                    && values.iter().all(|v| v.is_finite())
                    && (self.external.len() < 4096 || self.external.contains_key(&channel))
                {
                    self.external.insert(channel, values);
                }
            }
        }
    }
}

impl vjay_domain::execution::SignalInputs for InputState {
    fn media(&self, node: NodeId) -> vjay_domain::transport::MediaState {
        self.media.get(&node).copied().unwrap_or_default()
    }
    fn node_file(&self, node: NodeId, output: usize) -> Option<vjay_domain::execution::FileValue> {
        self.files
            .get(&node)
            .and_then(|v| v.get(output))
            .cloned()
            .flatten()
    }
    fn node_value(&self, node: NodeId, output: usize) -> f32 {
        self.values
            .get(&node)
            .and_then(|v| v.get(output))
            .copied()
            .unwrap_or(0.0)
    }
    fn node_text(&self, node: NodeId, output: usize) -> std::sync::Arc<str> {
        self.text
            .get(&node)
            .and_then(|v| v.get(output))
            .cloned()
            .unwrap_or_else(vjay_domain::execution::empty_text)
    }
    fn key(&self, key: &str) -> [f32; 2] {
        [
            f32::from(self.keys.contains(key)),
            f32::from(self.pressed.contains(key)),
        ]
    }
    fn trigger(&self, node: NodeId) -> bool {
        self.triggers.contains(&node)
    }
    fn midi(
        &self,
        source: Option<&str>,
        channel: usize,
        controller: usize,
        note: Option<usize>,
    ) -> [f32; 4] {
        let midi = self.midi_state(source);
        let channel = channel.min(15);
        let note = note.unwrap_or(midi.last_note[channel] as usize).min(127);
        let velocity = midi.notes[channel * 128 + note];
        [
            midi.cc[channel * 128 + controller.min(127)] as f32 / 127.0,
            note as f32,
            velocity as f32 / 127.0,
            f32::from(velocity > 0),
        ]
    }
    fn osc(&self, address: &str) -> [f32; 2] {
        [
            self.osc.get(address).copied().unwrap_or(0.0),
            f32::from(self.osc_changed.contains(address)),
        ]
    }
    fn external(&self, channel: &str, output: usize) -> f32 {
        self.external
            .get(channel)
            .and_then(|v| v.get(output))
            .copied()
            .unwrap_or(0.0)
    }
}

#[derive(Clone)]
pub(super) struct MidiState {
    pub cc: [u8; 2048],
    pub notes: [u8; 2048],
    pub last_note: [u8; 16],
}
impl Default for MidiState {
    fn default() -> Self {
        Self {
            cc: [0; 2048],
            notes: [0; 2048],
            last_note: [0; 16],
        }
    }
}
impl MidiState {
    fn apply(&mut self, channel: u8, message: MidiMessage) {
        let channel = channel.min(15) as usize;
        match message {
            MidiMessage::Cc { controller, value } => {
                self.cc[channel * 128 + controller.min(127) as usize] = value.min(127)
            }
            MidiMessage::Note { note, velocity } => {
                self.last_note[channel] = note.min(127);
                self.notes[channel * 128 + note.min(127) as usize] = velocity.min(127);
            }
        }
    }
}
