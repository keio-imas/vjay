use super::*;
use vjay_domain::{graph::Connection, uniform::ShaderUniform};
#[test]
fn copying_a_group_keeps_internal_wires_assets_settings_and_original_program() {
    let (mut session, _, _) = fixture(Project::demo(&builtin_registry()).unwrap().document());
    let video = session
        .import_video("/fixture/clip.mov".into(), None)
        .unwrap();
    let grade = session
        .add_node(ids::GRADE, Point { x: 500.0, y: 400.0 })
        .unwrap();
    session
        .connect(Connection {
            from: video,
            output: 0,
            to: grade,
            input: 0,
        })
        .unwrap();
    let mut settings = session
        .project()
        .graph
        .node(video)
        .unwrap()
        .settings
        .clone();
    settings.label = "Clip A".into();
    session.set_settings(video, settings).unwrap();
    let program = session.project().output;
    let edges = session.project().graph.connection_count();
    let fragment = session.copy_nodes(&[video, grade]).unwrap();
    let result = session
        .paste_nodes(&fragment, Point { x: 600.0, y: 500.0 })
        .unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(session.project().graph.connection_count(), edges + 1);
    assert_eq!(session.project().output, program);
    let copy = session.project().graph.node(result[0]).unwrap();
    assert_eq!(copy.settings.label, "Clip A");
    assert_eq!(
        copy.asset,
        session.project().graph.node(video).unwrap().asset
    );
    assert_eq!(session.project().assets.len(), 1);
    assert!(
        session
            .project()
            .graph
            .connections()
            .any(|c| c.from == result[0] && c.to == result[1])
    );
    let mut invalid = fragment.clone();
    invalid.connections.push(Connection {
        from: NodeId(999999),
        output: 0,
        to: grade,
        input: 0,
    });
    let count = session.project().graph.nodes().len();
    assert!(
        session
            .paste_nodes(&invalid, Point { x: 0.0, y: 0.0 })
            .is_err()
    );
    assert_eq!(count, session.project().graph.nodes().len());
    Project::restore(session.project().document(), &builtin_registry()).unwrap();
}
#[test]
fn uniform_removal_remaps_wires_controls_and_saved_parameter_values() {
    let (mut session, _, _) = fixture(Project::demo(&builtin_registry()).unwrap().document());
    let shader = session
        .add_node(ids::GLSL, Point { x: 0.0, y: 0.0 })
        .unwrap();
    let uniforms: Vec<_> = (0..8)
        .map(|i| ShaderUniform {
            name: format!("u_{i}"),
            min: -10.0,
            max: 10.0,
            default: 0.0,
        })
        .collect();
    session
        .set_shader_uniforms(shader, uniforms.clone())
        .unwrap();
    session.set_parameter(shader, 7, 0.75).unwrap();
    session.pin_control(shader, 7).unwrap();
    let value = session
        .add_node(ids::CONSTANT, Point { x: 0.0, y: 0.0 })
        .unwrap();
    session
        .connect(Connection {
            from: value,
            output: 0,
            to: shader,
            input: 9,
        })
        .unwrap();
    let mut uniforms = uniforms;
    uniforms.remove(2);
    session.set_shader_uniforms(shader, uniforms).unwrap();
    assert_eq!(session.schema(shader).parameters.len(), 7);
    assert_eq!(
        session.project().graph.node(shader).unwrap().parameters[6],
        0.75
    );
    assert!(
        session
            .project()
            .graph
            .connections()
            .any(|e| e.to == shader && e.input == 8)
    );
    assert_eq!(session.project().controls.last().unwrap().parameter, 6);
    session.render_at(0.0).unwrap();
    Project::restore(session.project().document(), &builtin_registry()).unwrap();
    let mut renamed = session.shader_uniforms(shader).unwrap();
    renamed[6].name = "opacity".into();
    session.set_shader_uniforms(shader, renamed).unwrap();
    assert!(
        session
            .project()
            .graph
            .connections()
            .any(|e| e.to == shader && e.input == 8)
    );
    session.set_shader_uniforms(shader, Vec::new()).unwrap();
    assert!(session.schema(shader).parameters.is_empty());
    assert!(
        session
            .project()
            .graph
            .connections()
            .all(|e| e.to != shader)
    );
    assert!(session.project().controls.iter().all(|c| c.node != shader));
}

#[test]
fn renaming_a_used_uniform_updates_the_shader_without_a_broken_intermediate_state() {
    let (mut session, _, _) = fixture(Project::demo(&builtin_registry()).unwrap().document());
    let id = session
        .add_node(ids::GLSL, Point { x: 0.0, y: 0.0 })
        .unwrap();
    let uniforms = vec![ShaderUniform {
        name: "gain".into(),
        min: 0.0,
        max: 1.0,
        default: 0.5,
    }];
    session.set_shader_uniforms(id, uniforms.clone()).unwrap();
    let mut settings = session.project().graph.node(id).unwrap().settings.clone();
    settings.shader = "void mainImage(out vec4 c,in vec2 p){c=vec4(gain);}".into();
    session.set_settings(id, settings).unwrap();
    let mut renamed = uniforms;
    renamed[0].name = "opacity".into();
    session.set_shader_uniforms(id, renamed).unwrap();
    assert!(
        session
            .project()
            .graph
            .node(id)
            .unwrap()
            .settings
            .shader
            .contains("vec4(opacity)")
    );
}

#[test]
fn copying_current_and_legacy_text_ports_keeps_parameter_connections() {
    let (mut session, _, _) = fixture(Project::demo(&builtin_registry()).unwrap().document());
    let value = session
        .add_node(ids::CONSTANT, Point { x: 0.0, y: 0.0 })
        .unwrap();
    let text = session
        .add_node("Text", Point { x: 250.0, y: 0.0 })
        .unwrap();
    session
        .connect(Connection {
            from: value,
            output: 0,
            to: text,
            input: 2,
        })
        .unwrap();
    let current = session.copy_nodes(&[value, text]).unwrap();
    let pasted = session
        .paste_nodes(&current, Point { x: 0.0, y: 400.0 })
        .unwrap();
    assert!(
        session
            .project()
            .graph
            .connections()
            .any(|e| e.from == pasted[0] && e.to == pasted[1] && e.input == 2)
    );
    let mut legacy = current;
    legacy
        .nodes
        .iter_mut()
        .find(|n| n.id == text)
        .unwrap()
        .settings
        .port_version = 0;
    legacy.connections[0].input = 0;
    let pasted = session
        .paste_nodes(&legacy, Point { x: 0.0, y: 700.0 })
        .unwrap();
    assert!(
        session
            .project()
            .graph
            .connections()
            .any(|e| e.from == pasted[0] && e.to == pasted[1] && e.input == 2)
    );
}
