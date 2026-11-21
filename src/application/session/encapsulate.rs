use super::*;
use std::collections::{HashMap, HashSet};
use vjay_domain::{
    composition::{Direction, InterfacePort},
    graph::NodeRecord,
    port::SignalType,
};
impl Session {
    pub fn encapsulate(&mut self, selected: &[NodeId]) -> PortResult<NodeId> {
        let ids = selected.iter().copied().collect::<HashSet<_>>();
        if ids.is_empty() {
            return Err("Select nodes to encapsulate".into());
        }
        if self.project.output.is_some_and(|id| ids.contains(&id)) {
            return Err("Keep the Program Output outside the subpatch".into());
        }
        for id in &ids {
            let node = self.project.graph.node(*id).map_err(|e| e.to_string())?;
            if self
                .nodes
                .get(&node.type_id)?
                .boundary(&node.settings)
                .is_some()
            {
                return Err("Keep In / Out nodes at their interface level".into());
            }
        }
        let mut parent = self.project.clone();
        let mut child = Project::blank();
        let incoming = parent
            .graph
            .connections()
            .filter(|e| !ids.contains(&e.from) && ids.contains(&e.to))
            .collect::<Vec<_>>();
        let outgoing = parent
            .graph
            .connections()
            .filter(|e| ids.contains(&e.from) && !ids.contains(&e.to))
            .collect::<Vec<_>>();
        let internal = parent
            .graph
            .connections()
            .filter(|e| ids.contains(&e.from) && ids.contains(&e.to))
            .collect::<Vec<_>>();
        for n in parent.graph.nodes().iter().filter(|n| ids.contains(&n.id)) {
            child
                .graph
                .insert(n.clone(), self.nodes.as_ref())
                .map_err(|e| e.to_string())?;
        }
        for e in internal {
            child.graph.connect(e).map_err(|e| e.to_string())?;
        }
        child.positions = parent
            .positions
            .iter()
            .filter(|p| ids.contains(&p.node))
            .cloned()
            .collect();
        let left = child
            .positions
            .iter()
            .map(|p| p.position.x)
            .fold(f32::INFINITY, f32::min);
        let top = child
            .positions
            .iter()
            .map(|p| p.position.y)
            .fold(f32::INFINITY, f32::min);
        for p in &mut child.positions {
            p.position.x = p.position.x - left + 260.0;
            p.position.y = p.position.y - top + 100.0;
        }
        child.controls = parent
            .controls
            .iter()
            .filter(|c| ids.contains(&c.node))
            .cloned()
            .collect();
        child.subpatches = parent
            .subpatches
            .iter()
            .filter(|p| ids.contains(&p.node))
            .cloned()
            .collect();
        let asset_ids = child
            .graph
            .nodes()
            .iter()
            .filter_map(|n| n.asset)
            .collect::<HashSet<_>>();
        child.assets = parent
            .assets
            .iter()
            .filter(|a| asset_ids.contains(&a.id))
            .cloned()
            .collect();
        parent.graph.remove_many(&ids).map_err(|e| e.to_string())?;
        parent.positions.retain(|p| !ids.contains(&p.node));
        parent.controls.retain(|c| !ids.contains(&c.node));
        parent.subpatches.retain(|p| !ids.contains(&p.node));
        let ty = self
            .catalog()
            .iter()
            .find(|d| self.nodes.get(d.type_id).is_ok_and(|n| n.composite()))
            .ok_or("No subpatch node registered")?
            .type_id;
        parent.graph.reserve_ids_until(self.next_node_id);
        let instance = parent
            .graph
            .add(ty, self.nodes.as_ref())
            .map_err(|e| e.to_string())?;
        let mut next = instance.0 + 1;
        let mut ports_in = HashMap::new();
        let mut ports_out = HashMap::new();
        let mut external = Vec::new();
        for (edges, direction, ports) in [
            (&incoming, Direction::In, &mut ports_in),
            (&outgoing, Direction::Out, &mut ports_out),
        ] {
            for edge in edges {
                let key = (edge.from, edge.output);
                let index = if let Some(i) = ports.get(&key) {
                    *i
                } else {
                    let index = ports.len();
                    let source = self
                        .project
                        .graph
                        .schema(edge.from)
                        .map_err(|e| e.to_string())?
                        .outputs
                        .get(edge.output)
                        .ok_or("Unknown source port")?;
                    let info = InterfacePort {
                        key: format!("port-{next}"),
                        name: source.name.clone(),
                        signal: source.signal,
                        delayed: source.delayed,
                    };
                    let endpoint =
                        self.boundary_record(direction, NodeId(next), index as u32 + 1, &info)?;
                    next += 1;
                    let id = endpoint.id;
                    child
                        .graph
                        .insert(endpoint, self.nodes.as_ref())
                        .map_err(|e| e.to_string())?;
                    child.positions.push(NodePosition {
                        node: id,
                        position: Point {
                            x: if direction == Direction::In {
                                20.0
                            } else {
                                760.0
                            },
                            y: 100.0 + index as f32 * 90.0,
                        },
                    });
                    if direction == Direction::Out {
                        child
                            .graph
                            .connect(Connection {
                                from: edge.from,
                                output: edge.output,
                                to: id,
                                input: 0,
                            })
                            .map_err(|e| e.to_string())?;
                    }
                    ports.insert(key, index);
                    index
                };
                let id = child
                    .graph
                    .nodes()
                    .iter()
                    .find(|n| {
                        self.nodes
                            .get(&n.type_id)
                            .ok()
                            .and_then(|d| d.boundary(&n.settings))
                            .is_some_and(|b| {
                                b.direction == direction && b.order == index as u32 + 1
                            })
                    })
                    .ok_or("Missing boundary")?
                    .id;
                if direction == Direction::In {
                    child
                        .graph
                        .connect(Connection {
                            from: id,
                            output: 0,
                            to: edge.to,
                            input: edge.input,
                        })
                        .map_err(|e| e.to_string())?;
                    if !external
                        .iter()
                        .any(|e: &Connection| e.to == instance && e.input == index)
                    {
                        external.push(Connection {
                            from: edge.from,
                            output: edge.output,
                            to: instance,
                            input: index,
                        });
                    }
                } else {
                    external.push(Connection {
                        from: instance,
                        output: index,
                        to: edge.to,
                        input: edge.input,
                    });
                }
            }
        }
        let interface = crate::composition::interface::of(&child, self.nodes.as_ref())?;
        let mut settings = parent
            .graph
            .node(instance)
            .map_err(|e| e.to_string())?
            .settings
            .clone();
        settings.label = "Subpatch".into();
        settings.preview = interface
            .outputs
            .iter()
            .any(|p| p.signal == SignalType::Texture);
        settings.interface = Some(interface);
        parent
            .graph
            .set_settings(instance, settings, self.nodes.as_ref())
            .map_err(|e| e.to_string())?;
        for e in external {
            parent.graph.connect(e).map_err(|e| e.to_string())?;
        }
        parent.positions.push(NodePosition {
            node: instance,
            position: Point { x: left, y: top },
        });
        parent.subpatches.push(Subpatch {
            node: instance,
            source: None,
            project: Box::new(child),
        });
        self.project = parent;
        self.next_node_id = next;
        self.invalidate(true);
        Ok(instance)
    }
    fn boundary_record(
        &self,
        direction: Direction,
        id: NodeId,
        order: u32,
        p: &InterfacePort,
    ) -> PortResult<NodeRecord> {
        let definition = self
            .nodes
            .catalog()
            .iter()
            .find_map(|d| {
                let n = self.nodes.get(d.type_id).ok()?;
                n.boundary(&n.default_settings())
                    .filter(|b| b.direction == direction)
                    .map(|_| n)
            })
            .ok_or("No boundary node registered")?;
        let mut settings = definition.default_settings();
        settings.port_version = definition.port_version();
        settings.properties.extend([
            ("port.key".into(), p.key.clone()),
            ("name".into(), p.name.clone()),
            ("order".into(), order.to_string()),
            (
                "type".into(),
                match p.signal {
                    SignalType::Texture => "Image",
                    SignalType::Scalar => "Value",
                    SignalType::Audio => "Audio",
                    SignalType::Text => "Text",
                    SignalType::File => "File",
                }
                .into(),
            ),
        ]);
        Ok(NodeRecord {
            id,
            type_id: definition.descriptor().type_id.into(),
            settings,
            parameters: Vec::new(),
            asset: None,
        })
    }
}
