use super::*;
use vjay_domain::graph::Connection;
#[test]
fn encapsulating_and_editing_nested_nodes_preserves_program_and_steady_execution() {
    let (mut s, counts, calls) = fixture(Project::demo(&builtin_registry()).unwrap().document());
    s.render_at(0.0).unwrap();
    let original = s
        .plan()
        .unwrap()
        .steps
        .iter()
        .map(|p| p.node)
        .collect::<Vec<_>>();
    let patch = s.encapsulate(&[NodeId(2), NodeId(3)]).unwrap();
    s.render_at(0.1).unwrap();
    assert_eq!(
        s.plan()
            .unwrap()
            .steps
            .iter()
            .map(|p| p.node)
            .collect::<Vec<_>>(),
        original
    );
    assert_eq!(s.schema(patch).inputs.len(), 2);
    assert_eq!(s.schema(patch).outputs.len(), 1);
    s.enter_subpatch(patch).unwrap();
    assert!(s.project().graph.node(NodeId(2)).is_ok());
    s.set_parameter(NodeId(2), 0, 0.7).unwrap();
    s.render_at(0.2).unwrap();
    assert!(
        (s.plan()
            .unwrap()
            .steps
            .iter()
            .find(|p| p.node == NodeId(2))
            .unwrap()
            .operation
            .parameters[0]
            - 0.7)
            .abs()
            < 1e-6
    );
    let nested = s.encapsulate(&[NodeId(2)]).unwrap();
    s.enter_subpatch(nested).unwrap();
    s.render_at(0.3).unwrap();
    let prepares = counts.borrow().prepares;
    let lookups = calls.load(Ordering::Relaxed);
    for frame in 4..20 {
        s.render_at(frame as f64 * 0.1).unwrap();
    }
    assert_eq!(counts.borrow().prepares, prepares);
    assert_eq!(calls.load(Ordering::Relaxed), lookups);
    s.leave_to(0).unwrap();
    s.render_at(2.0).unwrap();
    let doc = s.project().document();
    assert_eq!(doc.subpatches.len(), 1);
    assert_eq!(doc.subpatches[0].document.subpatches.len(), 1);
    let restored = Project::restore(doc, &builtin_registry()).unwrap();
    assert_eq!(restored.subpatches.len(), 1);
    let maximum = s
        .project()
        .all_ids()
        .0
        .into_iter()
        .map(|id| id.0)
        .max()
        .unwrap();
    let edge = s
        .project()
        .graph
        .connections()
        .find(|edge| edge.from == patch)
        .unwrap();
    let inserted = s
        .insert_node(edge, "Blur", Point { x: 600.0, y: 150.0 })
        .unwrap();
    assert!(
        inserted.0 > maximum,
        "Insertion must reserve IDs used by children"
    );
    s.render_at(2.1).unwrap();
}
#[test]
fn copied_subpatches_have_independent_state_and_unique_ids() {
    let (mut s, _, _) = fixture(Project::demo(&builtin_registry()).unwrap().document());
    let patch = s.encapsulate(&[NodeId(2), NodeId(3)]).unwrap();
    let fragment = s.copy_nodes(&[patch]).unwrap();
    let copied = s
        .paste_nodes(&fragment, Point { x: 600.0, y: 300.0 })
        .unwrap()[0];
    assert!(s.is_subpatch(copied));
    let first = s
        .project()
        .subpatches
        .iter()
        .find(|p| p.node == patch)
        .unwrap()
        .project
        .graph
        .nodes()
        .iter()
        .map(|n| n.id)
        .collect::<std::collections::HashSet<_>>();
    let second = s
        .project()
        .subpatches
        .iter()
        .find(|p| p.node == copied)
        .unwrap()
        .project
        .graph
        .nodes()
        .iter()
        .map(|n| n.id)
        .collect::<std::collections::HashSet<_>>();
    assert!(first.is_disjoint(&second));
    s.enter_subpatch(copied).unwrap();
    let mix = s
        .project()
        .graph
        .nodes()
        .iter()
        .find(|n| n.type_id == ids::MIX)
        .unwrap()
        .id;
    s.set_parameter(mix, 0, 0.9).unwrap();
    s.leave_subpatch().unwrap();
    assert_ne!(
        s.project()
            .subpatches
            .iter()
            .find(|p| p.node == patch)
            .unwrap()
            .project
            .graph
            .node(NodeId(2))
            .unwrap()
            .parameters[0],
        0.9
    );
}
#[test]
fn scalar_and_text_boundaries_preserve_live_values_and_port_renames_preserve_wires() {
    let (mut s, _, _) = fixture(Project::demo(&builtin_registry()).unwrap().document());
    let source = s.add_node("Constant", Point { x: 0.0, y: 0.0 }).unwrap();
    let math = s.add_node("Multiply", Point { x: 250.0, y: 0.0 }).unwrap();
    s.set_parameter(source, 0, 3.0).unwrap();
    s.set_parameter(math, 1, 5.0).unwrap();
    s.connect(Connection {
        from: source,
        output: 0,
        to: math,
        input: 0,
    })
    .unwrap();
    let consumer = s.add_node("Add", Point { x: 500.0, y: 0.0 }).unwrap();
    s.connect(Connection {
        from: math,
        output: 0,
        to: consumer,
        input: 0,
    })
    .unwrap();
    let patch = s.encapsulate(&[math]).unwrap();
    s.render_at(0.0).unwrap();
    assert_eq!(s.signal_value(consumer, 0), Some(15.0));
    assert_eq!(s.signal_value(patch, 0), Some(15.0));
    s.enter_subpatch(patch).unwrap();
    let inlet = s
        .project()
        .graph
        .nodes()
        .iter()
        .find(|n| n.type_id == "In")
        .unwrap()
        .id;
    let mut settings = s.project().graph.node(inlet).unwrap().settings.clone();
    settings
        .properties
        .insert("name".into(), "Amplitude".into());
    s.set_settings(inlet, settings).unwrap();
    s.leave_subpatch().unwrap();
    assert_eq!(s.schema(patch).inputs[0].name, "Amplitude");
    s.render_at(0.1).unwrap();
    assert_eq!(s.signal_value(consumer, 0), Some(15.0));
    let text = s.add_node("TextValue", Point { x: 0.0, y: 0.0 }).unwrap();
    {
        let display = s.add_node("Text", Point { x: 0.0, y: 0.0 }).unwrap();
        s.connect(Connection {
            from: text,
            output: 0,
            to: display,
            input: 0,
        })
        .unwrap();
        let patch = s.encapsulate(&[text]).unwrap();
        s.render_at(0.2).unwrap();
        assert!(s.text_value(patch, 0).is_some());
    }
}
#[test]
fn changing_port_order_tracks_identity_and_nested_save_uses_the_root_project() {
    let (mut s, _, _) = fixture(Project::demo(&builtin_registry()).unwrap().document());
    let patch = s.encapsulate(&[NodeId(2), NodeId(3)]).unwrap();
    let before = s
        .project()
        .graph
        .connections()
        .filter(|e| e.to == patch)
        .collect::<Vec<_>>();
    s.enter_subpatch(patch).unwrap();
    let inputs = s
        .project()
        .graph
        .nodes()
        .iter()
        .filter(|n| n.type_id == "In")
        .map(|n| n.id)
        .collect::<Vec<_>>();
    let mut settings = s.project().graph.node(inputs[0]).unwrap().settings.clone();
    settings.properties.insert("order".into(), "9".into());
    s.set_settings(inputs[0], settings).unwrap();
    s.save().unwrap();
    s.leave_subpatch().unwrap();
    let after = s
        .project()
        .graph
        .connections()
        .filter(|e| e.to == patch)
        .collect::<Vec<_>>();
    for old in before {
        let new = after.iter().find(|e| e.from == old.from).unwrap();
        assert_ne!(old.input, new.input);
    }
}

#[test]
fn copying_an_inlet_creates_a_new_port_identity_and_does_not_break_navigation() {
    let (mut s, _, _) = fixture(Project::demo(&builtin_registry()).unwrap().document());
    let patch = s.encapsulate(&[NodeId(2), NodeId(3)]).unwrap();
    s.enter_subpatch(patch).unwrap();
    let inlet = s
        .project()
        .graph
        .nodes()
        .iter()
        .find(|n| n.type_id == "In")
        .unwrap()
        .id;
    let fragment = s.copy_nodes(&[inlet]).unwrap();
    s.paste_nodes(&fragment, Point { x: 20.0, y: 500.0 })
        .unwrap();
    s.leave_subpatch().unwrap();
    assert_eq!(s.schema(patch).inputs.len(), 3);
    s.render_at(0.0).unwrap();
}

#[test]
fn reconnect_rejects_an_instantaneous_cycle_hidden_by_a_delayed_subpatch_output() {
    let (mut s, _, _) = fixture(Project::demo(&builtin_registry()).unwrap().document());
    let position = Point { x: 0.0, y: 0.0 };
    let source = s.add_node("Constant", position).unwrap();
    let video = s.add_node("Video", position).unwrap();
    let mix = s.add_node("Multiply", position).unwrap();
    let consumer = s.add_node("Add", position).unwrap();
    for (from, output, to, input) in [
        (source, 0, mix, 0),
        (video, 2, mix, 1),
        (mix, 0, consumer, 0),
    ] {
        s.connect(Connection {
            from,
            output,
            to,
            input,
        })
        .unwrap();
    }
    let patch = s.encapsulate(&[video, mix]).unwrap();
    assert!(s.schema(patch).outputs[0].delayed);
    let previous = Connection {
        from: source,
        output: 0,
        to: patch,
        input: 0,
    };
    let cycle = Connection {
        from: consumer,
        ..previous
    };
    assert!(s.connect(cycle).is_err());
    assert!(s.reconnect(previous, cycle).is_err());
    assert!(s.project().graph.has_connection(previous));
    assert!(!s.project().graph.has_connection(cycle));
    s.render_at(0.0).unwrap();
}
