//! CPU-only comparison of graph editing, planning and steady signal evaluation.
//! Run with `cargo run --release --example connection_bench`.
use std::{hint::black_box, time::Instant};
use vjay_application::{
    audio::AudioPlan, compiler::compile, media_timeline::MediaTimeline, signals::SignalEngine,
};
use vjay_domain::{
    builtins::{ids, registry},
    graph::{Connection, Graph, NodeId},
    node::{AssetId, NodeFactory},
};

fn connect(graph: &mut Graph, from: NodeId, to: NodeId, input: usize, _nodes: &dyn NodeFactory) {
    graph
        .connect(Connection {
            from,
            output: 0,
            to,
            input,
        })
        .unwrap();
}
fn image_patch(
    images: usize,
    scalars: usize,
    nodes: &dyn NodeFactory,
) -> (Graph, NodeId, Connection, Connection) {
    let mut graph = Graph::default();
    let seed = graph.add(ids::CONSTANT, nodes).unwrap();
    let first = graph.add(ids::PATTERN, nodes).unwrap();
    let mut previous = first;
    let mut effect = first;
    for _ in 1..images {
        effect = graph.add(ids::COLORIZE, nodes).unwrap();
        connect(&mut graph, previous, effect, 0, nodes);
        connect(&mut graph, seed, effect, 1, nodes);
        previous = effect;
    }
    let output = graph.add(ids::OUTPUT, nodes).unwrap();
    connect(&mut graph, previous, output, 0, nodes);
    let mut scalar = seed;
    for _ in 1..scalars {
        let next = graph.add(ids::ADD, nodes).unwrap();
        connect(&mut graph, scalar, next, 0, nodes);
        connect(&mut graph, seed, next, 1, nodes);
        scalar = next;
    }
    connect(&mut graph, scalar, effect, 3, nodes);
    let a = Connection {
        from: seed,
        output: 0,
        to: effect,
        input: 1,
    };
    let b = Connection { from: scalar, ..a };
    (graph, output, a, b)
}
fn audio_patch(nodes: &dyn NodeFactory) -> Graph {
    let mut graph = Graph::default();
    let seed = graph.add(ids::CONSTANT, nodes).unwrap();
    let mut previous = graph.add(ids::VIDEO, nodes).unwrap();
    graph.bind_asset(previous, AssetId(0), nodes).unwrap();
    for index in 0..64 {
        let gain = graph.add(ids::AUDIO_GAIN, nodes).unwrap();
        graph
            .connect(Connection {
                from: previous,
                output: usize::from(index == 0),
                to: gain,
                input: 0,
            })
            .unwrap();
        connect(&mut graph, seed, gain, 1, nodes);
        previous = gain;
    }
    let output = graph.add(ids::AUDIO_OUT, nodes).unwrap();
    connect(&mut graph, previous, output, 0, nodes);
    graph
}
fn measure(name: &str, operation: &str, graph: &Graph, iterations: usize, mut run: impl FnMut()) {
    for _ in 0..20 {
        run();
    }
    let mut samples = [0.0_f64; 9];
    for sample in &mut samples {
        let start = Instant::now();
        for _ in 0..iterations {
            run();
        }
        *sample = start.elapsed().as_secs_f64() * 1e6 / iterations as f64;
    }
    samples.sort_by(f64::total_cmp);
    println!(
        "{name},{operation},{},{},{:.3},{:.3},{:.3}",
        graph.nodes().len(),
        graph.connection_count(),
        samples[4],
        samples[0],
        samples[8]
    );
}
fn main() {
    let nodes = registry();
    println!("fixture,operation,nodes,connections,median_us,min_us,max_us");
    for (name, images, scalars) in [("mixed-48", 24, 23), ("mixed-256", 160, 95)] {
        let (graph, output, a, b) = image_patch(images, scalars, &nodes);
        measure(name, "image-plan", &graph, 300, || {
            black_box(compile(black_box(&graph), output, &nodes).unwrap());
        });
        let mut signals = SignalEngine::default();
        measure(name, "signal-plan", &graph, 300, || {
            signals.rebuild(black_box(&graph), &nodes).unwrap();
        });
        let mut plan = compile(&graph, output, &nodes).unwrap();
        signals.bind(&mut plan);
        let mut seconds = 0.0;
        measure(name, "signal-tick", &graph, 10_000, || {
            seconds += 1.0 / 60.0;
            signals.tick(black_box(seconds));
            signals.modulate(black_box(&mut plan));
            black_box(&signals.values);
        });
        let mut edited = graph.clone();
        let mut alternate = false;
        measure(name, "replace-wire", &graph, 300, || {
            alternate = !alternate;
            edited
                .connect(black_box(if alternate { a } else { b }))
                .unwrap();
        });
        let mut moving = graph.clone();
        let mut previous = a;
        measure(name, "reconnect-wire", &graph, 300, || {
            let next = if previous == a { b } else { a };
            moving.reconnect(previous, next).unwrap();
            previous = next;
        });
    }
    let graph = audio_patch(&nodes);
    let mut signals = SignalEngine::default();
    signals.rebuild(&graph, &nodes).unwrap();
    let mut timeline = MediaTimeline::default();
    timeline.rebuild(&graph, &signals).unwrap();
    let mut plan = AudioPlan::compile(&graph, &signals).unwrap();
    measure("audio-67", "audio-plan", &graph, 300, || {
        black_box(AudioPlan::compile(black_box(&graph), &signals).unwrap());
    });
    measure("audio-67", "audio-update", &graph, 10_000, || {
        plan.update(black_box(&signals), &timeline);
        black_box(&plan);
    });
}
