use crate::builtins::{FragmentNode, ImageNode, RoutingNode, ids};
use crate::capabilities::*;
use crate::{
    node::{NodeDescriptor, Parameter as P},
    port::Port,
    registry::NodeRegistry,
};

static NOISE: NodeDescriptor = NodeDescriptor {
    type_id: ids::NOISE,
    category: "Sources",
    capabilities: Capabilities::NONE,
    name: "Noise",
    inputs: &[],
    outputs: &[Port::texture("Image"), Port::scalar("Value")],
    parameters: &[
        P::new("Scale", 1.0, 100.0, 12.0),
        P::new("Speed", 0.0, 10.0, 1.0),
    ],
};

static CHECKERBOARD: NodeDescriptor = NodeDescriptor {
    type_id: ids::CHECKERBOARD,
    category: "Sources",
    capabilities: Capabilities::NONE,
    name: "Checkerboard",
    inputs: &[],
    outputs: &[Port::texture("Image")],
    parameters: &[P::new("Scale", 1.0, 100.0, 12.0)],
};

static RAMP: NodeDescriptor = NodeDescriptor {
    type_id: ids::RAMP,
    category: "Sources",
    capabilities: Capabilities::NONE,
    name: "Ramp",
    inputs: &[],
    outputs: &[Port::texture("Image")],
    parameters: &[P::new("Angle", -180.0, 180.0, 0.0)],
};

static INVERT: NodeDescriptor = NodeDescriptor {
    type_id: ids::INVERT,
    category: "Color",
    capabilities: Capabilities::NONE,
    name: "Invert",
    inputs: &[Port::texture("Image")],
    outputs: &[Port::texture("Image")],
    parameters: &[P::new("Amount", 0.0, 1.0, 1.0)],
};

static POSTERIZE: NodeDescriptor = NodeDescriptor {
    type_id: ids::POSTERIZE,
    category: "Color",
    capabilities: Capabilities::NONE,
    name: "Posterize",
    inputs: &[Port::texture("Image")],
    outputs: &[Port::texture("Image")],
    parameters: &[P::new("Levels", 2.0, 256.0, 8.0)],
};

static PIXELATE: NodeDescriptor = NodeDescriptor {
    type_id: ids::PIXELATE,
    category: "Geometry",
    capabilities: Capabilities::NONE,
    name: "Pixelate",
    inputs: &[Port::texture("Image")],
    outputs: &[Port::texture("Image")],
    parameters: &[P::new("Size", 1.0, 128.0, 8.0)],
};

static EDGE: NodeDescriptor = NodeDescriptor {
    type_id: ids::EDGE,
    category: "Geometry",
    capabilities: Capabilities::NONE,
    name: "Edge detect",
    inputs: &[Port::texture("Image")],
    outputs: &[Port::texture("Image")],
    parameters: &[P::new("Amount", 0.0, 4.0, 1.0)],
};

static RGBSPLIT: NodeDescriptor = NodeDescriptor {
    type_id: ids::RGB_SPLIT,
    category: "Geometry",
    capabilities: Capabilities::NONE,
    name: "Split RGBA",
    inputs: &[Port::texture("Image")],
    outputs: &[
        Port::texture("Red"),
        Port::texture("Green"),
        Port::texture("Blue"),
        Port::texture("Alpha"),
    ],
    parameters: &[],
};

static GLSL: NodeDescriptor = NodeDescriptor {
    type_id: ids::GLSL,
    category: "Effects",
    capabilities: Capabilities {
        settings: &[SettingEditor::Fragment],
        ..Capabilities::NONE
    },
    name: "GLSL fragment",
    inputs: &[
        Port::texture("Image A").optional(),
        Port::texture("Image B").optional(),
    ],
    outputs: &[Port::texture("Image")],
    parameters: &[
        P::new("parameter_1", -1000.0, 1000.0, 0.0),
        P::new("parameter_2", -1000.0, 1000.0, 0.0),
        P::new("parameter_3", -1000.0, 1000.0, 0.0),
        P::new("parameter_4", -1000.0, 1000.0, 0.0),
    ],
};

static AUDIOGAIN: NodeDescriptor = NodeDescriptor {
    type_id: ids::AUDIO_GAIN,
    category: "Audio",
    capabilities: Capabilities {
        audio: Some(AudioRecipe::Mix(&[AudioInput {
            input: 0,
            gain: Some(0),
        }])),
        ..Capabilities::NONE
    },
    name: "Audio gain",
    inputs: &[Port::audio("Audio")],
    outputs: &[Port::audio("Audio")],
    parameters: &[P::new("Gain", 0.0, 2.0, 1.0)],
};

static AUDIOMIX: NodeDescriptor = NodeDescriptor {
    type_id: ids::AUDIO_MIX,
    category: "Audio",
    capabilities: Capabilities {
        audio: Some(AudioRecipe::Mix(&[
            AudioInput {
                input: 0,
                gain: Some(0),
            },
            AudioInput {
                input: 1,
                gain: Some(1),
            },
        ])),
        ..Capabilities::NONE
    },
    name: "Audio mixer",
    inputs: &[Port::audio("A"), Port::audio("B")],
    outputs: &[Port::audio("Audio")],
    parameters: &[
        P::new("A level", 0.0, 2.0, 1.0),
        P::new("B level", 0.0, 2.0, 1.0),
    ],
};

static AUDIOOUT: NodeDescriptor = NodeDescriptor {
    type_id: ids::AUDIO_OUT,
    category: "Audio",
    capabilities: Capabilities {
        audio: Some(AudioRecipe::Sink { input: 0, gain: 0 }),
        ..Capabilities::NONE
    },
    name: "Audio Out",
    inputs: &[Port::audio("Audio")],
    outputs: &[],
    parameters: &[P::new("Volume", 0.0, 1.0, 0.8)],
};

pub(crate) fn register(registry: &mut NodeRegistry) {
    registry
        .register(crate::nodes::noise::NoiseNode(&NOISE))
        .expect("valid built-in definition");
    registry
        .register(ImageNode {
            descriptor: &CHECKERBOARD,
            selector: 13.0,
        })
        .expect("valid built-in definition");
    registry
        .register(ImageNode {
            descriptor: &RAMP,
            selector: 14.0,
        })
        .expect("valid built-in definition");
    registry
        .register(ImageNode {
            descriptor: &INVERT,
            selector: 15.0,
        })
        .expect("valid built-in definition");
    registry
        .register(ImageNode {
            descriptor: &POSTERIZE,
            selector: 16.0,
        })
        .expect("valid built-in definition");
    registry
        .register(ImageNode {
            descriptor: &PIXELATE,
            selector: 17.0,
        })
        .expect("valid built-in definition");
    registry
        .register(ImageNode {
            descriptor: &EDGE,
            selector: 18.0,
        })
        .expect("valid built-in definition");
    registry
        .register(ImageNode {
            descriptor: &RGBSPLIT,
            selector: 19.0,
        })
        .expect("valid built-in definition");
    registry
        .register(FragmentNode(&GLSL))
        .expect("valid built-in definition");
    registry
        .register(RoutingNode(&AUDIOGAIN))
        .expect("valid built-in definition");
    registry
        .register(RoutingNode(&AUDIOMIX))
        .expect("valid built-in definition");
    registry
        .register(RoutingNode(&AUDIOOUT))
        .expect("valid built-in definition");
}
