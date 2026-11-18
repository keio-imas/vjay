use super::{ImageNode, RoutingNode, ids};
use crate::capabilities::*;
use crate::{
    node::{NodeDescriptor, Parameter},
    port::Port,
    registry::NodeRegistry,
};

static PATTERN: NodeDescriptor = NodeDescriptor {
    type_id: ids::PATTERN,
    category: "Sources",
    capabilities: Capabilities::NONE,
    name: "Pattern",
    inputs: &[],
    outputs: &[Port::texture("Image")],
    parameters: &[
        Parameter {
            name: "Frequency",
            min: 1.0,
            max: 40.0,
            default: 8.0,
            wrap: None,
            choices: &[],
        },
        Parameter {
            name: "Speed",
            min: -4.0,
            max: 4.0,
            default: 0.4,
            wrap: None,
            choices: &[],
        },
        Parameter {
            name: "Angle",
            min: 0.0,
            max: 360.0,
            default: 35.0,
            wrap: None,
            choices: &[],
        },
    ],
};

static COLORIZE: NodeDescriptor = NodeDescriptor {
    type_id: ids::COLORIZE,
    category: "Color",
    capabilities: Capabilities::NONE,
    name: "Colorize",
    inputs: &[Port::texture("Image")],
    outputs: &[Port::texture("Image")],
    parameters: &[
        Parameter {
            name: "Hue",
            min: 0.0,
            max: 1.0,
            default: 0.55,
            wrap: None,
            choices: &[],
        },
        Parameter {
            name: "Saturation",
            min: 0.0,
            max: 1.0,
            default: 0.85,
            wrap: None,
            choices: &[],
        },
        Parameter {
            name: "Gain",
            min: 0.0,
            max: 2.0,
            default: 1.0,
            wrap: None,
            choices: &[],
        },
    ],
};

static MIX: NodeDescriptor = NodeDescriptor {
    type_id: ids::MIX,
    category: "Compositing",
    capabilities: Capabilities::NONE,
    name: "Mix",
    inputs: &[Port::texture("A"), Port::texture("B")],
    outputs: &[Port::texture("Image")],
    parameters: &[Parameter {
        name: "Amount",
        min: 0.0,
        max: 1.0,
        default: 0.5,
        wrap: None,
        choices: &[],
    }],
};

static OUTPUT: NodeDescriptor = NodeDescriptor {
    type_id: ids::OUTPUT,
    category: "Compositing",
    capabilities: Capabilities {
        program_output: Some(0),
        ..Capabilities::NONE
    },
    name: "Output",
    inputs: &[Port::texture("Image")],
    outputs: &[],
    parameters: &[],
};

pub(crate) fn register(registry: &mut NodeRegistry) {
    registry
        .register(ImageNode {
            descriptor: &PATTERN,
            selector: 0.0,
        })
        .expect("valid built-in definition");
    registry
        .register(ImageNode {
            descriptor: &COLORIZE,
            selector: 1.0,
        })
        .expect("valid built-in definition");
    registry
        .register(ImageNode {
            descriptor: &MIX,
            selector: 2.0,
        })
        .expect("valid built-in definition");
    registry
        .register(RoutingNode(&OUTPUT))
        .expect("valid built-in definition");
}
