use super::*;

impl Editor {
    pub(super) fn mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !event.dragging() {
            self.finish_gesture();
            return;
        }
        let pointer = position(event.position);
        if let Some(cable) = self.cable.as_mut() {
            cable.pointer = pointer;
            cx.notify();
            return;
        }
        if self.marquee.is_some() {
            self.update_marquee(pointer);
            cx.notify();
            return;
        }
        match self.drag {
            Some(Drag::MediaSeek(node, left, width)) => {
                let r = self
                    .session_mut()
                    .seek_clip(node, (pointer.x - left) / width.max(1.0));
                self.apply(r);
            }
            Some(Drag::MediaJog(node, mouse)) => {
                let factor = if event.modifiers.shift { 0.0003 } else { 0.003 };
                let result = self
                    .session_mut()
                    .jog_clip(node, f64::from(pointer.x - mouse.x) * factor);
                self.apply(result);
                self.drag = Some(Drag::MediaJog(node, pointer));
            }
            Some(Drag::Nodes) => self.move_selected_nodes(pointer),
            Some(Drag::Pan(start, mouse)) => {
                self.pan = Position {
                    x: start.x + pointer.x - mouse.x,
                    y: start.y + pointer.y - mouse.y,
                };
            }
            Some(Drag::Control(id, start, mouse)) => {
                if let Some(control) = self
                    .session()
                    .project()
                    .controls
                    .iter()
                    .find(|c| c.id == id)
                {
                    let size = control.size;
                    self.session_mut().update_control_layout(
                        id,
                        Position {
                            x: (start.x + pointer.x - mouse.x).max(0.0),
                            y: (start.y + pointer.y - mouse.y).max(0.0),
                        },
                        size,
                    );
                }
            }
            Some(Drag::Resize(id, start, mouse)) => {
                if let Some(control) = self
                    .session()
                    .project()
                    .controls
                    .iter()
                    .find(|c| c.id == id)
                {
                    let origin = control.position;
                    self.session_mut().update_control_layout(
                        id,
                        origin,
                        Position {
                            x: (start.x + pointer.x - mouse.x).clamp(80.0, 1200.0),
                            y: (start.y + pointer.y - mouse.y).clamp(70.0, 900.0),
                        },
                    );
                }
            }
            Some(Drag::Value(mut drag)) => {
                let value = drag.update(pointer, event.modifiers);
                self.drag = Some(Drag::Value(drag));
                let result = self
                    .session_mut()
                    .set_parameter(drag.node, drag.parameter, value);
                self.apply(result);
            }
            Some(Drag::XY(mut drag)) => {
                let [x, y] = drag.update(pointer, event.modifiers);
                self.drag = Some(Drag::XY(drag));
                let a = self
                    .session_mut()
                    .set_parameter(drag.node, drag.parameter, x);
                self.apply(a);
                let b = self
                    .session_mut()
                    .set_parameter(drag.node, drag.parameter + 1, y);
                self.apply(b);
            }
            Some(Drag::Panel(panel, layout, start)) => {
                self.resize_panel(panel, layout, start, pointer, window)
            }
            None => return,
        }
        cx.notify();
    }
}
