use super::*;
use crate::builtins::{ids, registry};

#[test]
fn outputs_own_fanout_and_registration_replaces_one_input_without_duplicates() {
    let nodes = registry();
    let mut graph = Graph::default();
    let source = graph.add(ids::LFO, &nodes).unwrap();
    let a = graph.add(ids::VIDEO, &nodes).unwrap();
    let b = graph.add(ids::VIDEO, &nodes).unwrap();
    let saw = graph.output(source, 2).unwrap();
    let sine = graph.output(source, 0).unwrap();
    let speed_a = graph.input(a, 0).unwrap();
    let position_b = graph.input(b, 1).unwrap();
    graph.register(saw, speed_a).unwrap();
    graph.register(saw, position_b).unwrap();
    graph.register(saw, speed_a).unwrap();
    assert_eq!(graph.destinations(saw).unwrap(), &[speed_a, position_b]);
    assert_eq!(graph.connection_count(), 2);
    graph.register(sine, speed_a).unwrap();
    assert_eq!(graph.destinations(saw).unwrap(), &[position_b]);
    assert_eq!(graph.destinations(sine).unwrap(), &[speed_a]);
    assert_eq!(graph.connection_count(), 2);
    graph.disconnect(b, 1);
    assert!(graph.destinations(saw).unwrap().is_empty());
    assert_eq!(graph.destinations(sine).unwrap(), &[speed_a]);
    assert_eq!(graph.connections().count(), graph.connection_count());
}

#[test]
fn cycle_rejection_restores_the_original_fanout_order_and_count() {
    let nodes = registry();
    let mut graph = Graph::default();
    let source = graph.add(ids::PATTERN, &nodes).unwrap();
    let a = graph.add(ids::COLORIZE, &nodes).unwrap();
    let b = graph.add(ids::COLORIZE, &nodes).unwrap();
    let c = graph.add(ids::COLORIZE, &nodes).unwrap();
    let output = graph.output(source, 0).unwrap();
    for node in [a, b, c] {
        graph
            .register(output, graph.input(node, 0).unwrap())
            .unwrap();
    }
    graph
        .register(graph.output(a, 0).unwrap(), graph.input(c, 0).unwrap())
        .unwrap();
    let before = graph.destinations(output).unwrap().to_vec();
    let wires = graph.connections().collect::<Vec<_>>();
    assert_eq!(
        graph.register(graph.output(c, 0).unwrap(), graph.input(a, 0).unwrap()),
        Err(GraphError::Cycle)
    );
    assert_eq!(graph.destinations(output).unwrap(), before);
    assert_eq!(graph.connections().collect::<Vec<_>>(), wires);
    assert_eq!(graph.connection_count(), 3);
}

#[test]
fn node_and_port_storage_stay_aligned_after_sparse_ids_swap_removal_and_batch_removal() {
    let nodes = registry();
    let mut graph = Graph::default();
    let unused = graph.add(ids::SLIDER, &nodes).unwrap();
    let source = graph.add(ids::LFO, &nodes).unwrap();
    let target = NodeId(1 << 60);
    graph
        .insert(
            NodeRecord {
                id: target,
                type_id: ids::VIDEO.into(),
                parameters: vec![1.0, 0.0],
                asset: None,
                settings: Default::default(),
            },
            &nodes,
        )
        .unwrap();
    let output = graph.output(source, 3).unwrap();
    let input = graph.input(target, 1).unwrap();
    graph.register(output, input).unwrap();
    graph.remove(unused).unwrap();
    assert_eq!(graph.node(target).unwrap().type_id, ids::VIDEO);
    assert_eq!(
        graph.ports()[graph.index(target).unwrap()].outputs().len(),
        8
    );
    assert_eq!(
        graph.ports()[graph.index(source).unwrap()].outputs().len(),
        4
    );
    assert_eq!(graph.destinations(output).unwrap(), &[input]);
    assert!(graph.topological_order().is_ok());
    graph.remove_many(&[target].into_iter().collect()).unwrap();
    assert!(graph.destinations(output).unwrap().is_empty());
    assert_eq!(graph.connection_count(), 0);
    assert_eq!(graph.nodes().len(), graph.ports().len());
}

#[test]
fn video_legacy_parameters_migrate_and_rotation_wraps() {
    use crate::{
        builtins::{ids, registry},
        graph::NodeRecord,
    };
    let factory = registry();
    let mut graph = super::Graph::default();
    let record = NodeRecord {
        id: super::NodeId(6),
        type_id: ids::VIDEO.into(),
        parameters: vec![1.0, 0.0],
        asset: None,
        settings: Default::default(),
    };
    graph.insert(record, &factory).unwrap();
    assert_eq!(
        graph.node(super::NodeId(6)).unwrap().parameters,
        vec![1.0, 0.0, 1.0, 0.0, 1.0, 0.0]
    );
    assert!(
        graph
            .set_parameter(super::NodeId(6), 3, 1.0, &factory)
            .is_err()
    );
    let transform = graph.add(ids::TRANSFORM, &factory).unwrap();
    graph.set_parameter(transform, 3, 1e20, &factory).unwrap();
    assert!((-180.0..180.0).contains(&graph.node(transform).unwrap().parameters[3]));
    assert!(
        graph
            .set_parameter(transform, 3, f32::INFINITY, &factory)
            .is_err()
    );
}
