use super::*;
use vjay_domain::graph::Connection;
#[test]
fn ear_matching_needs_no_bpm_jogs_from_playhead_and_retains_pitch_after_pause() {
    let (mut s, counts, _) = fixture(Project::demo(&builtin_registry()).unwrap().document());
    counts.borrow_mut().duration = Some(20.0);
    let video = s.import_video("/fixtures/clip.mov".into(), None).unwrap();
    let sync = s.setup_video_sync(video).unwrap();
    let b = s.schema(sync).capabilities.tempo_sync.unwrap();
    let manual = b.manual.unwrap();
    s.set_parameter(sync, b.clip_bpm, 333.0).unwrap();
    s.set_parameter(sync, b.dj_bpm, 81.0).unwrap();
    s.render_at(0.0).unwrap();
    assert_eq!(s.signal_value(sync, 0), Some(1.0));
    s.adjust_deck_rate(sync, 1.5).unwrap();
    s.render_at(1.0).unwrap();
    assert!((s.signal_value(sync, 0).unwrap() - 1.015).abs() < 1e-6);
    s.set_parameter(sync, b.nudge, 2.0).unwrap();
    s.render_at(2.0).unwrap();
    assert!((s.signal_value(sync, 0).unwrap() - 1.0353).abs() < 1e-6);
    s.set_parameter(sync, b.nudge, 0.0).unwrap();
    s.render_at(3.0).unwrap();
    let before = s.clip_position(video).unwrap().seconds;
    assert!(before > 2.0);
    s.jog_clip(video, 0.01).unwrap();
    s.render_at(3.001).unwrap();
    assert!(
        (s.clip_position(video).unwrap().seconds - before - 0.01).abs() < 1e-4,
        "jog must not return to the original cue"
    );
    s.mark_cue(video).unwrap();
    let cue = s.clip_position(video).unwrap().seconds;
    s.toggle_clip_playback(video).unwrap();
    s.render_at(3.1).unwrap();
    let paused = s.clip_position(video).unwrap().seconds;
    s.render_at(4.1).unwrap();
    assert_eq!(s.clip_position(video).unwrap().seconds, paused);
    s.toggle_clip_playback(video).unwrap();
    s.render_at(4.2).unwrap();
    assert_eq!(
        s.project().graph.node(sync).unwrap().parameters[manual.rate],
        101.5
    );
    s.return_to_cue(video).unwrap();
    s.render_at(4.3).unwrap();
    assert!((s.clip_position(video).unwrap().seconds - cue).abs() < 1e-4);
}
#[test]
fn partially_wired_deck_is_not_mistaken_for_a_complete_controller() {
    let (mut s, _, _) = fixture(Project::demo(&builtin_registry()).unwrap().document());
    let video = s.import_video("/fixtures/clip.mov".into(), None).unwrap();
    let sync = s.add_node("LipSync", Point { x: 0.0, y: 0.0 }).unwrap();
    s.connect(Connection {
        from: sync,
        output: 0,
        to: video,
        input: 0,
    })
    .unwrap();
    assert_eq!(s.video_sync(video), None);
    let count = s.project().graph.connection_count();
    assert!(s.setup_video_sync(video).is_err());
    assert_eq!(s.project().graph.connection_count(), count);
}
#[test]
fn old_bpm_controller_projects_keep_their_mode_and_cue() {
    let (mut s, _, _) = fixture(Project::demo(&builtin_registry()).unwrap().document());
    let sync = s.add_node("LipSync", Point { x: 0.0, y: 0.0 }).unwrap();
    let mut doc = s.project().document();
    let node = doc.nodes.iter_mut().find(|n| n.id == sync).unwrap();
    node.parameters.truncate(10);
    node.parameters[4] = 0.3;
    s.restore(doc).unwrap();
    let b = s
        .schema(sync)
        .capabilities
        .tempo_sync
        .unwrap()
        .manual
        .unwrap();
    let p = &s.project().graph.node(sync).unwrap().parameters;
    assert_eq!(p[b.mode], 1.0);
    assert_eq!(p[b.marker], 0.3);
}
