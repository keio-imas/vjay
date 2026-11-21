use super::*;
use vjay_domain::{graph::Connection, port::SignalType};
#[test]
fn files_are_typed_can_fan_out_and_clear_missing_artwork_without_hot_graph_walks() {
    let (mut s, counts, calls) = fixture(Project::demo(&builtin_registry()).unwrap().document());
    let files = s.add_node("Files", Point { x: 0.0, y: 0.0 }).unwrap();
    let image = s
        .add_node("LoadableImage", Point { x: 250.0, y: 0.0 })
        .unwrap();
    let second = s
        .add_node("LoadableImage", Point { x: 500.0, y: 0.0 })
        .unwrap();
    let output = s.project().output.unwrap();
    assert_eq!(s.port_signal(files, 0), Some(SignalType::File));
    for target in [image, second] {
        s.connect(Connection {
            from: files,
            output: 0,
            to: target,
            input: 0,
        })
        .unwrap();
    }
    s.connect(Connection {
        from: image,
        output: 0,
        to: output,
        input: 0,
    })
    .unwrap();
    s.pin_files(files).unwrap();
    assert_eq!(
        s.project().controls.last().unwrap().widget,
        WidgetKind::Files
    );
    let path: Arc<std::path::Path> = Arc::from(std::path::Path::new("/fixtures/cover.png"));
    s.input_event(crate::control_io::ControlEvent::NodeFile {
        node: files,
        values: vec![Some(path)],
    });
    s.render_at(0.0).unwrap();
    s.render_at(0.1).unwrap();
    assert!(s.project().graph.node(image).unwrap().asset.is_some());
    assert!(s.project().graph.node(second).unwrap().asset.is_some());
    let prepared = counts.borrow().prepares;
    let lowered = calls.load(Ordering::Relaxed);
    for i in 2..20 {
        s.render_at(i as f64 / 60.0).unwrap();
    }
    assert_eq!(counts.borrow().prepares, prepared);
    assert_eq!(calls.load(Ordering::Relaxed), lowered);
    s.input_event(crate::control_io::ControlEvent::NodeFile {
        node: files,
        values: vec![None],
    });
    s.render_at(1.0).unwrap();
    s.render_at(1.1).unwrap();
    assert!(s.project().graph.node(image).unwrap().asset.is_none());
    assert!(s.project().graph.node(second).unwrap().asset.is_none());
}
#[test]
fn font_controls_update_prepared_properties_without_recreating_gpu_resources() {
    let (mut s, counts, _) = fixture(Project::demo(&builtin_registry()).unwrap().document());
    let value = s.add_node(ids::CONSTANT, Point { x: 0.0, y: 0.0 }).unwrap();
    let text = s.add_node("Text", Point { x: 250.0, y: 0.0 }).unwrap();
    let output = s.project().output.unwrap();
    s.set_parameter(value, 0, 64.0).unwrap();
    s.connect(Connection {
        from: value,
        output: 0,
        to: text,
        input: 6,
    })
    .unwrap();
    s.connect(Connection {
        from: text,
        output: 0,
        to: output,
        input: 0,
    })
    .unwrap();
    s.render_at(0.0).unwrap();
    let prepared = counts.borrow().prepares;
    s.set_parameter(value, 0, 80.0).unwrap();
    s.render_at(0.1).unwrap();
    assert_eq!(counts.borrow().prepares, prepared);
    assert_eq!(
        s.plan()
            .unwrap()
            .steps
            .iter()
            .find(|p| p.node == text)
            .unwrap()
            .operation
            .prepared
            .as_ref()
            .unwrap()
            .properties["font.size"],
        "80"
    );
}
#[test]
fn bpm_sync_setup_is_explicit_idempotent_and_monitoring_is_optional() {
    let (mut s, _, _) = fixture(Project::demo(&builtin_registry()).unwrap().document());
    let video = s.import_video("/fixtures/clip.mov".into(), None).unwrap();
    let before = s.project().graph.connection_count();
    assert!(!s.monitored_video(video));
    let sync = s.setup_video_sync(video).unwrap();
    assert_eq!(s.project().graph.connection_count(), before + 3);
    assert_eq!(s.setup_video_sync(video).unwrap(), sync);
    let binding = s.schema(sync).capabilities.tempo_sync.unwrap();
    s.set_parameter(sync, binding.manual.unwrap().mode, 1.0)
        .unwrap();
    s.set_parameter(sync, binding.clip_bpm, 100.0).unwrap();
    s.set_parameter(sync, binding.dj_bpm, 120.0).unwrap();
    s.render_at(0.0).unwrap();
    assert!((s.signal_value(sync, binding.outputs[0]).unwrap() - 1.2).abs() < 1e-6);
    s.monitor_video(video, true).unwrap();
    assert!(s.monitored_video(video));
    s.monitor_video(video, false).unwrap();
    assert!(!s.monitored_video(video));
    let other = s.import_video("/fixtures/other.mov".into(), None).unwrap();
    let value = s.add_node(ids::CONSTANT, Point { x: 0.0, y: 0.0 }).unwrap();
    s.connect(Connection {
        from: value,
        output: 0,
        to: other,
        input: 0,
    })
    .unwrap();
    let nodes = s.project().graph.nodes().len();
    assert!(s.setup_video_sync(other).is_err());
    assert_eq!(s.project().graph.nodes().len(), nodes);
}
