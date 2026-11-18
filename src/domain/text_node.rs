use crate::{
    builtins::ImageNode,
    capabilities::Capabilities,
    execution::{ImageKernel, PreparedImage, PreparedParameter, TextInputBinding, TextKernel},
    node::{Node, NodeDescriptor, Parameter as P},
    port::Port,
    registry::NodeRegistry,
    schema::{NodeSchema, PropertyInfo},
    settings::NodeSettings,
};
struct TextNode;
static D: NodeDescriptor = NodeDescriptor {
    type_id: "Text",
    name: "Text",
    category: "Sources",
    inputs: &[
        Port::text("Text").optional(),
        Port::text("Font family").optional(),
    ],
    outputs: &[Port::texture("Image"), Port::text("Text")],
    capabilities: Capabilities::NONE,
    parameters: &[
        P::new("Red", 0.0, 1.0, 1.0),
        P::new("Green", 0.0, 1.0, 1.0),
        P::new("Blue", 0.0, 1.0, 1.0),
        P::new("Opacity", 0.0, 1.0, 1.0),
        P::new("Font size", 4.0, 512.0, 64.0),
        P::new("Font weight", 0.0, 8.0, 3.0).choices(&[
            "Thin",
            "Extra light",
            "Light",
            "Regular",
            "Medium",
            "Semibold",
            "Bold",
            "Extra bold",
            "Black",
        ]),
        P::new("Font style", 0.0, 2.0, 0.0).choices(&["Normal", "Italic", "Oblique"]),
        P::new("Alignment", 0.0, 2.0, 1.0).choices(&["Left", "Center", "Right"]),
    ],
};
impl Node for TextNode {
    fn descriptor(&self) -> &'static NodeDescriptor {
        &D
    }
    fn port_version(&self) -> u32 {
        2
    }
    fn migrate_input_port(&self, input: usize, from: u32) -> usize {
        input.saturating_add(match from {
            0 => 2,
            1 if input > 0 => 1,
            _ => 0,
        })
    }
    fn migrate_parameters_with_settings(&self, v: &mut Vec<f32>, s: &NodeSettings) {
        if v.len() == 4 {
            let size = s
                .properties
                .get("font.size")
                .and_then(|v| v.parse::<f32>().ok())
                .unwrap_or(64.0);
            let align = match s.properties.get("alignment").map(String::as_str) {
                Some("left") => 0.0,
                Some("right") => 2.0,
                _ => 1.0,
            };
            v.extend([size, 3.0, 0.0, align]);
        }
    }
    fn text(&self, s: &NodeSettings) -> Option<TextKernel> {
        Some(TextKernel {
            state: vec![std::sync::Arc::from(
                s.properties
                    .get("content.text")
                    .map_or("VJAY", String::as_str),
            )],
            evaluate: |ctx, state, out| {
                out[0] = ctx.inputs[0].clone().unwrap_or_else(|| state[0].clone())
            },
        })
    }
    fn default_settings(&self) -> NodeSettings {
        let mut s = NodeSettings {
            preview: true,
            ..Default::default()
        };
        s.properties.extend([
            ("content.text".into(), "VJAY".into()),
            ("font.family".into(), "sans-serif".into()),
        ]);
        s
    }
    fn schema(&self, _: &NodeSettings) -> NodeSchema {
        let mut s = NodeSchema::new(&D);
        s.properties = [
            ("content.text", "Text", true, &[] as &'static [&'static str]),
            ("font.family", "Font family", false, &[]),
        ]
        .into_iter()
        .map(|(key, label, multiline, choices)| PropertyInfo {
            key: key.into(),
            label: label.into(),
            multiline,
            path_picker: false,
            font_picker: key == "font.family",
            source_picker: None,
            choices,
        })
        .collect();
        s
    }
    fn validate_settings(&self, s: &NodeSettings) -> bool {
        s.is_valid()
            && s.properties.get("font.size").is_none_or(|v| {
                v.parse::<f32>()
                    .is_ok_and(|n| n.is_finite() && (4.0..=512.0).contains(&n))
            })
            && s.properties
                .get("alignment")
                .is_none_or(|v| matches!(v.as_str(), "left" | "center" | "right"))
    }
    fn image(&self, values: &[f32], s: &NodeSettings, output: usize) -> Option<ImageKernel> {
        let mut image = ImageNode {
            descriptor: &D,
            selector: 21.0,
        }
        .image(values, s, output)?;
        image.prepared = Some(PreparedImage {
            provider: "text".into(),
            properties: s.properties.clone(),
            text_inputs: vec![
                TextInputBinding {
                    input: 0,
                    property: "content.text".into(),
                },
                TextInputBinding {
                    input: 1,
                    property: "font.family".into(),
                },
            ],
            scalar_inputs: [
                (4, "font.size"),
                (5, "font.weight.index"),
                (6, "font.style.index"),
                (7, "alignment.index"),
            ]
            .into_iter()
            .map(|(parameter, property)| PreparedParameter {
                step: (parameter != 4).then_some(1),
                parameter,
                property: property.into(),
            })
            .collect(),
        });
        Some(image)
    }
}
pub(crate) fn register(r: &mut NodeRegistry) {
    r.register(TextNode).expect("valid text node");
}
