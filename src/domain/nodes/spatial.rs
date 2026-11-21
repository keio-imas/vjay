use crate::builtins::{ImageNode, ids};
use crate::capabilities::*;
use crate::{
    node::{NodeDescriptor, Parameter as P},
    port::Port,
    registry::NodeRegistry,
};

static TRANSFORM: NodeDescriptor = NodeDescriptor {
    type_id: ids::TRANSFORM,
    category: "Geometry",
    capabilities: Capabilities::NONE,
    name: "Transform",
    inputs: &[Port::texture("Image")],
    outputs: &[Port::texture("Image")],
    parameters: &[
        P::new("X", -1.0, 1.0, 0.0),
        P::new("Y", -1.0, 1.0, 0.0),
        P::new("Scale", 0.1, 4.0, 1.0),
        P::new("Rotation", -180.0, 180.0, 0.0).wrapping(360.0),
    ],
};

static KALEIDOSCOPE: NodeDescriptor = NodeDescriptor {
    type_id: ids::KALEIDOSCOPE,
    category: "Geometry",
    capabilities: Capabilities::NONE,
    name: "Kaleidoscope",
    inputs: &[Port::texture("Image")],
    outputs: &[Port::texture("Image")],
    parameters: &[
        P::new("Segments", 1.0, 16.0, 6.0),
        P::new("Rotation", -180.0, 180.0, 0.0).wrapping(360.0),
    ],
};

static BLUR: NodeDescriptor = NodeDescriptor {
    type_id: ids::BLUR,
    category: "Geometry",
    capabilities: Capabilities::NONE,
    name: "Blur",
    inputs: &[Port::texture("Image")],
    outputs: &[Port::texture("Image")],
    parameters: &[P::new("Radius", 0.0, 24.0, 3.0)],
};

pub(crate) fn register(registry: &mut NodeRegistry) {
    registry
        .register(ImageNode {
            descriptor: &TRANSFORM,
            selector: 5.0,
        })
        .expect("valid built-in definition");
    registry
        .register(ImageNode {
            descriptor: &KALEIDOSCOPE,
            selector: 7.0,
        })
        .expect("valid built-in definition");
    registry
        .register(ImageNode {
            descriptor: &BLUR,
            selector: 10.0,
        })
        .expect("valid built-in definition");
}
