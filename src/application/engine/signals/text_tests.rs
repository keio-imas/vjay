use super::*;
use vjay_domain::graph::Graph;
use vjay_domain::{
    builtins::{ids, registry},
    graph::Connection,
};
#[test]
fn text_inputs_keep_fallback_and_connected_empty_distinct_and_only_invalidate_on_change() {
    let f = registry();
    let mut g = Graph::default();
    let value = g.add("TextValue", &f).unwrap();
    let text = g.add("Text", &f).unwrap();
    let output = g.add(ids::OUTPUT, &f).unwrap();
    g.connect(Connection {
        from: text,
        output: 0,
        to: output,
        input: 0,
    })
    .unwrap();
    let mut s = SignalEngine::default();
    s.rebuild(&g, &f).unwrap();
    s.tick(0.0);
    assert_eq!(s.text_output(text, 1), Some("VJAY"));
    g.connect(Connection {
        from: value,
        output: 0,
        to: text,
        input: 0,
    })
    .unwrap();
    let mut settings = g.node(value).unwrap().settings.clone();
    settings
        .properties
        .insert("content.text".into(), "曲名 / Album".into());
    g.set_settings(value, settings.clone(), &f).unwrap();
    s.rebuild(&g, &f).unwrap();
    let mut plan = crate::compiler::compile(&g, output, &f).unwrap();
    s.bind(&mut plan);
    s.tick(0.1);
    s.modulate(&mut plan);
    let step = plan.steps.iter().find(|p| p.node == text).unwrap();
    assert_eq!(
        step.operation.prepared.as_ref().unwrap().properties["content.text"],
        "曲名 / Album"
    );
    let revision = step.prepared_revision;
    for i in 2..20 {
        s.tick(i as f64 / 10.0);
        s.modulate(&mut plan);
    }
    assert_eq!(plan.steps[0].prepared_revision, revision);
    settings
        .properties
        .insert("content.text".into(), String::new());
    g.set_settings(value, settings, &f).unwrap();
    s.rebuild(&g, &f).unwrap();
    s.bind(&mut plan);
    s.tick(2.0);
    s.modulate(&mut plan);
    assert_eq!(s.text_output(text, 1), Some(""));
    assert!(plan.steps[0].prepared_revision > revision);
    assert_eq!(
        plan.steps[0]
            .operation
            .prepared
            .as_ref()
            .unwrap()
            .properties["content.text"],
        ""
    );
    g.disconnect(text, 0);
    s.rebuild(&g, &f).unwrap();
    s.tick(2.1);
    assert_eq!(s.text_output(text, 1), Some("VJAY"));
}
