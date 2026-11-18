use super::*;
use crate::control_io::{ControlEvent, InputSettings};
use vjay_domain::port::SignalType;

impl Session {
    pub fn add_control(&mut self, widget: WidgetKind) -> PortResult<u64> {
        let desired = if matches!(widget, WidgetKind::Meter | WidgetKind::Label) {
            WidgetKind::Label
        } else {
            widget
        };
        let type_id = self
            .catalog()
            .iter()
            .find(|d| d.capabilities.widget == Some(desired))
            .map(|d| d.type_id)
            .ok_or("No registered node supports this control")?;
        let count = self.project.graph.nodes().len();
        let node = self.add_node(
            type_id,
            Point {
                x: 40.0 + (count % 4) as f32 * 260.0,
                y: 60.0 + (count / 4) as f32 * 180.0,
            },
        )?;
        self.pin_control(node, 0)?;
        let control = self
            .project
            .controls
            .last_mut()
            .ok_or("Control creation failed")?;
        control.widget = widget;
        if matches!(widget, WidgetKind::Meter | WidgetKind::Label) {
            control.target = ControlTarget::Output;
        }
        if widget == WidgetKind::XYPad {
            control.size = Point { x: 180.0, y: 180.0 };
        }
        if widget == WidgetKind::Files {
            control.size = Point { x: 300.0, y: 280.0 };
        }
        if widget == WidgetKind::Knob {
            control.size = Point { x: 140.0, y: 150.0 };
        }
        Ok(control.id)
    }
    pub fn pin_files(&mut self, node: NodeId) -> PortResult<()> {
        let parameter = self
            .project
            .graph
            .schema(node)
            .map_err(|e| e.to_string())?
            .capabilities
            .asset_selection_parameter
            .ok_or("Node has no file selection")?;
        self.pin_control(node, parameter)?;
        if let Some(c) = self.project.controls.last_mut() {
            c.widget = WidgetKind::Files;
            c.size = Point { x: 300.0, y: 280.0 };
        }
        Ok(())
    }
    pub fn pin_output(&mut self, node: NodeId, output: usize) -> PortResult<()> {
        if !matches!(
            self.port_signal(node, output),
            Some(SignalType::Scalar | SignalType::Text)
        ) {
            return Err("Choose a scalar output".into());
        }
        if self.project.controls.len() >= vjay_domain::graph::MAX_NODES * 3 {
            return Err("Control limit exceeded".into());
        }
        let id = self
            .project
            .controls
            .iter()
            .map(|c| c.id)
            .max()
            .map_or(Some(0), |id| id.checked_add(1))
            .filter(|id| *id != u64::MAX)
            .ok_or("Control IDs exhausted")?;
        self.modified.set(true);
        self.project.controls.push(ControlBinding {
            id,
            node,
            parameter: output,
            widget: if self.port_signal(node, output) == Some(SignalType::Text) {
                WidgetKind::Label
            } else {
                WidgetKind::Meter
            },
            target: ControlTarget::Output,
            ..Default::default()
        });
        Ok(())
    }
    pub fn configure_control(&mut self, control: ControlBinding) -> PortResult<()> {
        let node = self
            .project
            .graph
            .node(control.node)
            .map_err(|e| e.to_string())?;
        if !crate::project::valid_control(&control, node, self.nodes.as_ref()) {
            return Err("Invalid control binding or range".into());
        }
        let current = self
            .project
            .controls
            .iter_mut()
            .find(|c| c.id == control.id)
            .ok_or("Control no longer exists")?;
        *current = control;
        self.modified.set(true);
        Ok(())
    }
    pub fn control_value(&self, control: &ControlBinding) -> f32 {
        match control.target {
            ControlTarget::Parameter => self
                .signals
                .value(control.node, control.parameter)
                .or_else(|| {
                    self.project
                        .graph
                        .node(control.node)
                        .ok()?
                        .parameters
                        .get(control.parameter)
                        .copied()
                })
                .unwrap_or(0.0),
            ControlTarget::Output => self
                .signals
                .output(control.node, control.parameter)
                .unwrap_or(0.0),
        }
    }
    pub fn control_range(&self, control: &ControlBinding) -> [f32; 2] {
        if let Some(range) = control.range {
            return range;
        }
        if let Ok(node) = self.project.graph.node(control.node) {
            if control.parameter == 0
                && control.target == ControlTarget::Parameter
                && let Some([min, max]) = self.schema(node.id).capabilities.range_parameters
                && node.parameters[min] < node.parameters[max]
            {
                return [node.parameters[min], node.parameters[max]];
            }
            if control.target == ControlTarget::Parameter
                && let Some(p) = self.schema(node.id).parameters.get(control.parameter)
            {
                return [p.min, p.max];
            }
        }
        [0.0, 1.0]
    }
    pub fn parameter_driven(&self, node: NodeId, index: usize) -> bool {
        self.signals
            .parameter(node, index)
            .is_some_and(|reference| reference.source.is_some())
    }
    pub fn text_value(&self, node: NodeId, port: usize) -> Option<&str> {
        self.signals.text_output(node, port)
    }
    pub fn signal_value(&self, node: NodeId, output: usize) -> Option<f32> {
        self.signals.output(node, output)
    }
    pub fn input_event(&mut self, event: ControlEvent) {
        self.signals.events.push(event);
    }
    pub fn tap_tempo(&mut self, node: Option<NodeId>) {
        let seconds = self.clock.seconds();
        self.input_event(ControlEvent::Tap { node, seconds });
    }
    pub fn configure_inputs(&mut self, settings: InputSettings) -> PortResult<()> {
        self.input_status = self.control_io.configure(&settings)?;
        if self.input_settings.midi != settings.midi {
            self.signals.clear_midi();
            self.midi.learning = None;
        }
        self.input_settings = settings;
        Ok(())
    }
    pub fn set_program(&mut self, node: NodeId) -> PortResult<()> {
        let record = self.project.graph.node(node).map_err(|e| e.to_string())?;
        if self
            .descriptor(&record.type_id)
            .capabilities
            .program_output
            .is_none()
        {
            return Err("Choose a Program output node".into());
        }
        self.project.output = Some(node);
        self.invalidate(true);
        Ok(())
    }
    pub fn insert_node(
        &mut self,
        edge: Connection,
        type_id: &str,
        position: Point,
    ) -> PortResult<NodeId> {
        if !self.project.graph.has_connection(edge) {
            return Err("Connection no longer exists".into());
        }
        let source = self
            .project
            .graph
            .node(edge.from)
            .map_err(|e| e.to_string())?;
        let signal = self
            .schema(source.id)
            .outputs
            .get(edge.output)
            .ok_or("Unknown output")?
            .signal;
        let descriptor = self.nodes.get(type_id)?.descriptor();
        let input = (0..descriptor.input_count())
            .find(|&port| descriptor.input(port).is_some_and(|p| p.signal == signal))
            .ok_or("Node has no compatible input")?;
        let output = descriptor
            .outputs
            .iter()
            .position(|port| port.signal == signal)
            .ok_or("Node has no compatible output")?;
        let mut candidate = self.project.graph.clone();
        let id = candidate
            .add(type_id, self.nodes.as_ref())
            .map_err(|e| e.to_string())?;
        candidate.disconnect(edge.to, edge.input);
        candidate
            .connect(Connection {
                from: edge.from,
                output: edge.output,
                to: id,
                input,
            })
            .map_err(|e| e.to_string())?;
        candidate
            .connect(Connection {
                from: id,
                output,
                to: edge.to,
                input: edge.input,
            })
            .map_err(|e| e.to_string())?;
        self.project.graph = candidate;
        self.project
            .positions
            .push(NodePosition { node: id, position });
        self.invalidate(true);
        Ok(id)
    }
    pub fn can_insert(&self, edge: Connection, type_id: &str) -> bool {
        let Ok(source) = self.project.graph.node(edge.from) else {
            return false;
        };
        let Some(port) = self.schema(source.id).outputs.get(edge.output) else {
            return false;
        };
        let Ok(node) = self.nodes.get(type_id) else {
            return false;
        };
        let descriptor = node.descriptor();
        (0..descriptor.input_count())
            .any(|i| descriptor.input(i).is_some_and(|p| p.signal == port.signal))
            && descriptor.outputs.iter().any(|p| p.signal == port.signal)
    }
    pub fn is_control_node(&self, id: NodeId) -> bool {
        self.project.graph.node(id).is_ok_and(|node| {
            self.schema(node.id)
                .outputs
                .iter()
                .any(|port| port.signal == SignalType::Scalar)
        })
    }
    pub fn port_signal(&self, node: NodeId, output: usize) -> Option<SignalType> {
        let node = self.project.graph.node(node).ok()?;
        self.schema(node.id).outputs.get(output).map(|p| p.signal)
    }
}
