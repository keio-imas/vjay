use crate::{graph::NodeId, settings::NodeSettings};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ShaderSource {
    Registered(&'static str),
    Glsl(String),
    Wgsl { source: String, entry: String },
}
/// Open preparation request; adapters are selected by provider at the composition root.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PreparedImage {
    pub provider: String,
    pub properties: std::collections::BTreeMap<String, String>,
    pub text_inputs: Vec<TextInputBinding>,
    pub scalar_inputs: Vec<PreparedParameter>,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PreparedParameter {
    pub step: Option<u32>,
    pub parameter: usize,
    pub property: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TextInputBinding {
    pub input: usize,
    pub property: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AssetRequest {
    pub provider: &'static str,
    pub selection_parameter: Option<usize>,
    pub file_output: bool,
    pub location: String,
    pub options: std::collections::BTreeMap<String, String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct StreamRequest {
    pub provider: &'static str,
    pub name: String,
    pub options: std::collections::BTreeMap<String, String>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ImageKernel {
    /// The output is identical to the injected full-extent external image.
    /// Adapters may forward its persistent texture without a render pass.
    pub external_passthrough: bool,
    pub shader: ShaderSource,
    pub selector: f32,
    pub parameters: Vec<f32>,
    pub parameter_slots: Vec<Option<usize>>,
    pub media: Option<crate::capabilities::MediaBinding>,
    pub prepared: Option<PreparedImage>,
    pub stream: Option<StreamRequest>,
}
impl ImageKernel {
    pub fn set_parameter(&mut self, index: usize, value: f32) {
        for (slot, source) in self.parameter_slots.iter().enumerate() {
            if *source == Some(index) {
                self.parameters[slot] = value;
            }
        }
    }
}
/// Input protocols are independent of node identities. External channels let
/// new transports publish values without modifying this contract or scheduler.
pub type FileValue = std::sync::Arc<std::path::Path>;
pub struct FileContext<'a> {
    pub node: NodeId,
    pub inputs: &'a [Option<FileValue>],
    pub host: &'a dyn SignalInputs,
}
pub type FileEvaluate = fn(&FileContext<'_>, &mut [Option<FileValue>]);
pub trait SignalInputs {
    fn media(&self, _: NodeId) -> crate::transport::MediaState {
        Default::default()
    }
    fn node_file(&self, _node: NodeId, _output: usize) -> Option<FileValue> {
        None
    }
    fn node_value(&self, _node: NodeId, _output: usize) -> f32 {
        0.0
    }
    fn node_text(&self, _node: NodeId, _output: usize) -> std::sync::Arc<str> {
        empty_text()
    }
    fn key(&self, key: &str) -> [f32; 2];
    fn trigger(&self, node: NodeId) -> bool;
    fn midi(
        &self,
        source: Option<&str>,
        channel: usize,
        controller: usize,
        note: Option<usize>,
    ) -> [f32; 4];
    fn osc(&self, address: &str) -> [f32; 2];
    fn external(&self, channel: &str, output: usize) -> f32;
}
pub struct ScalarContext<'a> {
    pub node: NodeId,
    pub delta: f64,
    pub settings: &'a NodeSettings,
    pub parameters: &'a [f32],
    pub inputs: &'a [Option<f32>],
    pub host: &'a dyn SignalInputs,
}
pub type ScalarEvaluate = fn(&ScalarContext<'_>, &mut [f64], &mut [f32]);
pub type ScalarTap = fn(&mut [f64], f64);
pub struct ScalarKernel {
    pub evaluate: ScalarEvaluate,
    pub state: Vec<f64>,
    pub tap: Option<ScalarTap>,
}

pub struct TextContext<'a> {
    pub node: NodeId,
    pub inputs: &'a [Option<std::sync::Arc<str>>],
    pub host: &'a dyn SignalInputs,
}
pub type TextEvaluate =
    fn(&TextContext<'_>, &mut [std::sync::Arc<str>], &mut [std::sync::Arc<str>]);
pub struct TextKernel {
    pub evaluate: TextEvaluate,
    pub state: Vec<std::sync::Arc<str>>,
}

/// Shared empty value keeps disconnected text inputs allocation free.
pub fn empty_text() -> std::sync::Arc<str> {
    static EMPTY: std::sync::OnceLock<std::sync::Arc<str>> = std::sync::OnceLock::new();
    EMPTY.get_or_init(|| std::sync::Arc::from("")).clone()
}
