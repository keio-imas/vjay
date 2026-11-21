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
struct LoadableVideo;
static D: NodeDescriptor = NodeDescriptor {
    type_id: "LoadableVideo",
    name: "Loadable Video",
    category: "Sources",
    inputs: &[Port::file("File").optional()],
    outputs: &[Port::texture("Video"), Port::audio("Audio")],
    capabilities: Capabilities {
        transport: Some(crate::transport::TransportBinding::at(1)),
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
        file_input: Some(0),
        asset_selection_parameter: Some(6),
        ..Capabilities::NONE
    },
    parameters: &[
        P::new("Speed", 0.0, 8.0, 1.0),
        P::new("Seek position", 0.0, 1.0, 0.0),
        P::new("Loop", 0.0, 1.0, 1.0).choices(crate::choices::SWITCH),
        P::new("Range start", 0.0, 1.0, 0.0),
        P::new("Range end", 0.0, 1.0, 1.0),
        P::new("Seek", 0.0, 1.0, 0.0).choices(crate::choices::SWITCH),
        P::new("Clip index", 0.0, 4095.0, 0.0),
    ],
};
impl Node for LoadableVideo {
    fn feedback(&self) -> Option<crate::execution::ScalarKernel> {
        Some(crate::transport::feedback())
    }
    fn port_version(&self) -> u32 {
        2
    }
    fn migrate_input_port(&self, input: usize, from: u32) -> usize {
        let input = input + usize::from(from == 0);
        input + if from < 2 && input >= 1 { 8 } else { 0 }
    }
    fn descriptor(&self) -> &'static NodeDescriptor {
        &D
    }
    fn default_settings(&self) -> NodeSettings {
        let mut s = NodeSettings {
            preview: true,
            ..Default::default()
        };
        s.properties.insert("collection.path".into(), String::new());
        s
    }
    fn schema(&self, _: &NodeSettings) -> NodeSchema {
        let mut s = crate::transport::schema(&D);
        s.properties.push(PropertyInfo {
            key: "collection.path".into(),
            label: "Folder / playlist (.m3u, .m3u8)".into(),
            multiline: false,
            path_picker: true,
            font_picker: false,
            source_picker: None,
            choices: &[],
        });
        s
    }
    fn validate_parameters(&self, v: &[f32], _: &NodeSettings) -> bool {
        crate::node::validate_parameters(&D, v) && v[3] < v[4]
    }
    fn asset_source(&self, s: &NodeSettings) -> Option<AssetRequest> {
        let path = s.properties.get("collection.path")?.trim();
        if path.is_empty() {
            return None;
        }
        Some(AssetRequest {
            provider: "video-collection",
            selection_parameter: Some(6),
            file_output: false,
            location: path.into(),
            options: std::collections::BTreeMap::from([("filter".into(), "video".into())]),
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
    r.register(LoadableVideo)
        .expect("valid loadable video node");
}
