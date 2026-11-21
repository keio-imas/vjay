use super::*;
use vjay_domain::graph::Connection;

#[derive(Clone, Copy)]
pub(super) enum Socket {
    Output(NodeId, usize),
    Input(NodeId, usize),
}
#[derive(Clone, Copy)]
pub(super) struct CableGesture {
    pub anchor: Socket,
    pub pointer: Position,
    pub previous: Option<Connection>,
}
impl Editor {
    pub(super) fn begin_cable(&mut self, socket: Socket, pointer: Position) {
        self.drag = None;
        let previous = match socket {
            Socket::Input(node, input) => self
                .session()
                .project()
                .graph
                .connections()
                .find(|edge| edge.to == node && edge.input == input),
            Socket::Output(_, _) => None,
        };
        let anchor = previous.map_or(socket, |edge| Socket::Output(edge.from, edge.output));
        self.cable = Some(CableGesture {
            anchor,
            pointer,
            previous,
        });
    }
    pub(super) fn finish_cable(&mut self, target: Socket) {
        let Some(gesture) = self.cable.take() else {
            return;
        };
        let next = match (gesture.anchor, target) {
            (Socket::Output(from, output), Socket::Input(to, input))
            | (Socket::Input(to, input), Socket::Output(from, output)) => Connection {
                output,
                from,
                to,
                input,
            },
            (Socket::Output(_, _), Socket::Output(from, output)) if gesture.previous.is_some() => {
                let old = gesture.previous.unwrap();
                Connection {
                    output,
                    from,
                    to: old.to,
                    input: old.input,
                }
            }
            _ => return,
        };
        let result = if let Some(previous) = gesture.previous {
            self.session_mut().reconnect(previous, next)
        } else {
            self.session_mut().connect(next)
        };
        if result.is_ok() {
            self.message.clear();
        }
        self.apply(result);
    }
    pub(super) fn socket(
        &self,
        id: SharedString,
        socket: Socket,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        div()
            .id(id.clone())
            .debug_selector(|| id.to_string())
            .size(px(24.0 * self.zoom))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .cursor_crosshair()
            .hover(|style| style.bg(rgb(ELEVATED)))
            .child(
                div()
                    .size(px(7.0 * self.zoom))
                    .rounded_sm()
                    .when(
                        self.socket_signal(socket) == Some(vjay_domain::port::SignalType::Scalar),
                        |e| e.rounded_full(),
                    )
                    .bg(rgb(self.socket_color(socket))),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    window.focus(&this.focus);
                    this.begin_cable(socket, position(event.position));
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    if this.cable.is_some() {
                        this.finish_cable(socket);
                        cx.stop_propagation();
                    }
                    cx.notify();
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, _, _, cx| {
                    if let Socket::Input(node, input) = socket {
                        this.session_mut().disconnect(node, input);
                    }
                    this.cable = None;
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
    }
}

pub(super) fn paint_wire(
    window: &mut Window,
    start: Point<Pixels>,
    end: Point<Pixels>,
    pending: bool,
    zoom: f32,
    signal: vjay_domain::port::SignalType,
) {
    let bend = px(wire_bend(f32::from(start.x) / zoom, f32::from(end.x) / zoom) * zoom);
    let mut path = PathBuilder::stroke(px((if pending { 2.5 } else { 1.5 }) * zoom));
    path.move_to(start);
    path.cubic_bezier_to(
        end,
        start + point(bend, px(0.0)),
        end - point(bend, px(0.0)),
    );
    if let Ok(path) = path.build() {
        window.paint_path(path, rgb(signal_color(signal)));
    }
}

pub(super) fn signal_color(signal: vjay_domain::port::SignalType) -> u32 {
    match signal {
        vjay_domain::port::SignalType::Texture => IMAGE,
        vjay_domain::port::SignalType::Scalar => VALUE,
        vjay_domain::port::SignalType::Audio => AUDIO,
        vjay_domain::port::SignalType::Text => TEXT,
        vjay_domain::port::SignalType::File => ACCENT,
    }
}
impl Editor {
    pub(super) fn socket_color(&self, socket: Socket) -> u32 {
        self.socket_signal(socket).map_or(MUTED, signal_color)
    }
    pub(super) fn socket_signal(&self, socket: Socket) -> Option<vjay_domain::port::SignalType> {
        let (node, output, input) = match socket {
            Socket::Output(node, port) => (node, Some(port), None),
            Socket::Input(node, port) => (node, None, Some(port)),
        };
        let Ok(record) = self.session().project().graph.node(node) else {
            return None;
        };
        let descriptor = self.session().schema(record.id);
        let port = output
            .and_then(|p| descriptor.outputs.get(p).cloned())
            .or_else(|| input.and_then(|p| descriptor.input(p)));
        port.map(|p| p.signal)
    }
}

pub(super) fn wire_bend(a: f32, b: f32) -> f32 {
    ((b - a).abs() * 0.5).clamp(28.0, 90.0)
}
