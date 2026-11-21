use crate::{
    execution::*,
    node::*,
    schema::{NodeSchema, ParameterInfo},
    settings::NodeSettings,
};
pub(crate) struct FragmentNode(pub(crate) &'static NodeDescriptor);
impl Node for FragmentNode {
    fn descriptor(&self) -> &'static NodeDescriptor {
        self.0
    }
    fn schema(&self, settings: &NodeSettings) -> NodeSchema {
        let mut schema = NodeSchema::new(self.0);
        if let Some(uniforms) = &settings.shader_uniforms {
            schema.parameters = uniforms
                .iter()
                .map(|u| ParameterInfo {
                    name: u.name.clone(),
                    range: Parameter::new("Uniform", u.min, u.max, u.default),
                })
                .collect();
        }
        schema
    }
    fn image(&self, values: &[f32], settings: &NodeSettings, _: usize) -> Option<ImageKernel> {
        let schema = self.schema(settings);
        let mut shader = String::new();
        for (index, p) in schema.parameters.iter().enumerate() {
            let field = if index < 4 {
                format!("frame.parameters.{}", ["x", "y", "z", "w"][index])
            } else {
                format!(
                    "frame.extra[{}].{}",
                    (index - 4) / 4,
                    ["x", "y", "z", "w"][index % 4]
                )
            };
            shader.push_str(&format!("#define {} {field}\n", p.name));
        }
        for line in settings.shader.lines() {
            if let Some((name, rest)) = line
                .trim()
                .strip_prefix("uniform float ")
                .and_then(|s| s.split_once(';'))
                && schema.parameters.iter().any(|p| p.name == name.trim())
            {
                shader.push_str(rest);
                shader.push('\n');
                continue;
            }
            shader.push_str(line);
            shader.push('\n');
        }
        Some(ImageKernel {
            external_passthrough: false,
            shader: ShaderSource::Glsl(shader),
            selector: 20.0,
            parameters: values.to_vec(),
            parameter_slots: (0..values.len()).map(Some).collect(),
            prepared: None,
            stream: None,
            media: None,
        })
    }
}
