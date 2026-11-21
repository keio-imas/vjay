use crate::{
    builtins::ImageNode,
    capabilities::Capabilities,
    execution::{ImageKernel, StreamRequest},
    node::{Node, NodeDescriptor},
    port::Port,
    registry::NodeRegistry,
    schema::{NodeSchema, PropertyInfo},
    settings::NodeSettings,
};
struct Input {
    d: &'static NodeDescriptor,
    provider: &'static str,
}
struct Output {
    d: &'static NodeDescriptor,
    provider: &'static str,
}
fn settings(input: bool) -> NodeSettings {
    let mut s = NodeSettings {
        preview: input,
        ..Default::default()
    };
    s.properties
        .insert(if input { "source" } else { "name" }.into(), String::new());
    if !input {
        s.properties.insert("enabled".into(), "Off".into());
    }
    s
}
fn schema(d: &'static NodeDescriptor, input: bool, provider: &'static str) -> NodeSchema {
    let mut s = NodeSchema::new(d);
    s.properties.push(PropertyInfo {
        key: if input { "source" } else { "name" }.into(),
        label: if input { "Source" } else { "Sender name" }.into(),
        multiline: false,
        path_picker: false,
        font_picker: false,
        source_picker: input.then_some(provider),
        choices: &[],
    });
    if !input {
        s.properties.push(PropertyInfo {
            key: "enabled".into(),
            label: "Send".into(),
            multiline: false,
            path_picker: false,
            font_picker: false,
            source_picker: None,
            choices: &["Off", "On"],
        });
    }
    s
}
impl Node for Input {
    fn descriptor(&self) -> &'static NodeDescriptor {
        self.d
    }
    fn default_settings(&self) -> NodeSettings {
        settings(true)
    }
    fn schema(&self, _: &NodeSettings) -> NodeSchema {
        schema(self.d, true, self.provider)
    }
    fn image(&self, v: &[f32], s: &NodeSettings, o: usize) -> Option<ImageKernel> {
        let mut k = ImageNode {
            descriptor: self.d,
            selector: 20.0,
        }
        .image(v, s, o)?;
        k.stream = Some(StreamRequest {
            provider: self.provider,
            name: s.properties.get("source").cloned().unwrap_or_default(),
            options: s.properties.clone(),
        });
        Some(k)
    }
}
impl Node for Output {
    fn descriptor(&self) -> &'static NodeDescriptor {
        self.d
    }
    fn default_settings(&self) -> NodeSettings {
        settings(false)
    }
    fn schema(&self, _: &NodeSettings) -> NodeSchema {
        schema(self.d, false, self.provider)
    }
    fn video_sink(&self, s: &NodeSettings) -> Option<StreamRequest> {
        Some(StreamRequest {
            provider: self.provider,
            name: s
                .properties
                .get("name")
                .cloned()
                .unwrap_or_else(|| "VJAY".into()),
            options: s.properties.clone(),
        })
    }
}
pub(crate) fn register(r: &mut NodeRegistry) {
    macro_rules! pair {
        ($in_id:literal,$out_id:literal,$label_in:literal,$label_out:literal,$provider:literal) => {{
            static I: NodeDescriptor = NodeDescriptor {
                type_id: $in_id,
                name: $label_in,
                category: "Video I/O",
                inputs: &[],
                outputs: &[Port::texture("Image")],
                parameters: &[],
                capabilities: Capabilities::NONE,
            };
            static O: NodeDescriptor = NodeDescriptor {
                type_id: $out_id,
                name: $label_out,
                category: "Video I/O",
                inputs: &[Port::texture("Image")],
                outputs: &[],
                parameters: &[],
                capabilities: Capabilities {
                    program_output: Some(0),
                    ..Capabilities::NONE
                },
            };
            r.register(Input {
                d: &I,
                provider: $provider,
            })
            .expect("video input");
            r.register(Output {
                d: &O,
                provider: $provider,
            })
            .expect("video output");
        }};
    }
    pair!("NdiIn", "NdiOut", "NDI IN", "NDI OUT", "ndi");
    pair!("SyphonIn", "SyphonOut", "Syphon IN", "Syphon OUT", "syphon");
    pair!(
        "SpoutIn",
        "SpoutOut",
        "Spout IN (Windows)",
        "Spout OUT (Windows)",
        "spout"
    );
}
