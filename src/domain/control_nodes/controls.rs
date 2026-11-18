use crate::builtins::{SignalNode, ids, scalar};
use crate::{
    capabilities::*,
    node::{NodeDescriptor, Parameter as P},
    port::Port,
    registry::NodeRegistry,
};
static CONSTANT: NodeDescriptor = NodeDescriptor {
    type_id: ids::CONSTANT,
    category: "Signals",
    capabilities: Capabilities {
        widget: Some(WidgetKind::Label),
        ..Capabilities::NONE
    },
    name: "Constant",
    inputs: &[],
    outputs: &[Port::scalar("Value")],
    parameters: &[P::new("Value", -1000000.0, 1000000.0, 0.0)],
};

static SLIDER: NodeDescriptor = NodeDescriptor {
    type_id: ids::SLIDER,
    category: "Controls",
    capabilities: Capabilities {
        widget: Some(WidgetKind::Slider),
        range_parameters: Some([1, 2]),
        ..Capabilities::NONE
    },
    name: "Slider",
    inputs: &[],
    outputs: &[Port::scalar("Value")],
    parameters: &[
        P::new("Value", -1000000.0, 1000000.0, 0.0),
        P::new("Minimum", -1000000.0, 1000000.0, 0.0),
        P::new("Maximum", -1000000.0, 1000000.0, 1.0),
    ],
};

static KNOB: NodeDescriptor = NodeDescriptor {
    type_id: ids::KNOB,
    category: "Controls",
    capabilities: Capabilities {
        widget: Some(WidgetKind::Knob),
        range_parameters: Some([1, 2]),
        ..Capabilities::NONE
    },
    name: "Knob",
    inputs: &[],
    outputs: &[Port::scalar("Value")],
    parameters: &[
        P::new("Value", -1000000.0, 1000000.0, 0.0),
        P::new("Minimum", -1000000.0, 1000000.0, 0.0),
        P::new("Maximum", -1000000.0, 1000000.0, 1.0),
    ],
};

static NUMBER: NodeDescriptor = NodeDescriptor {
    type_id: ids::NUMBER,
    category: "Controls",
    capabilities: Capabilities {
        widget: Some(WidgetKind::Number),
        range_parameters: Some([1, 2]),
        ..Capabilities::NONE
    },
    name: "Number",
    inputs: &[],
    outputs: &[Port::scalar("Value")],
    parameters: &[
        P::new("Value", -1000000.0, 1000000.0, 0.0),
        P::new("Minimum", -1000000.0, 1000000.0, 0.0),
        P::new("Maximum", -1000000.0, 1000000.0, 1.0),
    ],
};

static TOGGLE: NodeDescriptor = NodeDescriptor {
    type_id: ids::TOGGLE,
    category: "Controls",
    capabilities: Capabilities {
        widget: Some(WidgetKind::Toggle),
        ..Capabilities::NONE
    },
    name: "Toggle",
    inputs: &[],
    outputs: &[Port::scalar("Value")],
    parameters: &[P::new("Value", 0.0, 1.0, 0.0).choices(crate::choices::SWITCH)],
};

static BUTTON: NodeDescriptor = NodeDescriptor {
    type_id: ids::BUTTON,
    category: "Controls",
    capabilities: Capabilities {
        widget: Some(WidgetKind::Button),
        momentary_parameter: Some(0),
        ..Capabilities::NONE
    },
    name: "Button",
    inputs: &[],
    outputs: &[Port::scalar("Gate"), Port::scalar("Trigger")],
    parameters: &[P::new("Value", 0.0, 1.0, 0.0).choices(crate::choices::SWITCH)],
};

static XYPAD: NodeDescriptor = NodeDescriptor {
    type_id: ids::XY_PAD,
    category: "Controls",
    capabilities: Capabilities {
        widget: Some(WidgetKind::XYPad),
        ..Capabilities::NONE
    },
    name: "XY pad",
    inputs: &[],
    outputs: &[Port::scalar("X"), Port::scalar("Y")],
    parameters: &[P::new("X", 0.0, 1.0, 0.5), P::new("Y", 0.0, 1.0, 0.5)],
};

pub(super) fn register(registry: &mut NodeRegistry) {
    registry
        .register(SignalNode {
            descriptor: &CONSTANT,
            evaluate: scalar::copy,
            cells: 0,
            tap: None,
        })
        .expect("valid built-in definition");
    registry
        .register(SignalNode {
            descriptor: &SLIDER,
            evaluate: scalar::copy,
            cells: 0,
            tap: None,
        })
        .expect("valid built-in definition");
    registry
        .register(SignalNode {
            descriptor: &KNOB,
            evaluate: scalar::copy,
            cells: 0,
            tap: None,
        })
        .expect("valid built-in definition");
    registry
        .register(SignalNode {
            descriptor: &NUMBER,
            evaluate: scalar::copy,
            cells: 0,
            tap: None,
        })
        .expect("valid built-in definition");
    registry
        .register(SignalNode {
            descriptor: &TOGGLE,
            evaluate: scalar::copy,
            cells: 0,
            tap: None,
        })
        .expect("valid built-in definition");
    registry
        .register(SignalNode {
            descriptor: &BUTTON,
            evaluate: scalar::button,
            cells: 1,
            tap: None,
        })
        .expect("valid built-in definition");
    registry
        .register(SignalNode {
            descriptor: &XYPAD,
            evaluate: scalar::xy,
            cells: 0,
            tap: None,
        })
        .expect("valid built-in definition");
}
