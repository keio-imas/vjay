use super::*;

impl Graph {
    /// Flat read-only view for UI and the existing project wire format.
    /// Registrations live only in output ports; no second editable edge list.
    pub fn connections(&self) -> impl Iterator<Item = Connection> + Clone + '_ {
        self.nodes
            .iter()
            .zip(&self.ports)
            .flat_map(|(node, ports)| {
                ports
                    .outputs
                    .iter()
                    .enumerate()
                    .flat_map(move |(output, port)| {
                        port.destinations.iter().map(move |input| Connection {
                            from: node.id,
                            output,
                            to: input.node,
                            input: input.port,
                        })
                    })
            })
    }
    pub fn connection_count(&self) -> usize {
        self.connection_count
    }
    pub fn has_connection(&self, edge: Connection) -> bool {
        self.destinations(OutputPortRef {
            node: edge.from,
            port: edge.output,
        })
        .is_ok_and(|inputs| {
            inputs.contains(&InputPortRef {
                node: edge.to,
                port: edge.input,
            })
        })
    }
    pub fn register(
        &mut self,
        output: OutputPortRef,
        input: InputPortRef,
    ) -> Result<(), GraphError> {
        self.connect(Connection {
            from: output.node,
            output: output.port,
            to: input.node,
            input: input.port,
        })
    }
    /// Replacing an input is transactional, including destination list order.
    pub fn connect(&mut self, edge: Connection) -> Result<(), GraphError> {
        let source = self.index(edge.from)?;
        let target = self.index(edge.to)?;
        let outputs = &self.ports[source].outputs;
        if outputs.is_empty() {
            return Err(GraphError::TerminalSource);
        }
        let signal = outputs
            .get(edge.output)
            .ok_or(GraphError::InvalidPort)?
            .signal;
        let input = *self.ports[target]
            .inputs
            .get(edge.input)
            .ok_or(GraphError::InvalidPort)?;
        if signal != input {
            return Err(GraphError::PortType);
        }
        let destination = InputPortRef {
            node: edge.to,
            port: edge.input,
        };
        let previous = self.find_input(destination);
        if previous.is_some_and(|(node, port, _)| node == source && port == edge.output) {
            return Ok(());
        }
        if let Some((node, port, index)) = previous {
            self.ports[node].outputs[port].destinations.remove(index);
        }
        self.ports[source].outputs[edge.output]
            .destinations
            .push(destination);
        if let Err(error) = self.topological_order() {
            self.ports[source].outputs[edge.output].destinations.pop();
            if let Some((node, port, index)) = previous {
                self.ports[node].outputs[port]
                    .destinations
                    .insert(index, destination);
            }
            return Err(error);
        }
        self.connection_count += usize::from(previous.is_none());
        Ok(())
    }
    fn find_input(&self, input: InputPortRef) -> Option<(usize, usize, usize)> {
        for (node, ports) in self.ports.iter().enumerate() {
            for (port, output) in ports.outputs.iter().enumerate() {
                if let Some(index) = output
                    .destinations
                    .iter()
                    .position(|target| *target == input)
                {
                    return Some((node, port, index));
                }
            }
        }
        None
    }
    pub fn disconnect(&mut self, to: NodeId, input: usize) {
        if let Some((node, port, index)) = self.find_input(InputPortRef {
            node: to,
            port: input,
        }) {
            self.ports[node].outputs[port].destinations.remove(index);
            self.connection_count -= 1;
        }
    }
    pub fn reconnect(&mut self, previous: Connection, next: Connection) -> Result<(), GraphError> {
        if !self.has_connection(previous) {
            return Err(GraphError::InvalidPort);
        }
        if previous == next {
            return Ok(());
        }
        let source = self.index(previous.from)?;
        let input = InputPortRef {
            node: previous.to,
            port: previous.input,
        };
        let position = self.ports[source].outputs[previous.output]
            .destinations
            .iter()
            .position(|target| *target == input)
            .expect("verified registration");
        self.ports[source].outputs[previous.output]
            .destinations
            .remove(position);
        self.connection_count -= 1;
        if let Err(error) = self.connect(next) {
            self.ports[source].outputs[previous.output]
                .destinations
                .insert(position, input);
            self.connection_count += 1;
            return Err(error);
        }
        Ok(())
    }
}
