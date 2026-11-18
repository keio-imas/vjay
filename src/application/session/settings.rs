use super::*;
use vjay_domain::settings::NodeSettings;
impl Session {
    pub fn set_settings(&mut self, node: NodeId, settings: NodeSettings) -> PortResult<()> {
        let record = self.project.graph.node(node).map_err(|e| e.to_string())?;
        let shader = self
            .descriptor(&record.type_id)
            .capabilities
            .settings
            .contains(&vjay_domain::capabilities::SettingEditor::Fragment);
        if shader {
            let definition = self.nodes.get(&record.type_id)?;
            let candidate = definition
                .image(&record.parameters, &settings, 0)
                .ok_or("Missing shader kernel")?;
            if let vjay_domain::execution::ShaderSource::Glsl(source) = candidate.shader {
                self.renderer.validate_shader(&source)?;
            }
        }
        let old_names: Vec<_> = self
            .schema(node)
            .parameters
            .iter()
            .map(|p| p.name.clone())
            .collect();
        self.project
            .graph
            .set_settings(node, settings, self.nodes.as_ref())
            .map_err(|e| e.to_string())?;
        let new_names: Vec<_> = self
            .schema(node)
            .parameters
            .iter()
            .map(|p| p.name.clone())
            .collect();
        if old_names.len() != new_names.len() {
            self.project.controls.retain_mut(|control| {
                if control.node != node || control.target != ControlTarget::Parameter {
                    return true;
                }
                let Some(name) = old_names.get(control.parameter) else {
                    return false;
                };
                let Some(index) = new_names.iter().position(|n| n == name) else {
                    return false;
                };
                control.parameter = index;
                true
            });
        }
        let schema = self.schema(node).clone();
        self.project.controls.retain_mut(|control| {
            if control.node != node || control.target != ControlTarget::Parameter {
                return true;
            }
            let Some(parameter) = schema.parameters.get(control.parameter) else {
                return false;
            };
            if control.widget == WidgetKind::XYPad
                && control.parameter + 1 >= schema.parameters.len()
            {
                return false;
            }
            if control
                .range
                .is_some_and(|[min, max]| min < parameter.min || max > parameter.max)
            {
                control.range = None;
            }
            true
        });
        self.invalidate(true);
        Ok(())
    }
}
