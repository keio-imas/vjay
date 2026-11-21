use crate::builtins::{SignalNode, ids, scalar};
use crate::{
    capabilities::*,
    node::{NodeDescriptor, Parameter as P},
    port::Port,
    registry::NodeRegistry,
};
static ADD: NodeDescriptor = NodeDescriptor {
    type_id: ids::ADD,
    category: "Math",
    capabilities: Capabilities::NONE,
    name: "Add",
    inputs: &[],
    outputs: &[Port::scalar("Value")],
    parameters: &[
        P::new("A", -1000000.0, 1000000.0, 0.0),
        P::new("B", -1000000.0, 1000000.0, 0.0),
    ],
};

static MULTIPLY: NodeDescriptor = NodeDescriptor {
    type_id: ids::MULTIPLY,
    category: "Math",
    capabilities: Capabilities::NONE,
    name: "Multiply",
    inputs: &[],
    outputs: &[Port::scalar("Value")],
    parameters: &[
        P::new("A", -1000000.0, 1000000.0, 1.0),
        P::new("B", -1000000.0, 1000000.0, 1.0),
    ],
};

static MAPRANGE: NodeDescriptor = NodeDescriptor {
    type_id: ids::MAP_RANGE,
    category: "Math",
    capabilities: Capabilities::NONE,
    name: "Map range",
    inputs: &[],
    outputs: &[Port::scalar("Value")],
    parameters: &[
        P::new("Value", -1000000.0, 1000000.0, 0.0),
        P::new("Input min", -1000000.0, 1000000.0, 0.0),
        P::new("Input max", -1000000.0, 1000000.0, 1.0),
        P::new("Output min", -1000000.0, 1000000.0, 0.0),
        P::new("Output max", -1000000.0, 1000000.0, 1.0),
    ],
};

static CLAMP: NodeDescriptor = NodeDescriptor {
    type_id: ids::CLAMP,
    category: "Math",
    capabilities: Capabilities::NONE,
    name: "Clamp",
    inputs: &[],
    outputs: &[Port::scalar("Value")],
    parameters: &[
        P::new("Value", -1000000.0, 1000000.0, 0.0),
        P::new("Minimum", -1000000.0, 1000000.0, 0.0),
        P::new("Maximum", -1000000.0, 1000000.0, 1.0),
    ],
};

static SMOOTH: NodeDescriptor = NodeDescriptor {
    type_id: ids::SMOOTH,
    category: "Math",
    capabilities: Capabilities::NONE,
    name: "Smooth",
    inputs: &[],
    outputs: &[Port::scalar("Value")],
    parameters: &[
        P::new("Value", -1000000.0, 1000000.0, 0.0),
        P::new("Seconds", 0.0, 10.0, 0.2),
    ],
};

static COUNTER: NodeDescriptor = NodeDescriptor {
    type_id: ids::COUNTER,
    category: "Math",
    capabilities: Capabilities::NONE,
    name: "Counter",
    inputs: &[Port::scalar("Trigger").optional()],
    outputs: &[Port::scalar("Count"), Port::scalar("Pulse")],
    parameters: &[P::new("Reset", 0.0, 1.0, 0.0)],
};

pub(super) fn register(registry: &mut NodeRegistry) {
    registry
        .register(SignalNode {
            descriptor: &ADD,
            evaluate: scalar::add,
            cells: 0,
            tap: None,
        })
        .expect("valid built-in definition");
    registry
        .register(SignalNode {
            descriptor: &MULTIPLY,
            evaluate: scalar::multiply,
            cells: 0,
            tap: None,
        })
        .expect("valid built-in definition");
    registry
        .register(SignalNode {
            descriptor: &MAPRANGE,
            evaluate: scalar::map_range,
            cells: 0,
            tap: None,
        })
        .expect("valid built-in definition");
    registry
        .register(SignalNode {
            descriptor: &CLAMP,
            evaluate: scalar::clamp,
            cells: 0,
            tap: None,
        })
        .expect("valid built-in definition");
    registry
        .register(SignalNode {
            descriptor: &SMOOTH,
            evaluate: scalar::smooth,
            cells: 1,
            tap: None,
        })
        .expect("valid built-in definition");
    registry
        .register(SignalNode {
            descriptor: &COUNTER,
            evaluate: scalar::counter,
            cells: 2,
            tap: None,
        })
        .expect("valid built-in definition");
}
