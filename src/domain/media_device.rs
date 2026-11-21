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

struct MediaDevice;
static D: NodeDescriptor = NodeDescriptor {
    type_id: "MediaDeviceInput",
    name: "Media Device Input",
    category: "Sources",
    inputs: &[],
    outputs: &[Port::texture("Image")],
    parameters: &[],
    capabilities: Capabilities::NONE,
};
impl Node for MediaDevice {
    fn descriptor(&self) -> &'static NodeDescriptor {
        &D
    }
    fn default_settings(&self) -> NodeSettings {
        let mut s = NodeSettings {
            preview: true,
            ..Default::default()
        };
        s.properties.extend([
            ("source".into(), String::new()),
            ("enabled".into(), "Off".into()),
            ("capture.size".into(), "1280x720".into()),
            ("capture.fps".into(), "30".into()),
        ]);
        s
    }
    fn schema(&self, _: &NodeSettings) -> NodeSchema {
        let mut s = NodeSchema::new(&D);
        s.properties = [
            (
                "source",
                "Camera / capture device",
                &[] as &'static [&'static str],
                Some("capture"),
            ),
            ("enabled", "Capture", &["Off", "On"], None),
            (
                "capture.size",
                "Capture size",
                &["640x480", "1280x720", "1920x1080", "3840x2160"],
                None,
            ),
            (
                "capture.fps",
                "Frame rate",
                &["24", "25", "30", "50", "60"],
                None,
            ),
        ]
        .into_iter()
        .map(|(key, label, choices, source_picker)| PropertyInfo {
            key: key.into(),
            label: label.into(),
            multiline: false,
            path_picker: false,
            font_picker: false,
            source_picker,
            choices,
        })
        .collect();
        s
    }
    fn validate_settings(&self, s: &NodeSettings) -> bool {
        s.is_valid()
            && self.schema(s).properties.iter().all(|p| {
                p.choices.is_empty()
                    || s.properties
                        .get(&p.key)
                        .is_some_and(|v| p.choices.contains(&v.as_str()))
            })
    }
    fn image(&self, v: &[f32], s: &NodeSettings, output: usize) -> Option<ImageKernel> {
        let mut k = ImageNode {
            descriptor: &D,
            selector: 20.0,
        }
        .image(v, s, output)?;
        if s.properties.get("enabled").is_some_and(|s| s == "On") {
            k.stream = Some(StreamRequest {
                provider: "capture",
                name: s.properties.get("source").cloned().unwrap_or_default(),
                options: s.properties.clone(),
            });
        }
        Some(k)
    }
}
pub(crate) fn register(r: &mut NodeRegistry) {
    r.register(MediaDevice).expect("valid media device node");
}
