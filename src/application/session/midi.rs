use super::*;
use crate::control_io::{ControlEvent, MidiMessage, MidiPort};

#[derive(Default)]
pub struct MidiActivity {
    pub learning: Option<NodeId>,
    pub received: u64,
    pub message: String,
    pub source: Option<String>,
}
impl Session {
    pub fn midi_ports(&self) -> &[MidiPort] {
        self.control_io.midi_ports()
    }
    pub fn midi_activity(&self) -> &MidiActivity {
        &self.midi
    }
    pub fn refresh_midi_inputs(&mut self) -> PortResult<()> {
        let mut settings = self.input_settings.clone();
        settings.midi = true;
        if self.input_settings.midi {
            self.control_io.refresh_midi()?;
        }
        self.configure_inputs(settings)?;
        self.signals.clear_midi();
        Ok(())
    }
    pub(super) fn ensure_midi(&mut self) {
        if !self.input_settings.midi
            && let Err(error) = self.refresh_midi_inputs()
        {
            self.input_status = error;
        }
    }
    pub fn set_midi_source(&mut self, node: NodeId, source: Option<String>) -> PortResult<()> {
        let record = self.project.graph.node(node).map_err(|e| e.to_string())?;
        if !self
            .descriptor(&record.type_id)
            .capabilities
            .settings
            .contains(&vjay_domain::capabilities::SettingEditor::Midi)
        {
            return Err("Choose a MIDI in node".into());
        }
        let mut settings = record.settings.clone();
        settings.midi_source = source;
        self.set_settings(node, settings)
    }
    pub fn learn_midi(&mut self, node: NodeId) -> PortResult<()> {
        let record = self.project.graph.node(node).map_err(|e| e.to_string())?;
        if !self
            .descriptor(&record.type_id)
            .capabilities
            .settings
            .contains(&vjay_domain::capabilities::SettingEditor::Midi)
        {
            return Err("Choose a MIDI input node".into());
        }
        if !self.input_settings.midi {
            self.refresh_midi_inputs()?;
        }
        self.midi.learning = Some(node);
        Ok(())
    }
    pub fn cancel_midi_learn(&mut self) {
        self.midi.learning = None;
    }
    pub(super) fn process_midi(&mut self) {
        let count = self
            .signals
            .events
            .iter()
            .filter(|e| e.midi().is_some())
            .count();
        if count == 0 {
            return;
        }
        self.midi.received = self.midi.received.saturating_add(count as u64);
        if let Some((source, channel, message)) = self
            .signals
            .events
            .iter()
            .rev()
            .find_map(ControlEvent::midi)
        {
            self.midi.source = source.map(str::to_owned);
            self.midi.message = match message {
                MidiMessage::Cc { controller, value } => format!(
                    "Channel {} · CC {controller} = {value}",
                    channel.min(15) + 1
                ),
                MidiMessage::Note { note, velocity } => format!(
                    "Channel {} · Note {note} · Velocity {velocity}",
                    channel.min(15) + 1
                ),
            };
        }
        let Some(node) = self.midi.learning else {
            return;
        };
        let Ok(record) = self.project.graph.node(node) else {
            self.midi.learning = None;
            return;
        };
        let source = record.settings.midi_source.as_deref();
        let candidate = self
            .signals
            .events
            .iter()
            .filter_map(ControlEvent::midi)
            .find(|(id, _, message)| {
                source.is_none_or(|source| Some(source) == *id)
                    && !matches!(message, MidiMessage::Note { velocity: 0, .. })
            });
        let Some((source, channel, message)) = candidate else {
            return;
        };
        let source = source.map(str::to_owned);
        let result = (|| -> PortResult<()> {
            self.set_parameter(node, 0, (channel.min(15) + 1) as f32)?;
            match message {
                MidiMessage::Cc { controller, .. } => {
                    self.set_parameter(node, 1, controller.min(127) as f32)?
                }
                MidiMessage::Note { note, .. } => {
                    self.set_parameter(node, 2, note.min(127) as f32)?
                }
            }
            self.set_midi_source(node, source)
        })();
        if let Err(error) = result {
            self.input_status = error;
        }
        self.midi.learning = None;
    }
}
