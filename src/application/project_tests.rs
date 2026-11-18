use crate::project::*;
use vjay_domain::builtins::registry as builtin_registry;
use vjay_domain::{builtins::ids, graph::Connection};

#[test]
fn roundtrip_preserves_patch_and_performance_bindings() {
    let f = builtin_registry();
    let project = Project::demo(&f).unwrap();
    let restored = Project::restore(project.document(), &f).unwrap();
    assert!(restored.graph.connections().eq(project.graph.connections()));
    assert_eq!(restored.controls.len(), 3);
    assert_eq!(restored.controls[1].node, project.controls[1].node);
    assert_eq!(
        restored.positions[1].position.x,
        project.positions[1].position.x
    );
}

#[test]
fn workspace_layout_is_validated_and_roundtrips() {
    let f = builtin_registry();
    let mut document = Project::demo(&f).unwrap().document();
    document.workspace.library_width = 220.0;
    document.workspace.preview_ratio = 0.65;
    let project = Project::restore(document.clone(), &f).unwrap();
    assert_eq!(project.workspace, document.workspace);
    document.workspace.preview_ratio = f32::NAN;
    assert!(Project::restore(document, &f).is_err());
}

#[test]
fn malformed_documents_are_rejected() {
    let f = builtin_registry();
    let good = Project::demo(&f).unwrap().document();
    let mut bad = good.clone();
    bad.version = 42;
    assert!(Project::restore(bad, &f).is_err());
    let mut bad = good.clone();
    bad.nodes[0].parameters[0] = f32::INFINITY;
    assert!(Project::restore(bad, &f).is_err());
    let mut bad = good.clone();
    bad.connections.push(bad.connections[0]);
    assert!(Project::restore(bad, &f).is_err());
    let mut bad = good.clone();
    bad.controls[0].parameter = usize::MAX;
    assert!(Project::restore(bad, &f).is_err());
    let mut bad = good.clone();
    bad.positions[0].position.x = f32::NAN;
    assert!(Project::restore(bad, &f).is_err());
    let mut bad = good;
    bad.controls[0].id = u64::MAX;
    assert!(Project::restore(bad, &f).is_err());
}

#[test]
fn an_empty_project_without_output_and_saved_midi_source_can_be_restored() {
    let f = builtin_registry();
    let mut project = Project::empty(&f).unwrap();
    let output = project.output.take().unwrap();
    project.graph.remove(output).unwrap();
    project.positions.clear();
    let empty = Project::restore(project.document(), &f).unwrap();
    assert!(empty.output.is_none());
    assert!(empty.graph.nodes().is_empty());
    let node = project.graph.add(ids::MIDI, &f).unwrap();
    project
        .graph
        .set_settings(
            node,
            vjay_domain::settings::NodeSettings {
                midi_source: Some("vjay:virtual-input".into()),
                ..Default::default()
            },
            &f,
        )
        .unwrap();
    project.positions.push(NodePosition {
        node,
        position: Point { x: 10.0, y: 10.0 },
    });
    let restored = Project::restore(project.document(), &f).unwrap();
    assert_eq!(
        restored
            .graph
            .node(node)
            .unwrap()
            .settings
            .midi_source
            .as_deref(),
        Some("vjay:virtual-input")
    );
}

#[test]
fn old_text_parameter_connections_migrate_and_do_not_shift_again() {
    let f = builtin_registry();
    let mut p = Project::empty(&f).unwrap();
    let value = p.graph.add(ids::CONSTANT, &f).unwrap();
    let text = p.graph.add("Text", &f).unwrap();
    for node in [value, text] {
        p.positions.push(NodePosition {
            node,
            position: Point { x: 0.0, y: 0.0 },
        });
    }
    p.graph
        .connect(Connection {
            from: value,
            output: 0,
            to: text,
            input: 2,
        })
        .unwrap();
    let mut doc = p.document();
    doc.nodes
        .iter_mut()
        .find(|n| n.id == text)
        .unwrap()
        .settings
        .port_version = 0;
    doc.connections[0].input = 0;
    let p = Project::restore(doc, &f).unwrap();
    assert_eq!(p.graph.connections().next().unwrap().input, 2);
    let p = Project::restore(p.document(), &f).unwrap();
    assert_eq!(p.graph.connections().next().unwrap().input, 2);
    let mut doc = p.document();
    doc.connections[0].input = usize::MAX;
    doc.nodes
        .iter_mut()
        .find(|n| n.id == text)
        .unwrap()
        .settings
        .port_version = 0;
    assert!(Project::restore(doc, &f).is_err());
}
