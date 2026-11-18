use super::*;
use vjay_domain::uniform::ShaderUniform;
impl Session {
    pub fn shader_uniforms(&self, node: NodeId) -> PortResult<Vec<ShaderUniform>> {
        let record = self.project.graph.node(node).map_err(|e| e.to_string())?;
        if !self
            .schema(node)
            .capabilities
            .settings
            .contains(&vjay_domain::capabilities::SettingEditor::Fragment)
        {
            return Err("This node has no fragment uniforms".into());
        }
        Ok(record.settings.shader_uniforms.clone().unwrap_or_else(|| {
            self.schema(node)
                .parameters
                .iter()
                .map(|p| ShaderUniform {
                    name: p.name.clone(),
                    min: p.min,
                    max: p.max,
                    default: p.default,
                })
                .collect()
        }))
    }
    pub fn set_shader_uniforms(
        &mut self,
        node: NodeId,
        uniforms: Vec<ShaderUniform>,
    ) -> PortResult<()> {
        let mut settings = self
            .project
            .graph
            .node(node)
            .map_err(|e| e.to_string())?
            .settings
            .clone();
        if uniforms.len() > 64 {
            return Err("Maximum 64 scalar uniforms".into());
        }
        let mut names = std::collections::HashSet::new();
        for uniform in &uniforms {
            if !uniform.is_valid() {
                return Err(format!(
                    "Invalid identifier, range or default for uniform {}",
                    uniform.name
                ));
            }
            if !names.insert(uniform.name.as_str()) {
                return Err(format!("Duplicate uniform name: {}", uniform.name));
            }
        }
        let old = self.shader_uniforms(node)?;
        if old.len() == uniforms.len() {
            let renamed = old
                .iter()
                .zip(&uniforms)
                .filter(|(a, b)| a.name != b.name)
                .map(|(a, b)| (a.name.as_str(), b.name.as_str()))
                .collect::<Vec<_>>();
            settings.shader = vjay_domain::uniform::rewrite_identifiers(&settings.shader, &renamed);
        }
        settings.shader_uniforms = Some(uniforms);
        self.set_settings(node, settings)
    }
}
