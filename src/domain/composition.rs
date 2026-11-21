use crate::{
    capabilities::Capabilities,
    node::{Node, NodeDescriptor},
    port::SignalType,
    registry::NodeRegistry,
    schema::{NodeSchema, PortInfo, PropertyInfo},
    settings::NodeSettings,
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Interface {
    pub inputs: Vec<InterfacePort>,
    pub outputs: Vec<InterfacePort>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterfacePort {
    pub key: String,
    pub name: String,
    pub signal: SignalType,
    #[serde(default)]
    pub delayed: bool,
}
impl Interface {
    pub fn valid(&self) -> bool {
        self.inputs.len() <= 32
            && self.outputs.len() <= 32
            && [&self.inputs, &self.outputs].iter().all(|ports| {
                let mut keys = std::collections::HashSet::new();
                ports.iter().all(|p| {
                    !p.key.is_empty()
                        && p.key.len() <= 128
                        && p.name.len() <= 128
                        && keys.insert(&p.key)
                })
            })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    In,
    Out,
}
#[derive(Clone, Debug)]
pub struct Boundary {
    pub direction: Direction,
    pub order: u32,
    pub port: InterfacePort,
}
const TYPES: &[&str] = &["Image", "Value", "Audio", "Text", "File"];
static PATCH: NodeDescriptor = NodeDescriptor {
    type_id: "Subpatch",
    name: "Subpatch",
    category: "Patching",
    inputs: &[],
    outputs: &[],
    parameters: &[],
    capabilities: Capabilities::NONE,
};
static IN: NodeDescriptor = NodeDescriptor {
    type_id: "In",
    name: "In",
    category: "Patching",
    inputs: &[],
    outputs: &[],
    parameters: &[],
    capabilities: Capabilities::NONE,
};
static OUT: NodeDescriptor = NodeDescriptor {
    type_id: "Out",
    name: "Out",
    category: "Patching",
    inputs: &[],
    outputs: &[],
    parameters: &[],
    capabilities: Capabilities::NONE,
};
struct Subpatch;
impl Node for Subpatch {
    fn descriptor(&self) -> &'static NodeDescriptor {
        &PATCH
    }
    fn composite(&self) -> bool {
        true
    }
    fn schema(&self, s: &NodeSettings) -> NodeSchema {
        let mut schema = NodeSchema::new(&PATCH);
        if let Some(i) = &s.interface {
            let convert = |p: &InterfacePort| PortInfo {
                name: p.name.clone(),
                signal: p.signal,
                optional: true,
                delayed: p.delayed,
            };
            schema.inputs = i.inputs.iter().map(convert).collect();
            schema.outputs = i.outputs.iter().map(convert).collect();
        }
        schema
    }
}
struct Endpoint(Direction);
impl Node for Endpoint {
    fn descriptor(&self) -> &'static NodeDescriptor {
        if self.0 == Direction::In { &IN } else { &OUT }
    }
    fn default_settings(&self) -> NodeSettings {
        let mut s = NodeSettings {
            preview: false,
            ..Default::default()
        };
        s.properties.extend([
            ("type".into(), "Image".into()),
            ("order".into(), "1".into()),
            ("port.key".into(), String::new()),
            (
                "name".into(),
                if self.0 == Direction::In { "In" } else { "Out" }.into(),
            ),
        ]);
        s
    }
    fn boundary(&self, s: &NodeSettings) -> Option<Boundary> {
        let signal = match s.properties.get("type")?.as_str() {
            "Image" => SignalType::Texture,
            "Value" => SignalType::Scalar,
            "Audio" => SignalType::Audio,
            "Text" => SignalType::Text,
            "File" => SignalType::File,
            _ => return None,
        };
        Some(Boundary {
            direction: self.0,
            order: s.properties.get("order")?.parse().ok()?,
            port: InterfacePort {
                key: s.properties.get("port.key")?.clone(),
                name: s.properties.get("name")?.clone(),
                signal,
                delayed: false,
            },
        })
    }
    fn schema(&self, s: &NodeSettings) -> NodeSchema {
        let mut schema = NodeSchema::new(self.descriptor());
        if let Some(b) = self.boundary(s) {
            let p = PortInfo {
                name: b.port.name,
                signal: b.port.signal,
                optional: false,
                delayed: false,
            };
            if self.0 == Direction::In {
                schema.outputs.push(p);
            } else {
                schema.inputs.push(p);
            }
        }
        schema.properties = [
            ("name", "Port name", &[] as &'static [&'static str]),
            ("type", "Type", TYPES),
            ("order", "Order", &[]),
        ]
        .map(|(key, label, choices)| PropertyInfo {
            key: key.into(),
            label: label.into(),
            choices,
            multiline: false,
            path_picker: false,
            font_picker: false,
            source_picker: None,
        })
        .to_vec();
        schema
    }
    fn validate_settings(&self, s: &NodeSettings) -> bool {
        s.is_valid()
            && self.boundary(s).is_some_and(|b| {
                b.order > 0
                    && b.order <= 4096
                    && b.port.name.len() <= 128
                    && b.port.key.len() <= 128
            })
    }
}
pub(crate) fn register(r: &mut NodeRegistry) {
    r.register(Subpatch).expect("valid subpatch");
    r.register(Endpoint(Direction::In)).expect("valid inlet");
    r.register(Endpoint(Direction::Out)).expect("valid outlet");
}
