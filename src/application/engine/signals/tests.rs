use super::*;
use vjay_domain::builtins::ids;
use vjay_domain::graph::Graph;
use vjay_domain::{builtins::registry as builtin_registry, graph::Connection};
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
fn external_controls_drive_math_and_gpu_parameters_without_rebuilding() {
    let f = builtin_registry();
    let mut g = Graph::default();
    let key = g.add(ids::KEYBOARD, &f).unwrap();
    let midi = g.add(ids::MIDI, &f).unwrap();
    let osc = g.add(ids::OSC, &f).unwrap();
    let add = g.add(ids::ADD, &f).unwrap();
    let multiply = g.add(ids::MULTIPLY, &f).unwrap();
    let solid = g.add(ids::SOLID, &f).unwrap();
    let output = g.add(ids::OUTPUT, &f).unwrap();
    wire(&mut g, key, 0, add, 0);
    wire(&mut g, midi, 0, add, 1);
    wire(&mut g, add, 0, multiply, 0);
    wire(&mut g, osc, 0, multiply, 1);
    wire(&mut g, multiply, 0, solid, 1);
    wire(&mut g, solid, 0, output, 0);
    let mut engine = SignalEngine::default();
    engine.rebuild(&g, &f).unwrap();
    let mut plan = crate::compiler::compile(&g, output, &f).unwrap();
    engine.bind(&mut plan);
    engine.events.extend([
        ControlEvent::Key {
            key: "space".into(),
            down: true,
        },
        ControlEvent::Midi {
            channel: 0,
            message: MidiMessage::Cc {
                controller: 1,
                value: 127,
            },
        },
        ControlEvent::Osc {
            address: "/vjay".into(),
            value: 0.25,
        },
    ]);
    engine.tick(0.0);
    engine.modulate(&mut plan);
    assert_eq!(engine.output(key, 1), Some(1.0));
    assert_eq!(engine.output(multiply, 0), Some(0.5));
    assert_eq!(plan.steps[0].operation.parameters[1], 0.5);
    engine.tick(0.1);
    assert_eq!(engine.output(key, 1), Some(0.0));
    engine.events.push(ControlEvent::Key {
        key: "space".into(),
        down: false,
    });
    engine.tick(0.2);
    engine.modulate(&mut plan);
    assert_eq!(engine.output(multiply, 0), Some(0.25));
}
#[test]
fn lfo_multi_outputs_and_tempo_state_survive_parameter_rebuilds() {
    let f = builtin_registry();
    let mut g = Graph::default();
    let lfo = g.add(ids::LFO, &f).unwrap();
    let tempo = g.add(ids::TEMPO, &f).unwrap();
    let mut engine = SignalEngine::default();
    engine.rebuild(&g, &f).unwrap();
    engine.tick(0.0);
    engine.tick(0.25);
    assert!((engine.output(lfo, 0).unwrap() - 1.0).abs() < 0.001);
    assert_eq!(engine.output(lfo, 2), Some(0.25));
    engine.events.extend([
        ControlEvent::Tap {
            node: Some(tempo),
            seconds: 10.0,
        },
        ControlEvent::Tap {
            node: Some(tempo),
            seconds: 11.0,
        },
    ]);
    engine.tick(0.25);
    assert_eq!(engine.output(tempo, 2), Some(60.0));
    g.set_parameter(lfo, 1, 0.5, &f).unwrap();
    engine.rebuild(&g, &f).unwrap();
    engine.tick(0.25);
    assert!((engine.output(lfo, 0).unwrap() - 0.5).abs() < 0.001);
    assert_eq!(engine.output(tempo, 2), Some(60.0));
}
#[test]
fn button_short_press_still_emits_one_trigger_and_midi_note_off_clears_gate() {
    let f = builtin_registry();
    let mut g = Graph::default();
    let button = g.add(ids::BUTTON, &f).unwrap();
    let counter = g.add(ids::COUNTER, &f).unwrap();
    let midi = g.add(ids::MIDI, &f).unwrap();
    wire(&mut g, button, 1, counter, 0);
    let mut engine = SignalEngine::default();
    engine.rebuild(&g, &f).unwrap();
    engine.events.push(ControlEvent::Trigger(button));
    engine.events.push(ControlEvent::Midi {
        channel: 0,
        message: MidiMessage::Note {
            note: 60,
            velocity: 100,
        },
    });
    engine.tick(0.0);
    assert_eq!(engine.output(counter, 0), Some(1.0));
    assert_eq!(engine.output(midi, 3), Some(1.0));
    engine.events.push(ControlEvent::Midi {
        channel: 0,
        message: MidiMessage::Note {
            note: 60,
            velocity: 0,
        },
    });
    engine.tick(0.1);
    assert_eq!(engine.output(counter, 0), Some(1.0));
    assert_eq!(engine.output(midi, 3), Some(0.0));
}

#[test]
fn midi_sources_remain_isolated_and_disconnect_clears_held_notes() {
    let f = builtin_registry();
    let mut g = Graph::default();
    let a = g.add(ids::MIDI, &f).unwrap();
    let b = g.add(ids::MIDI, &f).unwrap();
    let all = g.add(ids::MIDI, &f).unwrap();
    for (node, source) in [(a, Some("a")), (b, Some("b")), (all, None)] {
        g.set_parameter(node, 0, 3.0, &f).unwrap();
        g.set_parameter(node, 1, 74.0, &f).unwrap();
        g.set_settings(
            node,
            NodeSettings {
                midi_source: source.map(str::to_owned),
                ..Default::default()
            },
            &f,
        )
        .unwrap();
    }
    let mut signals = SignalEngine::default();
    signals.rebuild(&g, &f).unwrap();
    signals.events.extend([
        ControlEvent::MidiFrom {
            source: "a".into(),
            channel: 2,
            message: MidiMessage::Cc {
                controller: 74,
                value: 64,
            },
        },
        ControlEvent::MidiFrom {
            source: "b".into(),
            channel: 2,
            message: MidiMessage::Cc {
                controller: 74,
                value: 127,
            },
        },
        ControlEvent::MidiFrom {
            source: "a".into(),
            channel: 2,
            message: MidiMessage::Note {
                note: 60,
                velocity: 100,
            },
        },
    ]);
    signals.tick(0.0);
    assert_eq!(signals.output(a, 0), Some(64.0 / 127.0));
    assert_eq!(signals.output(b, 0), Some(1.0));
    assert_eq!(signals.output(a, 3), Some(1.0));
    assert_eq!(signals.output(b, 3), Some(0.0));
    signals.clear_midi();
    signals.tick(0.1);
    assert_eq!(signals.output(a, 3), Some(0.0));
    assert_eq!(signals.output(a, 0), Some(0.0));
    assert_eq!(signals.output(all, 0), Some(0.0));
}

#[test]
fn stale_offsets_have_a_bounded_fallback_instead_of_panicking() {
    let reference = ValueRef {
        constant_slot: None,
        source: Some(6),
        value: 0.25,
        min: 0.0,
        max: 1.0,
        wrap: None,
    };
    assert_eq!(reference.read(&[0.0; 3]), 0.25);
}
