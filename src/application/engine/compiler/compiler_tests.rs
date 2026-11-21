use super::*;
use crate::project::Project;
use vjay_domain::builtins::ids;
use vjay_domain::{builtins::registry as builtin_registry, graph::Connection};

#[test]
fn demo_eliminates_output_copy_and_reuses_textures_without_read_write_aliasing() {
    let f = builtin_registry();
    let project = Project::demo(&f).unwrap();
    let plan = compile(&project.graph, project.output, &f).unwrap();
    assert_eq!(plan.steps.len(), 4);
    assert_eq!(plan.texture_count, 3);
    for step in &plan.steps {
        assert!(
            step.inputs
                .iter()
                .flatten()
                .all(|&input| input != step.target)
        );
        assert_ne!(Some(step.node), project.output);
    }
    assert_eq!(plan.output_slot, plan.steps.last().unwrap().target);
}

#[test]
fn incomplete_dead_nodes_are_culled_but_live_missing_inputs_fail() {
    let f = builtin_registry();
    let mut project = Project::demo(&f).unwrap();
    let incomplete = project.graph.add(ids::MIX, &f).unwrap();
    assert_eq!(
        compile(&project.graph, project.output, &f)
            .unwrap()
            .steps
            .len(),
        4
    );
    project.graph.disconnect(project.output.unwrap(), 0);
    assert!(
        compile(&project.graph, project.output, &f)
            .unwrap()
            .steps
            .is_empty()
    );
    project
        .graph
        .connect(Connection {
            from: incomplete,
            output: 0,
            to: project.output.unwrap(),
            input: 0,
        })
        .unwrap();
    assert!(compile(&project.graph, project.output, &f).is_err());
}

#[test]
fn fanout_preserves_producer_until_its_last_consumer() {
    let f = builtin_registry();
    let mut graph = Graph::default();
    let pattern = graph.add(ids::PATTERN, &f).unwrap();
    let effect = graph.add(ids::COLORIZE, &f).unwrap();
    let mix = graph.add(ids::MIX, &f).unwrap();
    let output = graph.add(ids::OUTPUT, &f).unwrap();
    for (from, to, input) in [
        (pattern, effect, 0),
        (pattern, mix, 0),
        (effect, mix, 1),
        (mix, output, 0),
    ] {
        graph
            .connect(Connection {
                output: 0,
                from,
                to,
                input,
            })
            .unwrap();
    }
    let plan = compile(&graph, output, &f).unwrap();
    assert_eq!(plan.texture_count, 3);
    assert_eq!(plan.steps[2].inputs[0], Some(plan.steps[0].target));
    assert_eq!(plan.steps[2].inputs[1], Some(plan.steps[1].target));
    assert_ne!(plan.steps[0].target, plan.steps[1].target);
}

#[test]
fn repeated_input_is_released_once() {
    let f = builtin_registry();
    let mut g = Graph::default();
    let a = g.add(ids::PATTERN, &f).unwrap();
    let b = g.add(ids::MIX, &f).unwrap();
    let c = g.add(ids::COLORIZE, &f).unwrap();
    let d = g.add(ids::COLORIZE, &f).unwrap();
    let o = g.add(ids::OUTPUT, &f).unwrap();
    for (from, to, input) in [(a, b, 0), (a, b, 1), (b, c, 0), (c, d, 0), (d, o, 0)] {
        g.connect(Connection {
            output: 0,
            from,
            to,
            input,
        })
        .unwrap();
    }
    let p = compile(&g, o, &f).unwrap();
    assert_eq!(p.texture_count, 2);
    for step in p.steps {
        assert!(!step.inputs.contains(&Some(step.target)));
    }
}

#[test]
fn branched_dags_never_reuse_a_texture_before_its_last_read() {
    let f = builtin_registry();
    for seed in 1..65_u64 {
        let mut rng = seed;
        let mut next = || {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            (rng >> 32) as usize
        };
        let mut g = Graph::default();
        let mut ids = vec![g.add(ids::PATTERN, &f).unwrap()];
        for _ in 1..32 {
            let kind = if next() % 2 == 0 {
                ids::MIX
            } else {
                ids::COLORIZE
            };
            let node = g.add(kind, &f).unwrap();
            for input in 0..f.get(kind).unwrap().descriptor().inputs.len() {
                let from = ids[next() % ids.len()];
                g.connect(Connection {
                    output: 0,
                    from,
                    to: node,
                    input,
                })
                .unwrap();
            }
            ids.push(node);
        }
        let output = g.add(ids::OUTPUT, &f).unwrap();
        let final_source = *ids.last().unwrap();
        g.connect(Connection {
            output: 0,
            from: final_source,
            to: output,
            input: 0,
        })
        .unwrap();
        let plan = compile(&g, output, &f).unwrap();
        let mut contents = vec![None; plan.texture_count];
        for step in &plan.steps {
            for edge in g.connections().filter(|e| e.to == step.node) {
                let slot = step.inputs[edge.input].unwrap();
                assert_eq!(
                    contents[slot],
                    Some(edge.from),
                    "seed {seed}, node {:?}",
                    step.node
                );
                assert_ne!(slot, step.target);
            }
            contents[step.target] = Some(step.node);
        }
        assert_eq!(contents[plan.output_slot], Some(final_source));
    }
}
