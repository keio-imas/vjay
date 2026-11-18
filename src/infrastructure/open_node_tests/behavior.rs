use super::*;
use crate::{
    gpu::{WgpuRuntimeFactory, programs::ShaderPrograms},
    media::FfmpegDecoderFactory,
    storage::JsonProjectStore,
};
use std::sync::Arc;
use vjay_application::{
    compiler::compile, control_io::ControlEvent, ports::*, project::*, session::Session,
    signals::SignalEngine,
};
use vjay_domain::{
    builtins::{ids, registry},
    graph::Connection,
};

#[test]
fn extension_registers_executes_six_scalar_outputs_and_restores_without_builtin_dispatch() {
    let mut nodes = registry();
    nodes.register(Extension).unwrap();
    assert!(nodes.register(Extension).is_err());
    let mut project = Project::empty(&nodes).unwrap();
    let source = project.graph.add(TYPE_ID, &nodes).unwrap();
    let color = project.graph.add(ids::SOLID, &nodes).unwrap();
    for node in [source, color] {
        project.positions.push(NodePosition {
            node,
            position: Point { x: 30.0, y: 50.0 },
        });
    }
    for edge in [
        Connection {
            from: source,
            output: 6,
            to: color,
            input: 0,
        },
        Connection {
            from: color,
            output: 0,
            to: project.output.unwrap(),
            input: 0,
        },
    ] {
        project.graph.connect(edge).unwrap();
    }
    project.controls.push(ControlBinding {
        node: source,
        parameter: 6,
        target: ControlTarget::Output,
        widget: WidgetKind::Meter,
        ..Default::default()
    });
    let mut signals = SignalEngine::default();
    signals.rebuild(&project.graph, &nodes).unwrap();
    signals.events.push(ControlEvent::External {
        channel: "test.clock".into(),
        values: vec![0.0, 0.0, 0.1, 0.2, 0.3, 0.75],
    });
    signals.tick(0.0);
    signals.tick(0.25);
    assert_eq!(signals.output(source, 1), Some(0.25));
    assert_eq!(signals.output(source, 2), Some(0.25));
    assert_eq!(signals.output(source, 6), Some(0.75));
    let mut plan = compile(&project.graph, project.output, &nodes).unwrap();
    signals.bind(&mut plan);
    signals.modulate(&mut plan);
    assert_eq!(plan.steps[0].operation.parameters[0], 0.75);
    signals.rebuild(&project.graph, &nodes).unwrap();
    signals.tick(0.5);
    assert_eq!(signals.output(source, 1), Some(0.5));
    let json = serde_json::to_string(&project.document()).unwrap();
    assert!(json.contains(TYPE_ID));
    let document = serde_json::from_str(&json).unwrap();
    let restored = Project::restore(document, &nodes).unwrap();
    assert_eq!(
        restored.graph.node(source).unwrap().settings.properties["channel"],
        "test.clock"
    );
    assert_eq!(restored.controls[0].parameter, 6);
    let error = Project::restore(restored.document(), &registry())
        .err()
        .expect("unregistered extension must fail");
    assert!(error.contains(TYPE_ID));
    let mut invalid = restored.graph.node(source).unwrap().settings.clone();
    invalid.properties.clear();
    assert!(project.graph.set_settings(source, invalid, &nodes).is_err());
    assert_eq!(
        project.graph.node(source).unwrap().settings.properties["channel"],
        "test.clock"
    );
}

#[test]
#[ignore = "requires a native GPU; verifies a separately registered extension shader"]
fn extension_shader_and_parameter_selected_programs_render_without_renderer_branches() {
    let mut nodes = registry();
    nodes.register(Extension).unwrap();
    let source = "struct Frame { viewport: vec4<f32>, parameters: vec4<f32> };\n@group(0) @binding(0) var<uniform> frame: Frame;\n@fragment fn color() -> @location(0) vec4<f32> {return vec4<f32>(frame.parameters.x,frame.parameters.y,frame.parameters.z,1.0);}";
    let mut programs = ShaderPrograms::default();
    programs.register(PROGRAM, source, "color").unwrap();
    programs
        .register(
            ALTERNATE,
            &source.replace("frame.parameters.x", "(1.0-frame.parameters.x)"),
            "color",
        )
        .unwrap();
    let runtime = WgpuRuntimeFactory::headless(
        Arc::new(FfmpegDecoderFactory::new("ffmpeg".into(), "ffprobe".into())),
        Arc::new(programs),
    )
    .unwrap();
    let mut session = Session::new(
        Box::new(nodes),
        &runtime,
        Box::new(JsonProjectStore::new(
            "/tmp/vjay-open-extension-unused.json".into(),
        )),
        Extent::new(8, 8).unwrap(),
    )
    .unwrap();
    let output = session.project().output.unwrap();
    let node = session.add_node(TYPE_ID, Point { x: 0.0, y: 0.0 }).unwrap();
    session
        .connect(Connection {
            from: node,
            output: 0,
            to: output,
            input: 0,
        })
        .unwrap();
    for level in [0.25, 0.75] {
        session.set_parameter(node, 0, level).unwrap();
        session.render_at(0.0).unwrap();
        let frame = session.capture().unwrap();
        for (actual, expected) in frame.rgba[..3].iter().zip([64_u8, 128, 191]) {
            assert!(actual.abs_diff(expected) <= 1, "{actual} != {expected}");
        }
    }
}
