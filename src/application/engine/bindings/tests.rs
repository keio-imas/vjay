use super::*;
use vjay_domain::{
    builtins::{ids, registry},
    graph::{NodeId, NodeRecord},
};

#[test]
fn sparse_ids_resolve_to_dense_slots_and_input_indices_cannot_bleed_into_the_next_node() {
    let nodes = registry();
    let mut graph = Graph::default();
    let source = graph.add(ids::LFO, &nodes).unwrap();
    let first = graph.add(ids::VIDEO, &nodes).unwrap();
    let last = NodeId(1 << 60);
    graph
        .insert(
            NodeRecord {
                id: last,
                type_id: ids::VIDEO.into(),
                parameters: vec![1.0, 0.0],
                asset: None,
                settings: Default::default(),
            },
            &nodes,
        )
        .unwrap();
    let output = graph.output(source, 3).unwrap();
    graph
        .register(output, graph.input(first, 1).unwrap())
        .unwrap();
    graph
        .register(output, graph.input(last, 0).unwrap())
        .unwrap();
    let bindings = PortBindings::new(&graph).unwrap();
    let source_ref = Some(OutputRef {
        node: graph.index(source).unwrap(),
        port: 3,
    });
    assert_eq!(bindings.source(graph.index(first).unwrap(), 1), source_ref);
    assert_eq!(bindings.source(graph.index(last).unwrap(), 0), source_ref);
    assert_eq!(bindings.source(graph.index(first).unwrap(), 2), None);
    assert_eq!(
        bindings.output(OutputRef {
            node: graph.index(first).unwrap(),
            port: 0
        }),
        4
    );
    assert_eq!(
        bindings.output(OutputRef {
            node: graph.index(last).unwrap(),
            port: 0
        }),
        12
    );
}

#[test]
fn empty_graph_has_no_input_bindings_or_output_slots() {
    let bindings = PortBindings::new(&Graph::default()).unwrap();
    assert_eq!(bindings.output_count(), 0);
    assert_eq!(bindings.source(0, 0), None);
}
