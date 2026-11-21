use super::*;
use vjay_domain::graph::Connection;

pub(super) struct InsertMenu {
    pub edge: Connection,
    pub screen: Position,
    pub graph: Position,
}
impl Editor {
    pub(super) fn open_insert(&mut self, pointer: Position) {
        let Some(point) = self.world_pointer(pointer) else {
            return;
        };
        let positions: std::collections::HashMap<_, _> = self
            .session()
            .project()
            .positions
            .iter()
            .map(|p| (p.node, p.position))
            .collect();
        let edge = self
            .session()
            .project()
            .graph
            .connections()
            .filter_map(|edge| {
                let a = positions.get(&edge.from)?;
                let b = positions.get(&edge.to)?;
                let from = Position {
                    x: a.x + patch::NODE_WIDTH,
                    y: a.y + patch::port_y(edge.output),
                };
                let to = Position {
                    x: b.x,
                    y: b.y + patch::port_y(edge.input),
                };
                let distance = wire_distance(point, from, to);
                (distance < 9.0 / self.zoom).then_some((distance, edge))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, edge)| edge);
        self.patch_menu = None;
        if edge.is_none() {
            self.open_patch_menu(pointer);
        }
        self.insert_menu = edge.map(|edge| InsertMenu {
            edge,
            screen: pointer,
            graph: point,
        });
    }
    pub(super) fn insert_popup(
        &self,
        cx: &Context<Self>,
        width: f32,
        height: f32,
    ) -> Option<gpui::AnyElement> {
        let menu = self.insert_menu.as_ref()?;
        let edge = menu.edge;
        let position = menu.graph;
        let entries = self
            .session()
            .catalog()
            .iter()
            .copied()
            .filter(|descriptor| self.session().can_insert(edge, descriptor.type_id))
            .map(|descriptor| {
                let type_id = descriptor.type_id;
                button(
                    SharedString::from(format!("insert-{type_id}")),
                    descriptor.name,
                )
                .debug_selector(move || format!("insert-{type_id}"))
                .text_sm()
                .flex_shrink_0()
                .on_click(cx.listener(move |this, _, _, cx| {
                    let result = this.session_mut().insert_node(edge, type_id, position);
                    if let Ok(id) = &result {
                        this.selection.only(*id);
                        this.selected_control = None;
                    }
                    this.apply(result);
                    this.insert_menu = None;
                    cx.notify();
                }))
            });
        Some(
            div()
                .id("insert-menu")
                .debug_selector(|| "insert-menu".into())
                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                    this.insert_menu = None;
                    cx.notify();
                }))
                .on_scroll(|_, _, cx| cx.stop_propagation())
                .absolute()
                .left(px(menu.screen.x.min(width - 260.0).max(0.0)))
                .top(px(menu.screen.y.min(height - 380.0).max(0.0)))
                .w(px(250.0))
                .max_h(px(360.0))
                .flex()
                .flex_col()
                .gap_1()
                .p_2()
                .bg(rgb(ELEVATED))
                .border_1()
                .border_color(rgb(BORDER))
                .shadow_lg()
                .overflow_y_scroll()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .px_2()
                        .py_1()
                        .text_xs()
                        .text_color(rgb(MUTED))
                        .child("Insert into connection"),
                )
                .children(entries)
                .child(
                    button("disconnect-wire", "Disconnect")
                        .debug_selector(|| "disconnect-wire".into())
                        .text_color(rgb(0xe79791))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.session_mut().disconnect(edge.to, edge.input);
                            this.insert_menu = None;
                            cx.notify();
                        })),
                )
                .into_any_element(),
        )
    }
}
pub(super) fn wire_distance(point: Position, a: Position, b: Position) -> f32 {
    let bend = cables::wire_bend(a.x, b.x);
    let mut previous = a;
    let mut best = f32::MAX;
    for step in 1..=32 {
        let t = step as f32 / 32.0;
        let u = 1.0 - t;
        let next = Position {
            x: u * u * u * a.x
                + 3.0 * u * u * t * (a.x + bend)
                + 3.0 * u * t * t * (b.x - bend)
                + t * t * t * b.x,
            y: u * u * u * a.y + 3.0 * u * u * t * a.y + 3.0 * u * t * t * b.y + t * t * t * b.y,
        };
        let dx = next.x - previous.x;
        let dy = next.y - previous.y;
        let fraction = (((point.x - previous.x) * dx + (point.y - previous.y) * dy)
            / (dx * dx + dy * dy).max(1e-6))
        .clamp(0.0, 1.0);
        best = best.min(
            ((point.x - previous.x - fraction * dx).powi(2)
                + (point.y - previous.y - fraction * dy).powi(2))
            .sqrt(),
        );
        previous = next;
    }
    best
}
