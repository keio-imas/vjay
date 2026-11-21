use vjay_application::{ports::PortResult, project::*};
use vjay_domain::{builtins::ids, graph::*, node::NodeFactory};
pub fn project(factory: &dyn NodeFactory) -> PortResult<Project> {
    let mut graph = Graph::default();
    let a = graph
        .add(ids::PATTERN, factory)
        .map_err(|e| e.to_string())?;
    let b = graph
        .add(ids::PATTERN, factory)
        .map_err(|e| e.to_string())?;
    graph
        .set_parameter(b, 0, 13.0, factory)
        .map_err(|e| e.to_string())?;
    graph
        .set_parameter(b, 2, 125.0, factory)
        .map_err(|e| e.to_string())?;
    let mix = graph.add(ids::MIX, factory).map_err(|e| e.to_string())?;
    let color = graph
        .add(ids::COLORIZE, factory)
        .map_err(|e| e.to_string())?;
    let output = graph.add(ids::OUTPUT, factory).map_err(|e| e.to_string())?;
    for (from, to, input) in [
        (a, mix, 0),
        (b, mix, 1),
        (mix, color, 0),
        (color, output, 0),
    ] {
        graph
            .connect(Connection {
                output: 0,
                from,
                to,
                input,
            })
            .map_err(|e| e.to_string())?;
    }
    let positions = [
        (a, 30.0, 80.0),
        (b, 30.0, 240.0),
        (mix, 260.0, 140.0),
        (color, 490.0, 140.0),
        (output, 720.0, 140.0),
    ]
    .map(|(node, x, y)| NodePosition {
        node,
        position: Point { x, y },
    })
    .to_vec();
    let controls = [(mix, 0), (color, 0), (a, 1)]
        .into_iter()
        .enumerate()
        .map(|(id, (node, parameter))| ControlBinding {
            id: id as u64,
            node,
            parameter,
            position: Point {
                x: 12.0,
                y: 12.0 + id as f32 * 108.0,
            },
            size: Point { x: 240.0, y: 100.0 },
            ..Default::default()
        })
        .collect();
    Ok(Project {
        graph,
        output: Some(output),
        positions,
        controls,
        assets: Vec::new(),
        workspace: Default::default(),
        subpatches: Vec::new(),
    })
}
