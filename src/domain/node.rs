use crate::{
    execution::{ImageKernel, ScalarKernel},
    settings::NodeSettings,
};
use serde::{Deserialize, Serialize};
use std::{fmt, ops::Deref};

/// Persistent, open identifier. Registrations define the available nodes.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NodeTypeId(String);
impl From<&str> for NodeTypeId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}
impl PartialEq<&str> for NodeTypeId {
    fn eq(&self, value: &&str) -> bool {
        self.0 == *value
    }
}
impl From<String> for NodeTypeId {
    fn from(value: String) -> Self {
        Self(value)
    }
}
impl Deref for NodeTypeId {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}
impl AsRef<str> for NodeTypeId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
impl fmt::Display for NodeTypeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AssetId(pub u64);
#[derive(Clone, Copy, Debug)]
pub struct Parameter {
    pub name: &'static str,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub wrap: Option<f32>,
    pub choices: &'static [&'static str],
}
impl Parameter {
    pub const fn new(name: &'static str, min: f32, max: f32, default: f32) -> Self {
        Self {
            name,
            min,
            max,
            default,
            wrap: None,
            choices: &[],
        }
    }
    pub const fn choices(mut self, labels: &'static [&'static str]) -> Self {
        self.choices = labels;
        self
    }
    pub fn choice_label(self, value: f32) -> Option<&'static str> {
        self.choices
            .get((value - self.min).round().max(0.0) as usize)
            .copied()
    }
    pub const fn wrapping(mut self, period: f32) -> Self {
        self.wrap = Some(period);
        self
    }
    pub fn normalize(self, value: f32) -> Option<f32> {
        if !value.is_finite() {
            return None;
        }
        let value = self.wrap.map_or(value, |period| {
            ((value as f64 - self.min as f64).rem_euclid(period as f64) + self.min as f64) as f32
        });
        (self.min..=self.max).contains(&value).then_some(value)
    }
}
#[derive(Debug)]
pub struct NodeDescriptor {
    pub type_id: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    pub inputs: &'static [crate::port::Port],
    pub outputs: &'static [crate::port::Port],
    pub parameters: &'static [Parameter],
    pub capabilities: crate::capabilities::Capabilities,
}
impl NodeDescriptor {
    pub fn input(&self, index: usize) -> Option<crate::port::Port> {
        self.inputs.get(index).copied().or_else(|| {
            self.parameters
                .get(index.checked_sub(self.inputs.len())?)
                .map(|p| crate::port::Port::scalar(p.name).optional())
        })
    }
    pub fn input_count(&self) -> usize {
        self.inputs.len() + self.parameters.len()
    }
}
/// Edit-time lowering. The frame loop uses prepared data and scalar kernels;
/// it never calls the node trait or traverses the editable graph.
pub trait Node: Send + Sync {
    fn composite(&self) -> bool {
        false
    }
    fn boundary(&self, _: &NodeSettings) -> Option<crate::composition::Boundary> {
        None
    }
    fn feedback(&self) -> Option<ScalarKernel> {
        None
    }
    fn video_sink(&self, _: &NodeSettings) -> Option<crate::execution::StreamRequest> {
        None
    }
    fn file(&self) -> Option<crate::execution::FileEvaluate> {
        None
    }
    fn port_version(&self) -> u32 {
        0
    }
    fn migrate_input_port(&self, input: usize, _from_version: u32) -> usize {
        input
    }
    fn migrate_output_port(&self, output: usize, _from_version: u32) -> usize {
        output
    }
    fn asset_source(&self, _settings: &NodeSettings) -> Option<crate::execution::AssetRequest> {
        None
    }
    fn schema(&self, _settings: &NodeSettings) -> crate::schema::NodeSchema {
        crate::schema::NodeSchema::new(self.descriptor())
    }
    fn service(&self) -> Option<&'static str> {
        None
    }
    fn text(&self, _settings: &NodeSettings) -> Option<crate::execution::TextKernel> {
        None
    }
    fn descriptor(&self) -> &'static NodeDescriptor;
    fn default_settings(&self) -> NodeSettings {
        NodeSettings {
            preview: self
                .descriptor()
                .outputs
                .iter()
                .any(|p| p.signal == crate::port::SignalType::Texture),
            ..Default::default()
        }
    }
    fn validate_settings(&self, settings: &NodeSettings) -> bool {
        settings.is_valid()
    }
    fn migrate_parameters_with_settings(&self, values: &mut Vec<f32>, _: &NodeSettings) {
        self.migrate_parameters(values);
    }
    fn migrate_parameters(&self, _values: &mut Vec<f32>) {}
    fn validate_parameters(&self, values: &[f32], settings: &NodeSettings) -> bool {
        self.schema(settings).valid_values(values)
    }
    fn image(
        &self,
        _values: &[f32],
        _settings: &NodeSettings,
        _output: usize,
    ) -> Option<ImageKernel> {
        None
    }
    fn scalar(&self) -> Option<ScalarKernel> {
        None
    }
}
pub trait NodeFactory: Send + Sync {
    fn get(&self, type_id: &str) -> Result<&dyn Node, crate::registry::NodeError>;
    fn catalog(&self) -> &[&'static NodeDescriptor];
}
pub fn validate_parameters(descriptor: &NodeDescriptor, values: &[f32]) -> bool {
    values.len() == descriptor.parameters.len()
        && values
            .iter()
            .zip(descriptor.parameters)
            .all(|(value, parameter)| {
                value.is_finite() && (parameter.min..=parameter.max).contains(value)
            })
}
