use crate::{
    node::{NodeDescriptor, Parameter},
    port::{Port, SignalType},
};
use std::ops::Deref;
#[derive(Clone, Debug)]
pub struct PortInfo {
    pub name: String,
    pub signal: SignalType,
    pub optional: bool,
}
impl From<&Port> for PortInfo {
    fn from(p: &Port) -> Self {
        Self {
            name: p.name.into(),
            signal: p.signal,
            optional: p.optional,
        }
    }
}
#[derive(Clone, Debug)]
pub struct ParameterInfo {
    pub name: String,
    pub range: Parameter,
}
#[derive(Clone, Debug)]
pub struct PropertyInfo {
    pub key: String,
    pub label: String,
    pub multiline: bool,
    pub path_picker: bool,
    pub font_picker: bool,
    pub source_picker: Option<&'static str>,
    pub choices: &'static [&'static str],
}
impl Deref for ParameterInfo {
    type Target = Parameter;
    fn deref(&self) -> &Parameter {
        &self.range
    }
}
#[derive(Clone, Debug)]
pub struct NodeSchema {
    pub definition: &'static NodeDescriptor,
    pub inputs: Vec<PortInfo>,
    pub outputs: Vec<PortInfo>,
    pub parameters: Vec<ParameterInfo>,
    pub properties: Vec<PropertyInfo>,
}
impl Deref for NodeSchema {
    type Target = NodeDescriptor;
    fn deref(&self) -> &NodeDescriptor {
        self.definition
    }
}
impl NodeSchema {
    pub fn new(definition: &'static NodeDescriptor) -> Self {
        Self {
            definition,
            inputs: definition.inputs.iter().map(PortInfo::from).collect(),
            outputs: definition.outputs.iter().map(PortInfo::from).collect(),
            parameters: definition
                .parameters
                .iter()
                .map(|p| ParameterInfo {
                    name: p.name.into(),
                    range: *p,
                })
                .collect(),
            properties: Vec::new(),
        }
    }
    pub fn input_count(&self) -> usize {
        self.inputs.len() + self.parameters.len()
    }
    pub fn input(&self, index: usize) -> Option<PortInfo> {
        self.inputs.get(index).cloned().or_else(|| {
            self.parameters
                .get(index.checked_sub(self.inputs.len())?)
                .map(|p| PortInfo {
                    name: p.name.clone(),
                    signal: SignalType::Scalar,
                    optional: true,
                })
        })
    }
    pub fn valid_values(&self, values: &[f32]) -> bool {
        values.len() == self.parameters.len()
            && values
                .iter()
                .zip(&self.parameters)
                .all(|(&v, p)| p.normalize(v) == Some(v))
    }
}
