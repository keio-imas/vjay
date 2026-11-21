use super::*;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Default)]
pub(super) struct NodeSelection {
    pub ids: HashSet<NodeId>,
    primary: Option<NodeId>,
}
impl NodeSelection {
    pub fn set(&mut self, ids: &[NodeId]) {
        self.ids = ids.iter().copied().collect();
        self.primary = ids.first().copied();
    }
    pub fn primary(&self) -> Option<NodeId> {
        self.primary
    }
    pub fn len(&self) -> usize {
        self.ids.len()
    }
    pub fn contains(&self, id: NodeId) -> bool {
        self.ids.contains(&id)
    }
    pub fn clear(&mut self) {
        self.ids.clear();
        self.primary = None;
    }
    pub fn only(&mut self, id: NodeId) {
        self.ids.clear();
        self.ids.insert(id);
        self.primary = Some(id);
    }
    fn toggle(&mut self, id: NodeId) {
        if !self.ids.remove(&id) {
            self.ids.insert(id);
            self.primary = Some(id);
        } else if self.primary == Some(id) {
            self.primary = self.ids.iter().min_by_key(|n| n.0).copied();
        }
    }
}
pub(super) struct Marquee {
    start: Position,
    current: Position,
    before: NodeSelection,
    additive: bool,
}
pub(super) struct NodeMove {
    origins: HashMap<NodeId, Position>,
    copies: Vec<NodeId>,
    before: NodeSelection,
    copy_pending: bool,
    pointer: Position,
}
mod inspector;
mod painting;

impl Editor {
    pub(super) fn select_node(
        &mut self,
        id: NodeId,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let add = event.modifiers.shift || event.modifiers.platform || event.modifiers.control;
        if add {
            self.selection.toggle(id);
        } else if !self.selection.contains(id) {
            self.selection.only(id);
        }
        self.selected_control = None;
        window.focus(&self.focus, cx);
        cx.stop_propagation();
        cx.notify();
    }
    pub(super) fn begin_node_move(
        &mut self,
        id: NodeId,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_node(id, event, window, cx);
        if !self.selection.contains(id) {
            return;
        }
        let before = self.selection.clone();
        let copies = Vec::new();
        let origins = self
            .session()
            .project()
            .positions
            .iter()
            .filter(|p| self.selection.contains(p.node))
            .map(|p| (p.node, p.position))
            .collect();
        self.node_move = Some(NodeMove {
            origins,
            copies,
            before,
            copy_pending: event.modifiers.alt,
            pointer: position(event.position),
        });
        self.marquee = None;
        self.cable = None;
        self.drag = Some(Drag::Nodes);
    }
    pub(super) fn world_pointer(&self, pointer: Position) -> Option<Position> {
        let bounds = self.patch_bounds.get()?;
        Some(Position {
            x: (pointer.x - f32::from(bounds.left()) - self.pan.x) / self.zoom,
            y: (pointer.y - f32::from(bounds.top()) - self.pan.y) / self.zoom,
        })
    }
    pub(super) fn begin_marquee(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(start) = self.world_pointer(position(event.position)) else {
            return;
        };
        let additive = event.modifiers.shift || event.modifiers.platform || event.modifiers.control;
        let before = self.selection.clone();
        if !additive {
            self.selection.clear();
        }
        self.marquee = Some(Marquee {
            start,
            current: start,
            before,
            additive,
        });
        self.node_move = None;
        if let Some(Drag::MediaNudge(node)) = self.drag {
            let _ = self.session_mut().nudge_clip(node, 0.0);
        }
        self.drag = None;
        self.cable = None;
        self.selected_control = None;
        window.focus(&self.focus, cx);
        cx.stop_propagation();
        cx.notify();
    }
    pub(super) fn update_marquee(&mut self, pointer: Position) {
        let Some(current) = self.world_pointer(pointer) else {
            return;
        };
        let Some(marquee) = self.marquee.as_mut() else {
            return;
        };
        marquee.current = current;
        let (start, before, additive) = (marquee.start, marquee.before.clone(), marquee.additive);
        let mut selected = if additive {
            before
        } else {
            NodeSelection::default()
        };
        if (current.x - start.x).abs() + (current.y - start.y).abs() >= 3.0 {
            let (left, right, top, bottom) = (
                start.x.min(current.x),
                start.x.max(current.x),
                start.y.min(current.y),
                start.y.max(current.y),
            );
            for p in &self.session().project().positions {
                let Ok(node) = self.session().project().graph.node(p.node) else {
                    continue;
                };
                let height = self.node_height(node);
                if p.position.x <= right
                    && p.position.x + patch::NODE_WIDTH >= left
                    && p.position.y <= bottom
                    && p.position.y + height >= top
                {
                    selected.ids.insert(p.node);
                }
            }
        }
        if selected
            .primary
            .is_none_or(|id| !selected.ids.contains(&id))
        {
            selected.primary = selected.ids.iter().min_by_key(|n| n.0).copied();
        }
        self.selection = selected;
    }
    pub(super) fn move_selected_nodes(&mut self, pointer: Position) {
        let Some(mut movement) = self.node_move.take() else {
            return;
        };
        let delta = Position {
            x: (pointer.x - movement.pointer.x) / self.zoom,
            y: (pointer.y - movement.pointer.y) / self.zoom,
        };
        if movement.copy_pending {
            if (delta.x.abs() + delta.y.abs()) * self.zoom < 3.0 {
                self.node_move = Some(movement);
                return;
            }
            let ids: Vec<_> = self.selection.ids.iter().copied().collect();
            let result = self.session().copy_nodes(&ids).and_then(|fragment| {
                let anchor = fragment.positions.iter().fold(
                    Position {
                        x: f32::INFINITY,
                        y: f32::INFINITY,
                    },
                    |a, p| Position {
                        x: a.x.min(p.position.x),
                        y: a.y.min(p.position.y),
                    },
                );
                self.session_mut().paste_nodes(&fragment, anchor)
            });
            match result {
                Ok(ids) => {
                    movement.copies = ids;
                    self.selection.set(&movement.copies);
                }
                Err(error) => {
                    self.message = error;
                    if let Some(Drag::MediaNudge(node)) = self.drag {
                        let _ = self.session_mut().nudge_clip(node, 0.0);
                    }
                    self.drag = None;
                    return;
                }
            }
            movement.origins = self
                .session()
                .project()
                .positions
                .iter()
                .filter(|p| self.selection.contains(p.node))
                .map(|p| (p.node, p.position))
                .collect();
            movement.copy_pending = false;
        }
        self.session_mut().move_nodes(&movement.origins, delta);
        self.node_move = Some(movement);
    }
    pub(super) fn finish_gesture(&mut self) {
        self.marquee = None;
        self.node_move = None;
        if let Some(Drag::MediaNudge(node)) = self.drag {
            let _ = self.session_mut().nudge_clip(node, 0.0);
        }
        self.drag = None;
    }
    pub(super) fn cancel_gesture(&mut self) {
        if let Some(marquee) = self.marquee.take() {
            self.selection = marquee.before;
        }
        if let Some(movement) = self.node_move.take() {
            if movement.copies.is_empty() {
                self.session_mut()
                    .move_nodes(&movement.origins, Position { x: 0.0, y: 0.0 });
            } else {
                let result = self.session_mut().remove_nodes(&movement.copies);
                self.apply(result);
                self.selection = movement.before;
            }
        }
        if let Some(Drag::MediaNudge(node)) = self.drag {
            let _ = self.session_mut().nudge_clip(node, 0.0);
        }
        self.drag = None;
    }
    pub(super) fn select_all_nodes(&mut self) {
        self.selection.ids = self
            .session()
            .project()
            .graph
            .nodes()
            .iter()
            .map(|node| node.id)
            .collect();
        self.selection.primary = self.selection.ids.iter().min_by_key(|id| id.0).copied();
        self.selected_control = None;
    }
}
