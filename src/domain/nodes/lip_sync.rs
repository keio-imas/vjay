use crate::{
    capabilities::Capabilities,
    execution::ScalarKernel,
    node::Node,
    node::{NodeDescriptor, Parameter as P},
    port::Port,
    registry::NodeRegistry,
    settings::NodeSettings,
};
static D: NodeDescriptor = NodeDescriptor {
    type_id: "LipSync",
    name: "Deck Sync",
    category: "Timing",
    inputs: &[
        Port::scalar("Target beat").optional(),
        Port::scalar("Sync trigger").optional(),
        Port::scalar("Nudge backward").optional(),
        Port::scalar("Nudge forward").optional(),
    ],
    outputs: &[
        Port::scalar("Speed"),
        Port::scalar("Position"),
        Port::scalar("Seek"),
        Port::scalar("Phase"),
    ],
    capabilities: Capabilities {
        momentary_parameter: Some(8),
        tempo_sync: Some(crate::capabilities::TempoSyncBinding {
            manual: Some(crate::capabilities::ManualSyncBinding {
                mode: 10,
                rate: 11,
                range: 12,
                marker: 13,
            }),
            clip_bpm: 0,
            dj_bpm: 1,
            trim: 2,
            nudge: 3,
            cue: 4,
            duration: 5,
            align: 8,
            nudge_step: 9,
            outputs: [0, 1, 2],
        }),
        ..Capabilities::NONE
    },
    parameters: &[
        P::new("Clip BPM", 20.0, 999.0, 120.0),
        P::new("DJ BPM", 20.0, 999.0, 120.0),
        P::new("Fine rate %", -20.0, 20.0, 0.0),
        P::new("Nudge %", -25.0, 25.0, 0.0),
        P::new("Cue position", 0.0, 1.0, 0.0),
        P::new("Clip seconds", 0.01, 7200.0, 10.0),
        P::new("Beat offset", -32.0, 32.0, 0.0),
        P::new("Beats per phrase", 1.0, 256.0, 4.0),
        P::new("Align / Sync", 0.0, 1.0, 0.0).choices(crate::choices::SWITCH),
        P::new("Nudge step %", 0.0, 20.0, 2.0),
        P::new("Sync mode", 0.0, 1.0, 0.0).choices(&["By ear", "BPM ratio"]),
        P::new("Playback rate %", 0.0, 400.0, 100.0),
        P::new("Pitch range %", 4.0, 100.0, 16.0),
        P::new("Cue marker", 0.0, 1.0, 0.0),
    ],
};
struct Deck;
impl Node for Deck {
    fn descriptor(&self) -> &'static NodeDescriptor {
        &D
    }
    fn migrate_parameters(&self, values: &mut Vec<f32>) {
        if values.len() == 10 {
            let cue = values[4];
            values.extend([1.0, 100.0, 16.0, cue]);
        }
    }
    fn scalar(&self) -> Option<ScalarKernel> {
        Some(ScalarKernel {
            state: vec![0.0; 2],
            tap: None,
            evaluate: |c, state, out| {
                let p = c.parameters;
                let beat = c.inputs[0].unwrap_or(0.0);
                let sync = c.inputs[1].unwrap_or(0.0) > 0.5 || p[8] > 0.5 || c.host.trigger(c.node);
                let rising = sync && state[1] <= 0.5;
                let manual = p[10] < 0.5;
                if manual {
                    state[0] = 0.0;
                } else if rising {
                    state[0] = beat.rem_euclid(p[7]) as f64 * 60.0 / p[0] as f64 / p[5] as f64;
                }
                state[1] = f64::from(sync);
                let nudge = p[3] + (c.inputs[3].unwrap_or(0.0) - c.inputs[2].unwrap_or(0.0)) * p[9];
                let rate = if manual { p[11] * 0.01 } else { p[1] / p[0] };
                out[0] =
                    rate * (if manual { 1.0 } else { 1.0 + p[2] * 0.01 }) * (1.0 + nudge * 0.01);
                let offset = if manual {
                    0.0
                } else {
                    p[6] as f64 * 60.0 / p[0] as f64 / p[5] as f64
                };
                out[1] = (p[4] as f64 + state[0] + offset).rem_euclid(1.0) as f32;
                out[2] = f32::from(rising);
                out[3] = beat.rem_euclid(1.0);
            },
        })
    }
    fn validate_settings(&self, s: &NodeSettings) -> bool {
        s.is_valid()
    }
}
pub(crate) fn register(registry: &mut NodeRegistry) {
    registry.register(Deck).expect("valid deck sync");
}
