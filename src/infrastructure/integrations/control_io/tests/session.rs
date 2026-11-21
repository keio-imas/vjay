use super::*;
use crate::{gpu::WgpuRuntimeFactory, media::FfmpegDecoderFactory, storage::JsonProjectStore};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use vjay_application::{ports::*, project::Point, session::Session};
use vjay_domain::builtins::ids;
use vjay_domain::builtins::registry as builtin_registry;

#[test]
#[cfg(unix)]
#[ignore = "requires native MIDI and GPU services; sends only to the application's own virtual input"]
fn virtual_destination_to_learned_node_and_refresh_delivers_real_midi() {
    let _guard = MIDI_TEST_LOCK.lock().unwrap();
    let output = midir::MidiOutput::new("VJAY destination test").unwrap();
    let before: Vec<_> = output
        .ports()
        .iter()
        .map(midir::MidiOutputPort::id)
        .collect();
    let runtime = WgpuRuntimeFactory::headless(
        Arc::new(FfmpegDecoderFactory::new("ffmpeg".into(), "ffprobe".into())),
        crate::gpu::programs::builtin_programs(),
    )
    .unwrap()
    .with_controls(|| Box::new(NativeControlIo::default()));
    let mut session = Session::new(
        Box::new(builtin_registry()),
        &runtime,
        Box::new(JsonProjectStore::new(
            std::env::temp_dir().join("vjay-midi-test-unused.json"),
        )),
        Extent::new(32, 32).unwrap(),
    )
    .unwrap();
    let node = session
        .add_node(ids::MIDI, Point { x: 0.0, y: 0.0 })
        .unwrap();
    assert!(session.input_settings.midi);
    assert!(
        session
            .midi_ports()
            .iter()
            .any(|p| p.id == midi::VIRTUAL_SOURCE && p.error.is_none())
    );
    let destination = output
        .ports()
        .into_iter()
        .find(|p| {
            !before.contains(&p.id())
                && output.port_name(p).is_ok_and(|name| name == "VJAY MIDI In")
        })
        .expect("new VJAY MIDI destination");
    let destination_id = destination.id();
    let mut sender = output
        .connect(&destination, "VJAY isolated sender")
        .unwrap();
    session
        .set_midi_source(node, Some(midi::VIRTUAL_SOURCE.into()))
        .unwrap();
    session.learn_midi(node).unwrap();
    sender.send(&[0xb2, 74, 96]).unwrap();
    let start = Instant::now();
    while session.midi_activity().learning.is_some() && start.elapsed() < Duration::from_secs(2) {
        session.render_at(0.0).unwrap();
        std::thread::sleep(Duration::from_millis(5));
    }
    let record = session.project().graph.node(node).unwrap();
    assert_eq!(record.parameters[0], 3.0);
    assert_eq!(record.parameters[1], 74.0);
    assert_eq!(
        record.settings.midi_source.as_deref(),
        Some(midi::VIRTUAL_SOURCE)
    );
    assert!((session.signal_value(node, 0).unwrap() - 96.0 / 127.0).abs() < 1e-6);
    session.refresh_midi_inputs().unwrap();
    let output = midir::MidiOutput::new("VJAY destination check").unwrap();
    assert!(
        output.ports().iter().any(|p| p.id() == destination_id),
        "refresh must preserve the DAW destination"
    );
    sender.send(&[0x92, 60, 127]).unwrap();
    let start = Instant::now();
    while session.signal_value(node, 3) != Some(1.0) && start.elapsed() < Duration::from_secs(2) {
        session.render_at(0.0).unwrap();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(session.signal_value(node, 3), Some(1.0));
    sender.send(&[0x82, 60, 0]).unwrap();
    let start = Instant::now();
    while session.signal_value(node, 3) != Some(0.0) && start.elapsed() < Duration::from_secs(2) {
        session.render_at(0.0).unwrap();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(session.signal_value(node, 3), Some(0.0));
}
