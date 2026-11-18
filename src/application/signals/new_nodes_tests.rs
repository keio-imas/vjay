use super::*;
use vjay_domain::{
    builtins::{ids, registry},
    graph::Connection,
    node::AssetId,
};
fn wire(g: &mut Graph, from: NodeId, output: usize, to: NodeId, input: usize) {
    g.connect(Connection {
        from,
        output,
        to,
        input,
    })
    .unwrap();
}
#[test]
fn texture_noise_also_emits_a_numeric_signal_and_signal_noise_has_a_zero_rate() {
    let f = registry();
    let mut g = Graph::default();
    let noise = g.add(ids::NOISE, &f).unwrap();
    let signal = g.add("SignalNoise", &f).unwrap();
    for (p, v) in [(0, 0.0), (1, -1.0), (2, 1.0), (3, 42.0)] {
        g.set_parameter(signal, p, v, &f).unwrap();
    }
    let mut s = SignalEngine::default();
    s.rebuild(&g, &f).unwrap();
    s.tick(0.0);
    assert!(s.output(noise, 0).is_none());
    let start = s.output(noise, 1).unwrap();
    let held = s.output(signal, 0).unwrap();
    for i in 1..100 {
        s.tick(i as f64 * 0.01);
        let v = s.output(noise, 1).unwrap();
        assert!((0.0..=1.0).contains(&v));
        assert_eq!(s.output(signal, 0), Some(held));
    }
    assert_ne!(s.output(noise, 1), Some(start));
    assert!((-1.0..=1.0).contains(&held));
}
#[test]
fn lip_sync_sets_tempo_nudge_and_phase_and_seeks_again_at_an_unchanged_position() {
    let f = registry();
    let mut g = Graph::default();
    let sync = g.add("LipSync", &f).unwrap();
    let beat = g.add(ids::CONSTANT, &f).unwrap();
    let button = g.add(ids::BUTTON, &f).unwrap();
    let forward = g.add(ids::BUTTON, &f).unwrap();
    let video = g.add(ids::VIDEO, &f).unwrap();
    g.bind_asset(video, AssetId(0), &f).unwrap();
    for (p, v) in [(1, 150.0), (2, 2.0), (4, 0.2), (5, 10.0), (6, 1.0)] {
        g.set_parameter(sync, p, v, &f).unwrap();
    }
    g.set_parameter(beat, 0, 8.25, &f).unwrap();
    wire(&mut g, beat, 0, sync, 0);
    wire(&mut g, button, 1, sync, 1);
    wire(&mut g, forward, 0, sync, 3);
    for (output, input) in [(0, 0), (1, 1), (2, 5)] {
        wire(&mut g, sync, output, video, input);
    }
    let mut s = SignalEngine::default();
    s.rebuild(&g, &f).unwrap();
    s.tick(0.0);
    let mut timeline = crate::media_timeline::MediaTimeline::default();
    timeline.rebuild(&g, &s).unwrap();
    timeline.tick(0.0, &s);
    assert!((s.output(sync, 0).unwrap() - 1.275).abs() < 1e-5);
    for time in [0.1, 0.3] {
        s.events.push(ControlEvent::Trigger(button));
        s.tick(time);
        timeline.tick(time, &s);
        assert!((s.output(sync, 1).unwrap() - 0.2625).abs() < 1e-5);
        assert_eq!(s.output(sync, 2), Some(1.0));
        assert_eq!(timeline.base(video), 0.0);
        s.tick(time + 0.1);
        timeline.tick(time + 0.1, &s);
        assert_eq!(s.output(sync, 2), Some(0.0));
        assert!(timeline.base(video) > 0.0);
    }
    g.set_parameter(forward, 0, 1.0, &f).unwrap();
    s.rebuild(&g, &f).unwrap();
    s.tick(0.5);
    assert!((s.output(sync, 0).unwrap() - 1.3005).abs() < 1e-5);
    g.set_parameter(forward, 0, 0.0, &f).unwrap();
    s.rebuild(&g, &f).unwrap();
    s.tick(0.6);
    assert!((s.output(sync, 0).unwrap() - 1.275).abs() < 1e-5);
}
#[test]
fn zero_attack_and_release_have_instantaneous_neutral_behavior() {
    let f = registry();
    let mut g = Graph::default();
    let b = g.add(ids::BUTTON, &f).unwrap();
    let envelope = g.add("Envelope", &f).unwrap();
    wire(&mut g, b, 1, envelope, 0);
    g.set_parameter(envelope, 0, 0.0, &f).unwrap();
    g.set_parameter(envelope, 1, 0.0, &f).unwrap();
    let mut s = SignalEngine::default();
    s.rebuild(&g, &f).unwrap();
    s.events.push(ControlEvent::Trigger(b));
    s.tick(0.0);
    assert_eq!(s.output(envelope, 0), Some(1.0));
    s.tick(0.0);
    assert_eq!(s.output(envelope, 0), Some(0.0));
}
