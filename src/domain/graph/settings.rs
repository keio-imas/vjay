use super::*;
impl Graph {
    /// Schema changes preserve positional renames and match surviving names
    /// when the number of ports/parameters changes.
    pub fn set_settings(
        &mut self,
        id: NodeId,
        mut settings: crate::settings::NodeSettings,
        factory: &dyn NodeFactory,
    ) -> Result<(), GraphError> {
        let index = self.index(id)?;
        let definition = factory.get(&self.nodes[index].type_id)?;
        settings.port_version = definition.port_version();
        if !settings.is_valid() || !definition.validate_settings(&settings) {
            return Err(GraphError::InvalidParameter);
        }
        let old = &self.ports[index].schema;
        let new = definition.schema(&settings);
        let parameters = mapping(
            old.parameters.iter().map(|p| p.name.as_str()),
            new.parameters.iter().map(|p| p.name.as_str()),
        );
        let inputs = mapping(
            old.inputs.iter().map(|p| p.name.as_str()),
            new.inputs.iter().map(|p| p.name.as_str()),
        );
        let outputs = mapping(
            old.outputs.iter().map(|p| p.name.as_str()),
            new.outputs.iter().map(|p| p.name.as_str()),
        );
        let mut values: Vec<_> = new.parameters.iter().map(|p| p.default).collect();
        for (old_slot, new_slot) in parameters.iter().enumerate() {
            if let Some(slot) = new_slot {
                values[*slot] = self.nodes[index].parameters[old_slot]
                    .clamp(new.parameters[*slot].min, new.parameters[*slot].max);
            }
        }
        if !definition.validate_parameters(&values, &settings) {
            return Err(GraphError::InvalidParameter);
        }
        let old_inputs = old.inputs.len();
        let new_inputs = new.inputs.len();
        let mut ports = NodePorts::new(new);
        for (old_slot, slot) in outputs.iter().enumerate() {
            if let Some(slot) = slot {
                ports.outputs[*slot].destinations =
                    std::mem::take(&mut self.ports[index].outputs[old_slot].destinations);
            }
        }
        self.ports[index] = ports;
        for ports in &mut self.ports {
            for output in &mut ports.outputs {
                output.destinations.retain_mut(|input| {
                    if input.node != id {
                        return true;
                    }
                    let slot = if input.port < old_inputs {
                        inputs.get(input.port).copied().flatten()
                    } else {
                        parameters
                            .get(input.port - old_inputs)
                            .copied()
                            .flatten()
                            .map(|s| new_inputs + s)
                    };
                    if let Some(slot) = slot {
                        input.port = slot;
                        true
                    } else {
                        false
                    }
                });
            }
        }
        self.nodes[index].parameters = values;
        self.nodes[index].settings = settings;
        self.connection_count = self
            .ports
            .iter()
            .flat_map(|p| &p.outputs)
            .map(|p| p.destinations.len())
            .sum();
        Ok(())
    }
}
fn mapping<'a>(
    old: impl Iterator<Item = &'a str>,
    new: impl Iterator<Item = &'a str>,
) -> Vec<Option<usize>> {
    let old: Vec<_> = old.collect();
    let new: Vec<_> = new.collect();
    old.iter()
        .enumerate()
        .map(|(i, name)| {
            if old.len() == new.len() {
                Some(i)
            } else {
                new.iter().position(|n| n == name)
            }
        })
        .collect()
}
