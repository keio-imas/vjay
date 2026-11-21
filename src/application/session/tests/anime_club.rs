use super::*;
use crate::{
    audio::AudioPlan, control_io::ControlEvent, media_timeline::MediaTimeline,
    signals::SignalEngine,
};

#[test]
fn anime_club_starts_with_empty_decks_and_keeps_transport_and_headphones_independent() {
    let doc: ProjectDocument =
        serde_json::from_str(include_str!("../../../../sample/anime-club.vjay.json")).unwrap();
    let find = |label: &str| {
        doc.nodes
            .iter()
            .find(|n| n.settings.label == label)
            .unwrap()
            .id
    };
    let a = find("A / VIDEO");
    let b = find("B / VIDEO");
    let (mut s, counts, _) = fixture(doc.clone());
    s.restore(doc.clone()).unwrap();
    s.render_at(0.0).unwrap();
    assert!(s.plan().unwrap().program_active);
    assert!(s.audio_error.is_none());
    counts.borrow_mut().duration = Some(10.0);
    s.import_video("/fixtures/a.mov".into(), Some(a)).unwrap();
    s.render_at(0.0).unwrap();
    let nodes = builtin_registry();
    let mut signals = SignalEngine::default();
    signals.rebuild(&s.project().graph, &nodes).unwrap();
    assert_eq!(
        AudioPlan::compile(&s.project().graph, &signals)
            .unwrap()
            .routes
            .len(),
        1,
        "An empty B deck must not suppress A's headphone route"
    );
    s.import_video("/fixtures/b.mov".into(), Some(b)).unwrap();
    s.render_at(0.0).unwrap();
    s.seek_clip(a, 0.25).unwrap();
    s.seek_clip(b, 0.6).unwrap();
    s.render_at(0.0).unwrap();
    s.render_at(0.0).unwrap();
    let seek = |s: &Session, label: &str| {
        s.control_readout(
            s.project()
                .controls
                .iter()
                .find(|c| c.label == label)
                .unwrap(),
            3,
        )
    };
    assert_eq!(seek(&s, "A / Seek"), "00:00:02.500 / 00:00:10.000");
    assert_eq!(seek(&s, "B / Seek"), "00:00:06.000 / 00:00:10.000");
    s.set_parameter(find("A / NUDGE +"), 0, 1.0).unwrap();
    s.render_at(0.0).unwrap();
    assert!((s.media_state(a).unwrap().speed - 1.02).abs() < 1e-6);
    assert_eq!(s.media_state(b).unwrap().speed, 1.0);
    s.set_parameter(find("A / NUDGE +"), 0, 0.0).unwrap();
    s.input_event(ControlEvent::Trigger(find("A / JOG +10 MS")));
    s.render_at(0.0).unwrap();
    assert!((s.media_state(a).unwrap().seconds - 2.51).abs() < 1e-4);
    assert!((s.media_state(b).unwrap().seconds - 6.0).abs() < 1e-4);
    s.input_event(ControlEvent::Key {
        key: "q".into(),
        down: true,
    });
    s.render_at(0.0).unwrap();
    assert!(!s.media_state(a).unwrap().playing);
    assert!(s.media_state(b).unwrap().playing);
    let headphones = find("HEADPHONE SELECT A / B");
    s.set_parameter(headphones, 0, 1.0).unwrap();
    signals.rebuild(&s.project().graph, &nodes).unwrap();
    signals.tick(0.0);
    let mut audio = AudioPlan::compile(&s.project().graph, &signals).unwrap();
    audio.update(&signals, &MediaTimeline::default());
    assert!((audio.routes.iter().find(|r| r.node == a).unwrap().gain - 0.7).abs() < 1e-6);
    assert_eq!(audio.routes.iter().find(|r| r.node == b).unwrap().gain, 0.0);
    s.set_parameter(find("CROSSFADE A / B"), 0, 1.0).unwrap();
    s.set_parameter(find("MASTER / BLACKOUT"), 0, 1.0).unwrap();
    assert_eq!(
        s.project().graph.node(headphones).unwrap().parameters,
        vec![1.0, 0.0]
    );
}
