use super::*;
use crate::builtins::ids;
use crate::builtins::registry as builtin_registry;

#[test]
fn cycle_rejection_preserves_replaced_connection() {
    let f = builtin_registry();
    let mut g = Graph::default();
    let source = g.add(ids::PATTERN, &f).unwrap();
    let a = g.add(ids::COLORIZE, &f).unwrap();
    let b = g.add(ids::COLORIZE, &f).unwrap();
    let old = Connection {
        output: 0,
        from: source,
        to: a,
        input: 0,
    };
    g.connect(old).unwrap();
    g.connect(Connection {
        output: 0,
        from: a,
        to: b,
        input: 0,
    })
    .unwrap();
    assert_eq!(
        g.connect(Connection {
            output: 0,
            from: b,
            to: a,
            input: 0
        }),
        Err(GraphError::Cycle)
    );
    assert!(g.has_connection(old));
    assert!(g.topological_order().is_ok());
}

#[test]
fn removed_ids_do_not_alias_moved_or_new_nodes() {
    let f = builtin_registry();
    let mut g = Graph::default();
    let a = g.add(ids::PATTERN, &f).unwrap();
    let b = g.add(ids::MIX, &f).unwrap();
    g.connect(Connection {
        output: 0,
        from: a,
        to: b,
        input: 0,
    })
    .unwrap();
    g.remove(a).unwrap();
    assert!(g.node(a).is_err());
    assert_eq!(g.node(b).unwrap().type_id, ids::MIX);
    assert!(g.connection_count() == 0);
    assert_ne!(g.add(ids::PATTERN, &f).unwrap(), a);
}

#[test]
fn rejects_invalid_ports_terminal_sources_and_nonfinite_parameters() {
    let f = builtin_registry();
    let mut g = Graph::default();
    let a = g.add(ids::PATTERN, &f).unwrap();
    let output = g.add(ids::OUTPUT, &f).unwrap();
    assert_eq!(
        g.connect(Connection {
            output: 0,
            from: a,
            to: output,
            input: 1
        }),
        Err(GraphError::InvalidPort)
    );
    assert_eq!(
        g.connect(Connection {
            output: 0,
            from: output,
            to: a,
            input: 0
        }),
        Err(GraphError::TerminalSource)
    );
    assert_eq!(
        g.set_parameter(a, 0, f32::NAN, &f),
        Err(GraphError::InvalidParameter)
    );
    assert_eq!(
        g.set_parameter(a, 0, 0.0, &f),
        Err(GraphError::InvalidParameter)
    );
    assert_eq!(g.node(a).unwrap().parameters[0], 8.0);
}

#[test]
fn self_cycle_and_duplicate_ids_are_rejected() {
    let f = builtin_registry();
    let mut g = Graph::default();
    let a = g.add(ids::COLORIZE, &f).unwrap();
    assert_eq!(
        g.connect(Connection {
            output: 0,
            from: a,
            to: a,
            input: 0
        }),
        Err(GraphError::Cycle)
    );
    assert_eq!(
        g.insert(g.node(a).unwrap().clone(), &f),
        Err(GraphError::InvalidId)
    );
    assert!(g.connection_count() == 0);
}

#[test]
fn all_catalog_nodes_share_valid_descriptors_and_default_lowering() {
    let f = builtin_registry();
    for entry in f.catalog() {
        let node = f.get(entry.type_id).unwrap();
        let descriptor = node.descriptor();

        let parameters: Vec<_> = descriptor.parameters.iter().map(|p| p.default).collect();
        assert!(crate::node::validate_parameters(descriptor, &parameters));
        let _ = node.image(&parameters, &Default::default(), 0);
        let _ = node.scalar();
    }
}

#[test]
fn reconnect_is_atomic_including_displaced_destination() {
    let f = builtin_registry();
    let mut g = Graph::default();
    let a = g.add(ids::PATTERN, &f).unwrap();
    let b = g.add(ids::COLORIZE, &f).unwrap();
    let c = g.add(ids::COLORIZE, &f).unwrap();
    let ab = Connection {
        output: 0,
        from: a,
        to: b,
        input: 0,
    };
    let bc = Connection {
        output: 0,
        from: b,
        to: c,
        input: 0,
    };
    g.connect(ab).unwrap();
    g.connect(bc).unwrap();
    assert!(
        g.reconnect(
            ab,
            Connection {
                output: 0,
                from: c,
                to: b,
                input: 0
            }
        )
        .is_err()
    );
    assert!(g.has_connection(ab));
    assert!(g.has_connection(bc));
    g.reconnect(
        ab,
        Connection {
            output: 0,
            from: a,
            to: c,
            input: 0,
        },
    )
    .unwrap();
    assert_eq!(
        g.connections().collect::<Vec<_>>(),
        &[Connection {
            output: 0,
            from: a,
            to: c,
            input: 0
        }]
    );
}

#[test]
fn typed_multi_outputs_and_parameter_inlets_reject_mismatches_atomically() {
    let f = builtin_registry();
    let mut g = Graph::default();
    let video = g.add(ids::VIDEO, &f).unwrap();
    let out = g.add(ids::OUTPUT, &f).unwrap();
    let audio = g.add(ids::AUDIO_OUT, &f).unwrap();
    let lfo = g.add(ids::LFO, &f).unwrap();
    let picture = Connection {
        from: video,
        output: 0,
        to: out,
        input: 0,
    };
    g.connect(picture).unwrap();
    assert_eq!(
        g.connect(Connection {
            output: 1,
            ..picture
        }),
        Err(GraphError::PortType)
    );
    assert!(g.has_connection(picture));
    g.connect(Connection {
        from: video,
        output: 1,
        to: audio,
        input: 0,
    })
    .unwrap();
    g.connect(Connection {
        from: lfo,
        output: 2,
        to: video,
        input: 0,
    })
    .unwrap();
    assert_eq!(
        g.connect(Connection {
            from: lfo,
            output: 4,
            to: video,
            input: 1
        }),
        Err(GraphError::InvalidPort)
    );
    assert_eq!(g.connection_count(), 3);
}

#[test]
fn batch_removal_validates_every_id_before_mutating_and_prunes_incident_edges() {
    let f = builtin_registry();
    let mut graph = Graph::default();
    let a = graph.add(ids::PATTERN, &f).unwrap();
    let b = graph.add(ids::COLORIZE, &f).unwrap();
    let out = graph.add(ids::OUTPUT, &f).unwrap();
    for (from, to) in [(a, b), (b, out)] {
        graph
            .connect(Connection {
                from,
                output: 0,
                to,
                input: 0,
            })
            .unwrap();
    }
    assert!(
        graph
            .remove_many(&[a, NodeId(999)].into_iter().collect())
            .is_err()
    );
    assert_eq!(graph.nodes().len(), 3);
    assert_eq!(graph.connection_count(), 2);
    graph.remove_many(&[a, b].into_iter().collect()).unwrap();
    assert_eq!(graph.nodes().len(), 1);
    assert_eq!(graph.nodes()[0].id, out);
    assert!(graph.connection_count() == 0);
    assert!(graph.topological_order().is_ok());
}
