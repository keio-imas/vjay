use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct NodeSettings {
    pub port_version: u32,
    pub preview: bool,
    pub comment: String,
    pub shader_uniforms: Option<Vec<crate::uniform::ShaderUniform>>,
    pub key: String,
    pub osc_address: String,
    pub shader: String,
    pub label: String,
    pub midi_source: Option<String>,
    /// Namespaced settings owned by extension nodes; old projects may omit them.
    pub properties: std::collections::BTreeMap<String, String>,
}
impl Default for NodeSettings {
    fn default() -> Self {
        Self {
            port_version: 0,
            preview: false,
            comment: String::new(),
            shader_uniforms: None,
            key: "space".into(),
            osc_address: "/vjay".into(),
            label: String::new(),
            midi_source: None,
            properties: Default::default(),
            shader: DEFAULT_FRAGMENT.into(),
        }
    }
}
pub const DEFAULT_FRAGMENT: &str = "void mainImage(out vec4 color, in vec2 fragCoord) {\n    vec2 uv = fragCoord / iResolution.xy;\n    color = vec4(uv, 0.5 + 0.5 * sin(iTime), 1.0);\n}\n";

impl NodeSettings {
    pub fn is_valid(&self) -> bool {
        self.comment.len() <= 16384
            && self.shader_uniforms.as_ref().is_none_or(|u| {
                u.len() <= 64
                    && u.iter().all(|v| v.is_valid())
                    && u.iter()
                        .map(|v| v.name.as_str())
                        .collect::<std::collections::HashSet<_>>()
                        .len()
                        == u.len()
            })
            && self.shader.len() <= 256 * 1024
            && self.properties.len() <= 64
            && self
                .properties
                .iter()
                .all(|(key, value)| !key.is_empty() && key.len() <= 128 && value.len() <= 4096)
            && self.key.len() <= 64
            && self.osc_address.len() <= 256
            && self.label.len() <= 256
            && self
                .midi_source
                .as_ref()
                .is_none_or(|id| !id.is_empty() && id.len() <= 1024)
    }
}
