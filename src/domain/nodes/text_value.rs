use crate::{
    capabilities::Capabilities,
    execution::TextKernel,
    node::{Node, NodeDescriptor},
    port::Port,
    registry::NodeRegistry,
    schema::{NodeSchema, PropertyInfo},
    settings::NodeSettings,
};
struct TextValue;
static D: NodeDescriptor = NodeDescriptor {
    type_id: "TextValue",
    name: "Text Value",
    category: "Controls",
    inputs: &[],
    outputs: &[Port::text("Text")],
    parameters: &[],
    capabilities: Capabilities::NONE,
};
impl Node for TextValue {
    fn descriptor(&self) -> &'static NodeDescriptor {
        &D
    }
    fn default_settings(&self) -> NodeSettings {
        let mut s = NodeSettings::default();
        s.properties.insert("content.text".into(), "Text".into());
        s
    }
    fn schema(&self, _: &NodeSettings) -> NodeSchema {
        let mut s = NodeSchema::new(&D);
        s.properties.push(PropertyInfo {
            key: "content.text".into(),
            label: "Text".into(),
            multiline: true,
            path_picker: false,
            font_picker: false,
            source_picker: None,
            choices: &[],
        });
        s
    }
    fn text(&self, s: &NodeSettings) -> Option<TextKernel> {
        Some(TextKernel {
            state: vec![std::sync::Arc::from(
                s.properties.get("content.text").map_or("", String::as_str),
            )],
            evaluate: |_, state, out| out[0] = state[0].clone(),
        })
    }
}
pub(crate) fn register(r: &mut NodeRegistry) {
    r.register(TextValue).expect("valid text value node");
}
