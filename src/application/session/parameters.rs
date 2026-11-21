use super::*;

impl Session {
    pub fn set_parameter(&mut self, node: NodeId, parameter: usize, value: f32) -> PortResult<()> {
        let trigger = self.project.graph.node(node).is_ok_and(|record| {
            let caps = self.descriptor(&record.type_id).capabilities;
            caps.value_control.is_some_and(|binding| {
                binding.parameter == parameter && binding.request_trigger_output.is_some()
            }) || caps.momentary_parameter == Some(parameter)
                && value > 0.0
                && record.parameters[parameter] <= 0.0
        });
        self.project
            .graph
            .set_parameter(node, parameter, value, self.nodes.as_ref())
            .map_err(|e| e.to_string())?;
        if trigger {
            self.signals
                .events
                .push(crate::control_io::ControlEvent::Trigger(node));
        }
        let value = self
            .project
            .graph
            .node(node)
            .map_err(|e| e.to_string())?
            .parameters[parameter];
        // Only this edited definition is lowered. Open extensions may choose a
        // different shader or bake a parameter into otherwise static resources.
        let record = self.project.graph.node(node).map_err(|e| e.to_string())?;
        let definition = self.nodes.get(&record.type_id)?;
        let dynamic = !self.dirty
            && self.valid
            && self.plan.as_ref().is_some_and(|plan| {
                plan.steps.iter().filter(|s| s.node == node).all(|step| {
                    let old = &step.operation;
                    let Some(next) =
                        definition.image(&record.parameters, &record.settings, step.output)
                    else {
                        return false;
                    };
                    old.external_passthrough == next.external_passthrough
                        && old.prepared.is_none()
                        && old.parameter_slots.contains(&Some(parameter))
                        && old.shader == next.shader
                        && old.selector == next.selector
                        && old.media == next.media
                        && old.stream == next.stream
                        && old.prepared == next.prepared
                        && old.parameter_slots == next.parameter_slots
                        && old.parameters.len() == next.parameters.len()
                        && old.parameters.iter().zip(&next.parameters).enumerate().all(
                            |(i, (a, b))| {
                                old.parameter_slots.get(i).is_some_and(Option::is_some) || a == b
                            },
                        )
                })
            });
        if !self.dirty
            && self.valid
            && dynamic
            && self.signals.update_parameter(node, parameter, value)
        {
            // Selection ports can launch a file job immediately; do not add an
            // extra render-frame delay just because bindings stayed intact.
            self.asset_sources.tick(&self.signals);
            self.mark_modified();
        } else {
            self.invalidate(false);
        }
        Ok(())
    }
}
