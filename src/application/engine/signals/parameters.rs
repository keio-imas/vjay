use super::*;
impl SignalEngine {
    /// Existing scalar/audio/media references keep reading this preallocated cell.
    pub fn update_parameter(&mut self, node: NodeId, parameter: usize, value: f32) -> bool {
        let Some(reference) = self.parameters.get_mut(&(node, parameter)) else {
            return false;
        };
        let Some(slot) = reference.constant_slot.and_then(|i| self.values.get_mut(i)) else {
            return false;
        };
        *slot = value;
        reference.value = value;
        true
    }
}
