use super::*;

impl Session {
    pub fn add_node(
        &mut self,
        type_id: impl Into<vjay_domain::node::NodeTypeId>,
        position: Point,
    ) -> PortResult<NodeId> {
        let type_id = type_id.into();
        let capabilities = self.nodes.get(&type_id)?.descriptor().capabilities;
        let id = self
            .project
            .graph
            .add(type_id, self.nodes.as_ref())
            .map_err(|e| e.to_string())?;
        self.project
            .positions
            .push(NodePosition { node: id, position });
        if capabilities.program_output.is_some() && self.project.output.is_none() {
            self.project.output = Some(id);
        }
        if capabilities
            .settings
            .contains(&vjay_domain::capabilities::SettingEditor::Midi)
        {
            self.ensure_midi();
        }
        self.invalidate(true);
        Ok(id)
    }
    pub fn remove_node(&mut self, id: NodeId) -> PortResult<()> {
        self.remove_nodes(&[id])
    }
    pub fn remove_nodes(&mut self, ids: &[NodeId]) -> PortResult<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let ids: std::collections::HashSet<_> = ids.iter().copied().collect();
        self.project
            .graph
            .remove_many(&ids)
            .map_err(|e| e.to_string())?;
        if self.midi.learning.is_some_and(|id| ids.contains(&id)) {
            self.midi.learning = None;
        }
        if self.project.output.is_some_and(|id| ids.contains(&id)) {
            self.project.output = None;
        }
        self.project.positions.retain(|p| !ids.contains(&p.node));
        self.project.controls.retain(|p| !ids.contains(&p.node));
        self.sync_taps.retain(|node, _| !ids.contains(node));
        let assets: std::collections::HashSet<_> = self
            .project
            .graph
            .nodes()
            .iter()
            .filter_map(|n| n.asset)
            .collect();
        self.project
            .assets
            .retain(|asset| assets.contains(&asset.id));
        self.invalidate(true);
        Ok(())
    }
    pub fn move_nodes(&mut self, origins: &std::collections::HashMap<NodeId, Point>, delta: Point) {
        if !delta.x.is_finite() || !delta.y.is_finite() {
            return;
        }
        for item in &mut self.project.positions {
            if let Some(origin) = origins.get(&item.node) {
                item.position = Point {
                    x: (origin.x + delta.x).clamp(-1e6, 1e6),
                    y: (origin.y + delta.y).clamp(-1e6, 1e6),
                };
            }
        }
    }
    pub fn connect(&mut self, connection: Connection) -> PortResult<()> {
        self.project
            .graph
            .connect(connection)
            .map_err(|e| e.to_string())?;
        self.invalidate(true);
        Ok(())
    }
    pub fn disconnect(&mut self, node: NodeId, input: usize) {
        self.project.graph.disconnect(node, input);
        self.invalidate(true);
    }
    pub fn reconnect(&mut self, previous: Connection, next: Connection) -> PortResult<()> {
        self.project
            .graph
            .reconnect(previous, next)
            .map_err(|e| e.to_string())?;
        self.invalidate(true);
        Ok(())
    }
    pub fn set_workspace(&mut self, layout: crate::workspace::WorkspaceLayout) -> PortResult<()> {
        if !layout.is_valid() {
            return Err("Invalid workspace layout".into());
        }
        self.project.workspace = layout;
        self.modified.set(true);
        Ok(())
    }
    pub fn set_parameter(&mut self, node: NodeId, parameter: usize, value: f32) -> PortResult<()> {
        let trigger = self.project.graph.node(node).is_ok_and(|record| {
            self.descriptor(&record.type_id)
                .capabilities
                .momentary_parameter
                == Some(parameter)
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
        self.invalidate(false);
        Ok(())
    }
    pub fn set_extent(&mut self, extent: Extent) -> PortResult<()> {
        Extent::new(extent.width, extent.height)?;
        if self.extent != extent {
            self.extent = extent;
            self.invalidate(true);
        }
        Ok(())
    }
    pub fn move_node(&mut self, node: NodeId, position: Point) {
        self.modified.set(true);
        if let Some(p) = self.project.positions.iter_mut().find(|p| p.node == node) {
            p.position = position;
        }
    }
    pub fn pin_control(&mut self, node: NodeId, parameter: usize) -> PortResult<()> {
        self.modified.set(true);
        let record = self.project.graph.node(node).map_err(|e| e.to_string())?;
        if parameter >= self.schema(node).parameters.len() {
            return Err("Invalid control parameter".into());
        }
        if self.project.controls.len() >= vjay_domain::graph::MAX_NODES * 3 {
            return Err("Control limit exceeded".into());
        }
        let id = match self.project.controls.iter().map(|c| c.id).max() {
            Some(id) => id
                .checked_add(1)
                .filter(|id| *id != u64::MAX)
                .ok_or("Control IDs exhausted")?,
            None => 0,
        };
        let widget = if self.schema(node).capabilities.momentary_parameter == Some(parameter) {
            WidgetKind::Button
        } else if parameter == 0 {
            self.descriptor(&record.type_id)
                .capabilities
                .widget
                .unwrap_or_default()
        } else {
            WidgetKind::Slider
        };
        self.project.controls.push(ControlBinding {
            widget,
            id,
            node,
            parameter,
            position: Point {
                x: 12.0,
                y: 12.0 + (id % 6) as f32 * 108.0,
            },
            size: Point { x: 240.0, y: 100.0 },
            ..Default::default()
        });
        Ok(())
    }
    pub fn update_control_layout(&mut self, id: u64, position: Point, size: Point) {
        self.modified.set(true);
        if let Some(control) = self.project.controls.iter_mut().find(|c| c.id == id) {
            control.position = position;
            control.size = size;
        }
    }
    pub fn remove_control(&mut self, id: u64) {
        self.modified.set(true);
        self.project.controls.retain(|c| c.id != id);
    }
}
