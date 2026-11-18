use crate::{
    builtins::SignalNode,
    capabilities::Capabilities,
    node::{NodeDescriptor, Parameter as P},
    port::Port,
    registry::NodeRegistry,
};
pub(crate) fn register(registry: &mut NodeRegistry) {
    static D: NodeDescriptor = NodeDescriptor {
        type_id: "LipSync",
        name: "BPM Sync",
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
        ],
    };
    registry
        .register(SignalNode {
            descriptor: &D,
            cells: 2,
            tap: None,
            evaluate: |c, state, out| {
                let p = c.parameters;
                let beat = c.inputs[0].unwrap_or(0.0);
                let sync = c.inputs[1].unwrap_or(0.0) > 0.5 || p[8] > 0.5 || c.host.trigger(c.node);
                let rising = sync && state[1] <= 0.5;
                if rising {
                    state[0] = beat.rem_euclid(p[7]) as f64 * 60.0 / p[0] as f64 / p[5] as f64;
                }
                state[1] = f64::from(sync);
                let nudge = p[3] + (c.inputs[3].unwrap_or(0.0) - c.inputs[2].unwrap_or(0.0)) * p[9];
                out[0] = p[1] / p[0] * (1.0 + p[2] * 0.01) * (1.0 + nudge * 0.01);
                out[1] = (p[4] as f64 + state[0] + p[6] as f64 * 60.0 / p[0] as f64 / p[5] as f64)
                    .rem_euclid(1.0) as f32;
                out[2] = f32::from(rising);
                out[3] = beat.rem_euclid(1.0);
            },
        })
        .expect("valid lip sync control");
}
