use crate::builtins::{ImageNode, ids};
use crate::capabilities::*;
use crate::{
    node::{NodeDescriptor, Parameter as P},
    port::Port,
    registry::NodeRegistry,
};

static GRADE: NodeDescriptor = NodeDescriptor {
    type_id: ids::GRADE,
    category: "Color",
    capabilities: Capabilities::NONE,
    name: "Color adjustment",
    inputs: &[Port::texture("Image")],
    outputs: &[Port::texture("Image")],
    parameters: &[
        P::new("Brightness", -1.0, 1.0, 0.0),
        P::new("Contrast", 0.0, 2.0, 1.0),
        P::new("Saturation", 0.0, 2.0, 1.0),
        P::new("Hue", -180.0, 180.0, 0.0),
    ],
};

static KEY: NodeDescriptor = NodeDescriptor {
    type_id: ids::LUMA_KEY,
    category: "Effects",
    capabilities: Capabilities::NONE,
    name: "Luma key",
    inputs: &[Port::texture("Image")],
    outputs: &[Port::texture("Image")],
    parameters: &[
        P::new("Threshold", 0.0, 1.0, 0.2),
        P::new("Softness", 0.001, 1.0, 0.1),
    ],
};

static STROBE: NodeDescriptor = NodeDescriptor {
    type_id: ids::STROBE,
    category: "Effects",
    capabilities: Capabilities::NONE,
    name: "Strobe",
    inputs: &[Port::texture("Image")],
    outputs: &[Port::texture("Image")],
    parameters: &[
        P::new("Frequency (Hz)", 0.0, 20.0, 4.0),
        P::new("Duty", 0.0, 1.0, 0.5),
    ],
};

static BLEND: NodeDescriptor = NodeDescriptor {
    type_id: ids::BLEND,
    category: "Compositing",
    capabilities: Capabilities::NONE,
    name: "Blend",
    inputs: &[Port::texture("Base"), Port::texture("Layer")],
    outputs: &[Port::texture("Image")],
    parameters: &[
        P::new("Amount", 0.0, 1.0, 1.0),
        P::new("Mode", 0.0, 3.0, 0.0).choices(crate::choices::BLEND),
    ],
};

pub(crate) fn register(registry: &mut NodeRegistry) {
    registry
        .register(ImageNode {
            descriptor: &GRADE,
            selector: 6.0,
        })
        .expect("valid built-in definition");
    registry
        .register(ImageNode {
            descriptor: &KEY,
            selector: 8.0,
        })
        .expect("valid built-in definition");
    registry
        .register(ImageNode {
            descriptor: &STROBE,
            selector: 9.0,
        })
        .expect("valid built-in definition");
    registry
        .register(ImageNode {
            descriptor: &BLEND,
            selector: 11.0,
        })
        .expect("valid built-in definition");
}
