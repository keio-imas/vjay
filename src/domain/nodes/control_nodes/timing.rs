use crate::builtins::{SignalNode, ids, scalar};
use crate::{
    capabilities::*,
    node::{NodeDescriptor, Parameter as P},
    port::Port,
    registry::NodeRegistry,
};
static LFO: NodeDescriptor = NodeDescriptor {
    type_id: ids::LFO,
    category: "Signals",
    capabilities: Capabilities::NONE,
    name: "LFO",
    inputs: &[Port::scalar("Phase").optional()],
    outputs: &[
        Port::scalar("Sine"),
        Port::scalar("Triangle"),
        Port::scalar("Saw"),
        Port::scalar("Square"),
    ],
    parameters: &[
        P::new("Frequency", -40.0, 40.0, 1.0),
        P::new("Amplitude", -1000.0, 1000.0, 1.0),
        P::new("Offset", -1000.0, 1000.0, 0.0),
        P::new("Phase", 0.0, 1.0, 0.0),
    ],
};

static TEMPO: NodeDescriptor = NodeDescriptor {
    type_id: ids::TEMPO,
    category: "Signals",
    capabilities: Capabilities {
        tap: true,
        ..Capabilities::NONE
    },
    name: "Tap tempo",
    inputs: &[Port::scalar("Tap").optional()],
    outputs: &[
        Port::scalar("Beat"),
        Port::scalar("Phase"),
        Port::scalar("BPM"),
    ],
    parameters: &[
        P::new("BPM", 20.0, 300.0, 120.0),
        P::new("Division", 1.0, 16.0, 1.0),
    ],
};

pub(super) fn register(registry: &mut NodeRegistry) {
    registry
        .register(SignalNode {
            descriptor: &LFO,
            evaluate: scalar::lfo,
            cells: 1,
            tap: None,
        })
        .expect("valid built-in definition");
    registry
        .register(SignalNode {
            descriptor: &TEMPO,
            evaluate: scalar::tempo,
            cells: 16,
            tap: Some(scalar::tap),
        })
        .expect("valid built-in definition");
}
