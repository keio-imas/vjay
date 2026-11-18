use super::*;
impl SignalEngine {
    pub fn bind(&self, plan: &mut crate::compiler::RenderPlan) {
        let mut bindings: HashMap<NodeId, Vec<(usize, ValueRef)>> = HashMap::new();
        for (&(node, parameter), &value) in &self.parameters {
            if value.source.is_some() {
                bindings.entry(node).or_default().push((parameter, value));
            }
        }
        for step in &mut plan.steps {
            step.text_controls = step
                .operation
                .prepared
                .as_ref()
                .map(|p| {
                    p.text_inputs
                        .iter()
                        .filter_map(|b| {
                            self.text
                                .input_slot(step.node, b.input)
                                .map(|slot| (b.property.clone(), slot, None))
                        })
                        .collect()
                })
                .unwrap_or_default();
            step.prepared_controls = step
                .operation
                .prepared
                .as_ref()
                .map(|p| {
                    p.scalar_inputs
                        .iter()
                        .filter_map(|p| {
                            self.parameter(step.node, p.parameter)
                                .map(|v| (p.property.clone(), v, None, p.step.map(|s| s as f32)))
                        })
                        .collect()
                })
                .unwrap_or_default();
            step.prepared_revision = self.text_revision << 32;
            step.controls = bindings.get(&step.node).cloned().unwrap_or_default();
            step.uniform_controls = step
                .operation
                .parameter_slots
                .iter()
                .enumerate()
                .filter_map(|(slot, index)| {
                    index
                        .and_then(|p| self.parameter(step.node, p))
                        .filter(|v| v.source.is_some())
                        .map(|v| (slot, v))
                })
                .collect();
        }
    }
    pub fn modulate(&self, plan: &mut crate::compiler::RenderPlan) {
        for step in &mut plan.steps {
            for (property, value, last, step_size) in &mut step.prepared_controls {
                let value = value.read(&self.values);
                let value = step_size.map_or(value, |step| (value / step).round() * step);
                if *last != Some(value) {
                    if let Some(p) = &mut step.operation.prepared {
                        p.properties.insert(property.clone(), value.to_string());
                    }
                    step.prepared_revision = step.prepared_revision.wrapping_add(1);
                    *last = Some(value);
                }
            }
            for (property, slot, last) in &mut step.text_controls {
                let value = self.text.value(*slot);
                if last.as_ref().is_none_or(|last| {
                    !std::sync::Arc::ptr_eq(last, value) && last.as_ref() != value.as_ref()
                }) {
                    if let Some(p) = &mut step.operation.prepared {
                        p.properties.insert(property.clone(), value.to_string());
                    }

                    step.prepared_revision = step.prepared_revision.wrapping_add(1);
                }
                if last
                    .as_ref()
                    .is_none_or(|last| !std::sync::Arc::ptr_eq(last, value))
                {
                    *last = Some(value.clone());
                }
            }
            for &(slot, value) in &step.uniform_controls {
                step.operation.parameters[slot] = value.read(&self.values);
            }
            for &(index, value) in &step.controls {
                let current = value.read(&self.values);
                if let Some(binding) = step.operation.media
                    && let Some(playback) = step.playback.as_mut()
                {
                    if binding.speed == index {
                        playback.speed = current;
                    }
                    if binding.looping == Some(index) {
                        playback.window.looping = current >= 0.5;
                    }
                    if binding.start == Some(index) {
                        playback.window.start = current;
                    }
                    if binding.end == Some(index) {
                        playback.window.end = current;
                    }
                    if binding.position == index {
                        playback.position = current;
                    }
                }
            }
        }
    }
}
