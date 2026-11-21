use super::*;
use crate::control_io::ControlEvent;
use vjay_domain::{graph::Connection, transport::TransportAction};
#[test]
fn media_transport_works_without_a_controller_and_never_writes_a_rate_source() {
    let (mut s, counts, calls) = fixture(Project::demo(&builtin_registry()).unwrap().document());
    counts.borrow_mut().duration = Some(20.0);
    let video = s.import_video("/fixtures/clip.mov".into(), None).unwrap();
    let controller = s.setup_video_sync(video).unwrap();
    s.set_parameter(controller, 1, 103.0).unwrap();
    s.render_at(0.0).unwrap();
    s.render_at(1.0).unwrap();
    let stored = s
        .project()
        .graph
        .node(controller)
        .unwrap()
        .parameters
        .clone();
    let lookups = calls.load(Ordering::Relaxed);
    let prepared = counts.borrow().prepares;
    for a in [
        TransportAction::Pause,
        TransportAction::Seek(0.6),
        TransportAction::SetCue,
        TransportAction::Jog(0.01),
        TransportAction::RecallCue,
        TransportAction::Play,
    ] {
        s.transport(video, a).unwrap();
        s.render_at(1.1).unwrap();
    }
    assert_eq!(
        s.project().graph.node(controller).unwrap().parameters,
        stored
    );
    assert_eq!(counts.borrow().prepares, prepared);
    assert_eq!(calls.load(Ordering::Relaxed), lookups);
    assert!((s.clip_position(video).unwrap().seconds - 12.0).abs() < 1e-4);
    s.disconnect(video, s.schema(video).inputs.len());
    s.remove_node(controller).unwrap();
    s.render_at(1.2).unwrap();
    s.toggle_clip_playback(video).unwrap();
    s.render_at(1.3).unwrap();
    assert!(!s.media_state(video).unwrap().playing);
}
#[test]
fn keyboard_and_buttons_drive_transport_and_the_seek_control_tracks_actual_playhead() {
    let (mut s, counts, _) = fixture(Project::demo(&builtin_registry()).unwrap().document());
    counts.borrow_mut().duration = Some(10.0);
    let video = s.import_video("/fixtures/clip.mov".into(), None).unwrap();
    let key = s.add_node("Keyboard", Point { x: 0.0, y: 0.0 }).unwrap();
    let toggle = s.schema(video).capabilities.transport.unwrap().inputs[2];
    s.connect(Connection {
        from: key,
        output: 1,
        to: video,
        input: toggle,
    })
    .unwrap();
    let seek = s.setup_seek_control(video).unwrap();
    s.render_at(0.0).unwrap();
    s.render_at(1.0).unwrap();
    s.render_at(1.0).unwrap();
    assert!((s.signal_value(seek, 1).unwrap() - 0.1).abs() < 1e-5);
    s.input_event(ControlEvent::Key {
        key: "space".into(),
        down: true,
    });
    s.render_at(1.1).unwrap();
    assert!(!s.media_state(video).unwrap().playing);
    let p = s.clip_position(video).unwrap().seconds;
    s.render_at(2.0).unwrap();
    assert_eq!(s.clip_position(video).unwrap().seconds, p);
    s.input_event(ControlEvent::Key {
        key: "space".into(),
        down: false,
    });
    s.render_at(2.1).unwrap();
    s.input_event(ControlEvent::Key {
        key: "space".into(),
        down: true,
    });
    s.render_at(2.2).unwrap();
    assert!(s.media_state(video).unwrap().playing);
    s.set_parameter(seek, 0, 0.7).unwrap();
    s.render_at(2.3).unwrap();
    s.render_at(2.3).unwrap();
    assert!((s.signal_value(seek, 1).unwrap() - 0.7).abs() < 1e-5);
    assert!((s.control_value(s.project().controls.last().unwrap()) - 0.7).abs() < 1e-5);
    s.render_at(3.3).unwrap();
    assert!((s.media_state(video).unwrap().position - 0.8).abs() < 1e-5);
    s.set_parameter(seek, 0, 0.7).unwrap();
    s.render_at(3.4).unwrap();
    assert!((s.media_state(video).unwrap().position - 0.7).abs() < 1e-5);
}
#[test]
fn held_nudge_and_relative_jog_are_graph_inputs_and_update_audio_and_video_together() {
    let (mut s, counts, _) = fixture(Project::demo(&builtin_registry()).unwrap().document());
    counts.borrow_mut().duration = Some(20.0);
    let video = s.import_video("/fixtures/clip.mov".into(), None).unwrap();
    let hold = s.add_node("Button", Point { x: 0.0, y: 0.0 }).unwrap();
    let step = s.add_node("Button", Point { x: 0.0, y: 0.0 }).unwrap();
    let multiply = s.add_node("Multiply", Point { x: 0.0, y: 0.0 }).unwrap();
    s.set_parameter(multiply, 1, 0.01).unwrap();
    let binding = s.schema(video).capabilities.transport.unwrap();
    s.connect(Connection {
        from: hold,
        output: 0,
        to: video,
        input: binding.inputs[5],
    })
    .unwrap();
    s.connect(Connection {
        from: step,
        output: 1,
        to: multiply,
        input: 0,
    })
    .unwrap();
    s.connect(Connection {
        from: multiply,
        output: 0,
        to: video,
        input: binding.inputs[3],
    })
    .unwrap();
    s.render_at(0.0).unwrap();
    s.set_parameter(hold, 0, 1.0).unwrap();
    s.render_at(1.0).unwrap();
    assert!((s.media_state(video).unwrap().speed - 1.02).abs() < 1e-6);
    s.set_parameter(hold, 0, 0.0).unwrap();
    s.render_at(2.0).unwrap();
    assert_eq!(s.media_state(video).unwrap().speed, 1.0);
    let p = s.clip_position(video).unwrap().seconds;
    s.input_event(ControlEvent::Trigger(step));
    s.render_at(2.0).unwrap();
    assert!((s.clip_position(video).unwrap().seconds - p - 0.01).abs() < 1e-5);
}

#[test]
fn seek_feedback_survives_subpatch_boundaries_and_cues_are_saved_with_the_media() {
    let (mut s, counts, _) = fixture(Project::demo(&builtin_registry()).unwrap().document());
    counts.borrow_mut().duration = Some(10.0);
    let video = s.import_video("/fixtures/clip.mov".into(), None).unwrap();
    let seek = s.setup_seek_control(video).unwrap();
    let patch = s.encapsulate(&[seek]).unwrap();
    s.render_at(0.0).unwrap();
    s.render_at(1.0).unwrap();
    s.render_at(1.0).unwrap();
    assert!((s.signal_value(patch, 0).unwrap()).abs() < 1e-5);
    s.mark_cue(video).unwrap();
    s.render_at(1.0).unwrap();
    s.save().unwrap();
    s.seek_clip(video, 0.7).unwrap();
    s.render_at(1.0).unwrap();
    s.load().unwrap();
    s.render_at(0.0).unwrap();
    s.return_to_cue(video).unwrap();
    s.render_at(0.0).unwrap();
    assert!((s.clip_position(video).unwrap().seconds - 1.0).abs() < 1e-4);
}

#[test]
fn playing_an_ended_nonlooping_clip_restarts_its_range_and_cue_saves_immediately() {
    let (mut s, counts, _) = fixture(Project::demo(&builtin_registry()).unwrap().document());
    counts.borrow_mut().duration = Some(10.0);
    let video = s.import_video("/fixtures/clip.mov".into(), None).unwrap();
    s.set_parameter(video, 2, 0.0).unwrap();
    s.render_at(0.0).unwrap();
    s.render_at(11.0).unwrap();
    assert!(!s.media_state(video).unwrap().playing);
    assert_eq!(s.media_state(video).unwrap().speed, 0.0);
    s.toggle_clip_playback(video).unwrap();
    s.render_at(11.0).unwrap();
    assert!(s.media_state(video).unwrap().playing);
    assert!(s.clip_position(video).unwrap().seconds < 1e-5);
    s.render_at(12.0).unwrap();
    s.mark_cue(video).unwrap();
    s.save().unwrap();
    s.load().unwrap();
    s.render_at(0.0).unwrap();
    s.return_to_cue(video).unwrap();
    s.render_at(0.0).unwrap();
    assert!((s.clip_position(video).unwrap().seconds - 1.0).abs() < 1e-4);
}

#[test]
fn legacy_connected_deck_cues_migrate_without_rewiring_the_saved_patch() {
    let (mut s, counts, _) = fixture(Project::demo(&builtin_registry()).unwrap().document());
    counts.borrow_mut().duration = Some(20.0);
    let video = s.import_video("/fixtures/clip.mov".into(), None).unwrap();
    let legacy = s.add_node("LipSync", Point { x: 0.0, y: 0.0 }).unwrap();
    let b = s.schema(legacy).capabilities.tempo_sync.unwrap();
    s.set_parameter(legacy, b.manual.unwrap().marker, 0.4)
        .unwrap();
    for (output, parameter) in b.outputs.into_iter().zip([0, 1, 5]) {
        s.connect(Connection {
            from: legacy,
            output,
            to: video,
            input: s.schema(video).inputs.len() + parameter,
        })
        .unwrap();
    }
    let mut doc = s.project().document();
    doc.nodes
        .iter_mut()
        .find(|n| n.id == video)
        .unwrap()
        .settings
        .port_version = 0;
    for e in &mut doc.connections {
        if e.to == video {
            e.input -= 8;
        }
    }
    s.restore(doc).unwrap();
    assert_eq!(
        s.project()
            .graph
            .connections()
            .filter(|e| e.to == video)
            .count(),
        3
    );
    s.render_at(0.0).unwrap();
    s.return_to_cue(video).unwrap();
    s.render_at(0.0).unwrap();
    assert!((s.clip_position(video).unwrap().seconds - 8.0).abs() < 1e-4);
}
