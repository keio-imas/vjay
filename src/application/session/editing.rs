use super::*;
use vjay_domain::graph::{Graph, GraphError};

impl Session {
    pub fn add_node(
        &mut self,
        type_id: impl Into<vjay_domain::node::NodeTypeId>,
        position: Point,
    ) -> PortResult<NodeId> {
        let type_id = type_id.into();
        let capabilities = self.nodes.get(&type_id)?.descriptor().capabilities;
        self.project.graph.reserve_ids_until(self.next_node_id);
        let composite = self.nodes.get(&type_id)?.composite();
        let boundary = self
            .nodes
            .get(&type_id)?
            .boundary(&self.nodes.get(&type_id)?.default_settings())
            .is_some();
        let id = self
            .project
            .graph
            .add(type_id, self.nodes.as_ref())
            .map_err(|e| e.to_string())?;
        self.next_node_id = id.0.checked_add(1).ok_or("Node IDs exhausted")?;
        if composite {
            self.project.subpatches.push(Subpatch {
                node: id,
                source: None,
                project: Box::new(Project::blank()),
            });
        }
        if boundary {
            let mut settings = self
                .project
                .graph
                .node(id)
                .map_err(|e| e.to_string())?
                .settings
                .clone();
            settings
                .properties
                .insert("port.key".into(), format!("port-{}", id.0));
            self.project
                .graph
                .set_settings(id, settings, self.nodes.as_ref())
                .map_err(|e| e.to_string())?;
        }
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
        self.project.subpatches.retain(|p| !ids.contains(&p.node));
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
        self.edit_connections(|graph| graph.connect(connection))
    }
    fn edit_connections(
        &mut self,
        edit: impl FnOnce(&mut Graph) -> Result<(), GraphError>,
    ) -> PortResult<()> {
        if self.ancestors.is_empty() && self.project.subpatches.is_empty() {
            edit(&mut self.project.graph).map_err(|e| e.to_string())?;
        } else {
            let mut candidate = self.project.graph.clone();
            edit(&mut candidate).map_err(|e| e.to_string())?;
            let previous = std::mem::replace(&mut self.project.graph, candidate);
            if let Err(e) = self
                .root_snapshot()
                .and_then(|root| crate::composition::expand(&root, self.nodes.as_ref()))
            {
                self.project.graph = previous;
                return Err(e);
            }
        }
        self.invalidate(true);
        Ok(())
    }
    pub fn disconnect(&mut self, node: NodeId, input: usize) {
        self.project.graph.disconnect(node, input);
        self.invalidate(true);
    }
    pub fn reconnect(&mut self, previous: Connection, next: Connection) -> PortResult<()> {
        self.edit_connections(|graph| graph.reconnect(previous, next))
    }
    pub fn set_workspace(&mut self, layout: crate::workspace::WorkspaceLayout) -> PortResult<()> {
        if !layout.is_valid() {
            return Err("Invalid workspace layout".into());
        }
        self.project.workspace = layout;
        self.modified.set(true);
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
            display: Default::default(),
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
