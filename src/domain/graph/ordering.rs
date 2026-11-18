use super::*;
use std::collections::VecDeque;
impl Graph {
    /// Kahn's algorithm walks registered destinations directly: O(V + E).
    pub fn topological_order(&self) -> Result<Vec<usize>, GraphError> {
        let mut indegrees = vec![0; self.nodes.len()];
        for ports in &self.ports {
            for output in &ports.outputs {
                for input in &output.destinations {
                    indegrees[self.index(input.node)?] += 1;
                }
            }
        }
        let mut ready: VecDeque<_> = (0..self.nodes.len())
            .filter(|&i| indegrees[i] == 0)
            .collect();
        let mut order = Vec::with_capacity(self.nodes.len());
        while let Some(index) = ready.pop_front() {
            order.push(index);
            for output in &self.ports[index].outputs {
                for input in &output.destinations {
                    let next = self.index(input.node)?;
                    indegrees[next] -= 1;
                    if indegrees[next] == 0 {
                        ready.push_back(next);
                    }
                }
            }
        }
        if order.len() != self.nodes.len() {
            return Err(GraphError::Cycle);
        }
        Ok(order)
    }
}
