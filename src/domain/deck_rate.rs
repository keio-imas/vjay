use crate::{
    builtins::SignalNode,
    capabilities::{Capabilities, RateBinding},
    node::{NodeDescriptor, Parameter as P},
    port::Port,
    registry::NodeRegistry,
};
static D: NodeDescriptor = NodeDescriptor {
    type_id: "DeckRate",
    name: "Deck Rate",
    category: "Timing",
    inputs: &[
        Port::scalar("Nudge backward").optional(),
        Port::scalar("Nudge forward").optional(),
    ],
    outputs: &[Port::scalar("Speed")],
    parameters: &[
        P::new("Sync mode", 0.0, 1.0, 0.0).choices(&["By ear", "BPM ratio"]),
        P::new("Playback rate %", 0.0, 400.0, 100.0),
        P::new("Pitch range %", 4.0, 100.0, 16.0),
        P::new("Clip BPM", 20.0, 999.0, 120.0),
        P::new("DJ BPM", 20.0, 999.0, 120.0),
        P::new("Fine rate %", -20.0, 20.0, 0.0),
        P::new("Nudge step %", 0.0, 20.0, 2.0),
    ],
    capabilities: Capabilities {
        rate: Some(RateBinding {
            mode: 0,
            rate: 1,
            range: 2,
            clip_bpm: 3,
            dj_bpm: 4,
            trim: 5,
            speed_output: 0,
        }),
        ..Capabilities::NONE
    },
};
pub(crate) fn register(r: &mut NodeRegistry) {
    r.register(SignalNode {
        descriptor: &D,
        cells: 0,
        tap: None,
        evaluate: |c, _, o| {
            let p = c.parameters;
            let rate = if p[0] < 0.5 {
                p[1] * 0.01
            } else {
                p[4] / p[3] * (1.0 + p[5] * 0.01)
            };
            let nudge = (c.inputs[1].unwrap_or(0.0) - c.inputs[0].unwrap_or(0.0)) * p[6] * 0.01;
            o[0] = rate * (1.0 + nudge).max(0.0);
        },
    })
    .expect("valid deck rate");
}
