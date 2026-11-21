use super::*;
use vjay_domain::builtins::ids;
use vjay_domain::{builtins::registry as builtin_registry, graph::Connection};
#[test]
fn video_is_silent_until_audio_out_is_explicitly_connected_and_gain_is_modulated() {
    let f = builtin_registry();
    let mut g = Graph::default();
    let video = g.add(ids::VIDEO, &f).unwrap();
    g.bind_asset(video, AssetId(4), &f).unwrap();
    let gain = g.add(ids::AUDIO_GAIN, &f).unwrap();
    let output = g.add(ids::AUDIO_OUT, &f).unwrap();
    let value = g.add(ids::CONSTANT, &f).unwrap();
    g.set_parameter(value, 0, 0.25, &f).unwrap();
    let mut signals = SignalEngine::default();
    signals.rebuild(&g, &f).unwrap();
    assert!(AudioPlan::compile(&g, &signals).unwrap().routes.is_empty());
    for (from, port, to, input) in [
        (video, 1, gain, 0),
        (gain, 0, output, 0),
        (value, 0, gain, 1),
    ] {
        g.connect(Connection {
            from,
            output: port,
            to,
            input,
        })
        .unwrap();
    }
    signals.rebuild(&g, &f).unwrap();
    signals.tick(0.0);
    let mut timeline = MediaTimeline::default();
    timeline.rebuild(&g, &signals).unwrap();
    timeline.tick(0.0, &signals);
    timeline.tick(0.5, &signals);
    let mut plan = AudioPlan::compile(&g, &signals).unwrap();
    plan.update(&signals, &timeline);
    assert_eq!(plan.routes.len(), 1);
    assert!((plan.routes[0].gain - 0.2).abs() < 1e-6);
    assert_eq!(plan.routes[0].seconds, 0.5);
    g.disconnect(output, 0);
    assert!(AudioPlan::compile(&g, &signals).unwrap().routes.is_empty());
}
