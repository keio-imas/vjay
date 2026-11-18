use crate::{
    builtins::{ImageNode, SignalNode},
    capabilities::Capabilities,
    execution::{ImageKernel, ScalarKernel},
    node::{Node, NodeDescriptor, Parameter as P},
    port::Port,
    registry::NodeRegistry,
    settings::NodeSettings,
};
pub(crate) struct NoiseNode(pub &'static NodeDescriptor);
fn hash(value: f64, seed: f64) -> f64 {
    let value = (value * 127.1 + seed * 311.7).sin() * 43758.5453;
    value - value.floor()
}
pub(crate) fn sample(phase: f64, seed: f64, smooth: bool) -> f32 {
    let cell = phase.floor();
    let t = phase - cell;
    let weight = if smooth { t * t * (3.0 - 2.0 * t) } else { 0.0 };
    (hash(cell, seed) * (1.0 - weight) + hash(cell + 1.0, seed) * weight) as f32
}
impl Node for NoiseNode {
    fn descriptor(&self) -> &'static NodeDescriptor {
        self.0
    }
    fn image(&self, values: &[f32], settings: &NodeSettings, output: usize) -> Option<ImageKernel> {
        ImageNode {
            descriptor: self.0,
            selector: 12.0,
        }
        .image(values, settings, output)
    }
    fn scalar(&self) -> Option<ScalarKernel> {
        Some(ScalarKernel {
            state: vec![0.0],
            tap: None,
            evaluate: |c, state, output| {
                state[0] += c.delta * c.parameters[0] as f64 * c.parameters[1] as f64;
                output[0] = sample(state[0], 0.0, true);
            },
        })
    }
}
pub(crate) fn register(registry: &mut NodeRegistry) {
    static D: NodeDescriptor = NodeDescriptor {
        type_id: "SignalNoise",
        name: "Signal noise",
        category: "Signals",
        inputs: &[],
        outputs: &[Port::scalar("Value")],
        capabilities: Capabilities::NONE,
        parameters: &[
            P::new("Frequency", 0.0, 100.0, 1.0),
            P::new("Minimum", -1e6, 1e6, 0.0),
            P::new("Maximum", -1e6, 1e6, 1.0),
            P::new("Seed", 0.0, 65535.0, 0.0),
            P::new("Interpolation", 0.0, 1.0, 1.0).choices(&["Sample and hold", "Smooth"]),
        ],
    };
    registry
        .register(SignalNode {
            descriptor: &D,
            cells: 1,
            tap: None,
            evaluate: |c, state, out| {
                state[0] += c.delta * c.parameters[0] as f64;
                let v = sample(state[0], c.parameters[3] as f64, c.parameters[4] >= 0.5);
                out[0] = c.parameters[1] + v * (c.parameters[2] - c.parameters[1]);
            },
        })
        .expect("valid signal noise");
}
