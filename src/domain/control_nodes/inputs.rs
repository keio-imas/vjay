use crate::builtins::{SignalNode, ids, scalar};
use crate::{
    capabilities::*,
    node::{NodeDescriptor, Parameter as P},
    port::Port,
    registry::NodeRegistry,
};
static KEYBOARD: NodeDescriptor = NodeDescriptor {
    type_id: ids::KEYBOARD,
    category: "Input",
    capabilities: Capabilities {
        settings: &[SettingEditor::Keyboard],
        ..Capabilities::NONE
    },
    name: "Keyboard",
    inputs: &[],
    outputs: &[Port::scalar("Gate"), Port::scalar("Trigger")],
    parameters: &[],
};

static MIDI: NodeDescriptor = NodeDescriptor {
    type_id: ids::MIDI,
    category: "Input",
    capabilities: Capabilities {
        settings: &[SettingEditor::Midi],
        ..Capabilities::NONE
    },
    name: "MIDI in",
    inputs: &[],
    outputs: &[
        Port::scalar("CC"),
        Port::scalar("Note"),
        Port::scalar("Velocity"),
        Port::scalar("Gate"),
    ],
    parameters: &[
        P::new("Channel", 1.0, 16.0, 1.0),
        P::new("Controller", 0.0, 127.0, 1.0),
        P::new("Note", -1.0, 127.0, -1.0),
    ],
};

static OSC: NodeDescriptor = NodeDescriptor {
    type_id: ids::OSC,
    category: "Input",
    capabilities: Capabilities {
        settings: &[SettingEditor::Osc],
        ..Capabilities::NONE
    },
    name: "OSC in",
    inputs: &[],
    outputs: &[Port::scalar("Value"), Port::scalar("Trigger")],
    parameters: &[],
};

pub(super) fn register(registry: &mut NodeRegistry) {
    registry
        .register(SignalNode {
            descriptor: &KEYBOARD,
            evaluate: scalar::keyboard,
            cells: 0,
            tap: None,
        })
        .expect("valid built-in definition");
    registry
        .register(SignalNode {
            descriptor: &MIDI,
            evaluate: scalar::midi,
            cells: 0,
            tap: None,
        })
        .expect("valid built-in definition");
    registry
        .register(SignalNode {
            descriptor: &OSC,
            evaluate: scalar::osc,
            cells: 0,
            tap: None,
        })
        .expect("valid built-in definition");
}
