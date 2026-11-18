use crate::{
    builtins::ImageNode,
    capabilities::{Capabilities, WidgetKind},
    execution::{AssetRequest, FileEvaluate, ImageKernel, ScalarKernel, TextKernel},
    node::{Node, NodeDescriptor, Parameter as P},
    port::Port,
    registry::NodeRegistry,
    schema::{NodeSchema, PropertyInfo},
    settings::NodeSettings,
};
struct Files;
struct LoadableImage;
static FILES: NodeDescriptor = NodeDescriptor {
    type_id: "Files",
    name: "Files",
    category: "Controls",
    inputs: &[],
    outputs: &[
        Port::file("File"),
        Port::scalar("Index"),
        Port::scalar("Count"),
        Port::text("File name"),
    ],
    parameters: &[P::new("Index", 0.0, 4095.0, 0.0)],
    capabilities: Capabilities {
        widget: Some(WidgetKind::Files),
        asset_selection_parameter: Some(0),
        ..Capabilities::NONE
    },
};
static IMAGE: NodeDescriptor = NodeDescriptor {
    type_id: "LoadableImage",
    name: "Loadable Image",
    category: "Sources",
    inputs: &[Port::file("File").optional()],
    outputs: &[Port::texture("Image")],
    parameters: &[P::new("Image index", 0.0, 4095.0, 0.0)],
    capabilities: Capabilities {
        still_image: true,
        file_input: Some(0),
        asset_selection_parameter: Some(0),
        ..Capabilities::NONE
    },
};
fn settings(filter: &str) -> NodeSettings {
    let mut s = NodeSettings::default();
    s.properties.extend([
        ("collection.path".into(), String::new()),
        ("filter".into(), filter.into()),
    ]);
    s
}
fn schema(d: &'static NodeDescriptor, filter: bool) -> NodeSchema {
    let mut s = NodeSchema::new(d);
    s.properties.push(PropertyInfo {
        key: "collection.path".into(),
        label: "File / folder / playlist".into(),
        multiline: false,
        path_picker: true,
        font_picker: false,
        source_picker: None,
        choices: &[],
    });
    if filter {
        s.properties.push(PropertyInfo {
            key: "filter".into(),
            label: "Filter".into(),
            multiline: false,
            path_picker: false,
            font_picker: false,
            source_picker: None,
            choices: &["all", "video", "image"],
        });
    }
    s
}
fn request(s: &NodeSettings, file_output: bool) -> Option<AssetRequest> {
    let path = s.properties.get("collection.path")?.trim();
    if path.is_empty() {
        return None;
    }
    Some(AssetRequest {
        provider: "file-collection",
        file_output,
        selection_parameter: Some(0),
        location: path.into(),
        options: s.properties.clone(),
    })
}
impl Node for Files {
    fn descriptor(&self) -> &'static NodeDescriptor {
        &FILES
    }
    fn default_settings(&self) -> NodeSettings {
        settings("all")
    }
    fn schema(&self, _: &NodeSettings) -> NodeSchema {
        schema(&FILES, true)
    }
    fn asset_source(&self, s: &NodeSettings) -> Option<AssetRequest> {
        request(s, true)
    }
    fn file(&self) -> Option<FileEvaluate> {
        Some(|ctx, out| out[0] = ctx.host.node_file(ctx.node, 0))
    }
    fn scalar(&self) -> Option<ScalarKernel> {
        Some(ScalarKernel {
            state: vec![],
            tap: None,
            evaluate: |ctx, _, out| {
                out[0] = ctx.host.node_value(ctx.node, 0);
                out[1] = ctx.host.node_value(ctx.node, 1);
            },
        })
    }
    fn text(&self, _: &NodeSettings) -> Option<TextKernel> {
        Some(TextKernel {
            state: vec![],
            evaluate: |ctx, _, out| out[0] = ctx.host.node_text(ctx.node, 0),
        })
    }
}
impl Node for LoadableImage {
    fn descriptor(&self) -> &'static NodeDescriptor {
        &IMAGE
    }
    fn default_settings(&self) -> NodeSettings {
        let mut s = settings("image");
        s.properties.remove("filter");
        s.preview = true;
        s
    }
    fn schema(&self, _: &NodeSettings) -> NodeSchema {
        schema(&IMAGE, false)
    }
    fn asset_source(&self, s: &NodeSettings) -> Option<AssetRequest> {
        let mut r = request(s, false)?;
        r.options.insert("filter".into(), "image".into());
        Some(r)
    }
    fn image(&self, _: &[f32], s: &NodeSettings, o: usize) -> Option<ImageKernel> {
        ImageNode {
            descriptor: &IMAGE,
            selector: 20.0,
        }
        .image(&[], s, o)
    }
}
pub(crate) fn register(r: &mut NodeRegistry) {
    static CUE: NodeDescriptor = NodeDescriptor {
        type_id: "AudioCue",
        name: "Cue / Headphones",
        category: "Audio",
        inputs: &[Port::audio("Audio")],
        outputs: &[],
        parameters: &[P::new("Monitor volume", 0.0, 2.0, 1.0)],
        capabilities: Capabilities {
            audio_cue: true,
            audio: Some(crate::capabilities::AudioRecipe::Sink { input: 0, gain: 0 }),
            ..Capabilities::NONE
        },
    };
    r.register(crate::builtins::RoutingNode(&CUE))
        .expect("cue node");
    r.register(Files).expect("files node");
    r.register(LoadableImage).expect("image node");
}
