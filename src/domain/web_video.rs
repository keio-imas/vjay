use crate::{
    builtins::ImageNode,
    capabilities::{AudioRecipe, Capabilities, MediaBinding},
    execution::{AssetRequest, ImageKernel},
    node::{Node, NodeDescriptor, Parameter as P},
    port::Port,
    registry::NodeRegistry,
    schema::{NodeSchema, PropertyInfo},
    settings::NodeSettings,
};
struct WebVideo;
static D: NodeDescriptor = NodeDescriptor {
    type_id: "InternalBrowser",
    name: "Internal Browser / URL video",
    category: "Sources",
    inputs: &[],
    outputs: &[Port::texture("Video"), Port::audio("Audio")],
    capabilities: Capabilities {
        transport: Some(crate::transport::TransportBinding::at(0)),
        media: Some(MediaBinding {
            speed: 0,
            position: 1,
            looping: Some(2),
            start: Some(3),
            end: Some(4),
            seek: Some(5),
        }),
        audio: Some(AudioRecipe::Source),
        momentary_parameter: Some(5),
        ..Capabilities::NONE
    },
    parameters: &[
        P::new("Speed", 0.0, 8.0, 1.0),
        P::new("Seek position", 0.0, 1.0, 0.0),
        P::new("Loop", 0.0, 1.0, 1.0).choices(crate::choices::SWITCH),
        P::new("Range start", 0.0, 1.0, 0.0),
        P::new("Range end", 0.0, 1.0, 1.0),
        P::new("Seek", 0.0, 1.0, 0.0).choices(crate::choices::SWITCH),
    ],
};
impl Node for WebVideo {
    fn feedback(&self) -> Option<crate::execution::ScalarKernel> {
        Some(crate::transport::feedback())
    }
    fn port_version(&self) -> u32 {
        1
    }
    fn migrate_input_port(&self, input: usize, from: u32) -> usize {
        input + if from == 0 { 8 } else { 0 }
    }

    fn descriptor(&self) -> &'static NodeDescriptor {
        &D
    }
    fn default_settings(&self) -> NodeSettings {
        let mut s = NodeSettings {
            preview: true,
            ..Default::default()
        };
        s.properties.extend([
            ("url".into(), String::new()),
            ("max_height".into(), "1080".into()),
        ]);
        s
    }
    fn schema(&self, _: &NodeSettings) -> NodeSchema {
        let mut s = crate::transport::schema(&D);
        s.properties = [
            ("url", "Video URL"),
            ("max_height", "Maximum height (360–2160)"),
        ]
        .into_iter()
        .map(|(key, label)| PropertyInfo {
            key: key.into(),
            label: label.into(),
            multiline: false,
            path_picker: false,
            font_picker: false,
            source_picker: None,
            choices: &[],
        })
        .collect();
        s
    }
    fn validate_settings(&self, s: &NodeSettings) -> bool {
        s.is_valid()
            && s.properties
                .get("max_height")
                .is_none_or(|v| v.parse::<u32>().is_ok_and(|n| (360..=2160).contains(&n)))
            && s.properties.get("url").is_none_or(|v| {
                v.is_empty() || v.starts_with("https://") || v.starts_with("http://")
            })
    }
    fn validate_parameters(&self, v: &[f32], _: &NodeSettings) -> bool {
        crate::node::validate_parameters(&D, v) && v[3] < v[4]
    }
    fn asset_source(&self, s: &NodeSettings) -> Option<AssetRequest> {
        let url = s.properties.get("url")?.trim();
        if url.is_empty() {
            return None;
        }
        Some(AssetRequest {
            provider: "web-video",
            selection_parameter: None,
            file_output: false,
            location: url.into(),
            options: s.properties.clone(),
        })
    }
    fn image(&self, v: &[f32], s: &NodeSettings, o: usize) -> Option<ImageKernel> {
        ImageNode {
            descriptor: &D,
            selector: 3.0,
        }
        .image(v, s, o)
    }
}
pub(crate) fn register(r: &mut NodeRegistry) {
    r.register(WebVideo).expect("valid URL video node");
}
