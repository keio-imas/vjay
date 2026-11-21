use super::*;
use vjay_application::{
    control_io::ControlEvent,
    project::{ControlBinding, ControlTarget, WidgetKind, valid_control},
    signals::SignalEngine,
};
use vjay_domain::{
    builtins::{ids, registry},
    graph::{Connection, Graph},
    node::NodeFactory,
};
struct TextPass;
static PASS: NodeDescriptor = NodeDescriptor {
    type_id: "TestTextPass",
    name: "Pass",
    category: "Tests",
    inputs: &[Port::text("Text")],
    outputs: &[Port::text("Text")],
    parameters: &[],
    capabilities: Capabilities::NONE,
};
impl Node for TextPass {
    fn descriptor(&self) -> &'static NodeDescriptor {
        &PASS
    }
    fn text(&self, _: &NodeSettings) -> Option<TextKernel> {
        Some(TextKernel {
            evaluate: |context, _, output| {
                output[0] = context.inputs[0]
                    .clone()
                    .unwrap_or_else(vjay_domain::execution::empty_text)
            },
            state: vec![],
        })
    }
}
#[test]
fn registered_service_metadata_routes_as_text_and_pins_to_labels() {
    let mut factory = registry();
    factory.register(RekordboxHistoryNode).unwrap();
    factory.register(TextPass).unwrap();
    let mut graph = Graph::default();
    let source = graph.add(REKORDBOX.type_id, &factory).unwrap();
    let text = graph.add(PASS.type_id, &factory).unwrap();
    let number = graph.add(ids::CONSTANT, &factory).unwrap();
    graph
        .connect(Connection {
            from: source,
            output: 4,
            to: text,
            input: 0,
        })
        .unwrap();
    assert!(
        graph
            .connect(Connection {
                from: source,
                output: 4,
                to: number,
                input: 0
            })
            .is_err()
    );
    graph
        .connect(Connection {
            from: source,
            output: 0,
            to: number,
            input: 0,
        })
        .unwrap();
    let mut signals = SignalEngine::default();
    signals.rebuild(&graph, &factory).unwrap();
    let mut values = [0.0; 16];
    values[0] = 128.5;
    signals.events.push(ControlEvent::NodeValues {
        node: source,
        values,
    });
    signals.events.push(ControlEvent::NodeText {
        node: source,
        values: vec![std::sync::Arc::from("曲名 — live")],
    });
    signals.tick(0.0);
    assert_eq!(signals.text_output(text, 0), Some("曲名 — live"));
    assert_eq!(signals.value(number, 0), Some(128.5));
    let mut binding = ControlBinding {
        node: source,
        parameter: 4,
        target: ControlTarget::Output,
        widget: WidgetKind::Label,
        ..Default::default()
    };
    assert!(valid_control(
        &binding,
        graph.node(source).unwrap(),
        &factory
    ));
    binding.widget = WidgetKind::Meter;
    assert!(!valid_control(
        &binding,
        graph.node(source).unwrap(),
        &factory
    ));
    assert_eq!(
        factory.get(REKORDBOX.type_id).unwrap().service(),
        Some("rekordbox-history")
    );
}
