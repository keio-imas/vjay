use super::*;
use std::collections::HashMap;
use vjay_domain::port::SignalType;
mod node;
mod wires;

pub(super) const NODE_WIDTH: f32 = 200.0;
pub(super) fn port_y(port: usize) -> f32 {
    40.0 + port as f32 * 20.0
}
pub(super) fn node_height(inputs: usize, outputs: usize) -> f32 {
    32.0 + inputs.max(outputs).max(1) as f32 * 20.0
}
impl Editor {
    pub(super) fn patch(&self, cx: &Context<Self>) -> gpui::AnyElement {
        let positions: HashMap<_, _> = self
            .session()
            .project()
            .positions
            .iter()
            .map(|p| (p.node, p.position))
            .collect();
        let stack = std::rc::Rc::new(
            self.session()
                .project()
                .graph
                .nodes()
                .iter()
                .map(|node| {
                    let p = positions
                        .get(&node.id)
                        .copied()
                        .unwrap_or(Position { x: 30.0, y: 30.0 });
                    crate::gpu::presentation::PreviewRegion {
                        x: p.x,
                        y: p.y,
                        width: NODE_WIDTH,
                        height: self.node_height(node),
                    }
                })
                .collect::<Vec<_>>(),
        );
        let nodes = self
            .session()
            .project()
            .graph
            .nodes()
            .iter()
            .enumerate()
            .map(|(index, node)| {
                self.patch_node(
                    node,
                    positions
                        .get(&node.id)
                        .copied()
                        .unwrap_or(Position { x: 30.0, y: 30.0 }),
                    stack.clone(),
                    index,
                    cx,
                )
            })
            .collect::<Vec<_>>();
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .bg(rgb(BACKGROUND))
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .items_center()
                    .h(px(30.0))
                    .px_3()
                    .gap_2()
                    .bg(rgb(SURFACE))
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .child(self.patch_breadcrumbs(cx))
                    .child(div().text_xs().text_color(rgb(MUTED)).child(
                        if self.selection.len() > 0 {
                            format!("{} selected", self.selection.len())
                        } else {
                            format!("{} nodes", nodes.len())
                        },
                    ))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MUTED))
                            .child(format!("{:.0}%", self.zoom * 100.0)),
                    )
                    .child(div().flex_1())
                    .children(
                        [
                            (SignalType::Texture, "Image"),
                            (SignalType::Scalar, "Value"),
                            (SignalType::Audio, "Audio"),
                            (SignalType::Text, "Text"),
                            (SignalType::File, "File"),
                        ]
                        .into_iter()
                        .map(|(kind, label)| {
                            div()
                                .text_xs()
                                .text_color(rgb(cables::signal_color(kind)))
                                .child(label)
                        }),
                    )
                    .child(
                        button("reset-pan", "Reset view")
                            .text_xs()
                            .text_color(rgb(MUTED))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.pan = Position { x: 0.0, y: 0.0 };
                                this.zoom = 1.0;
                                let mut layout = this.session().project().workspace;
                                layout.patch_zoom = 1.0;
                                let result = this.session_mut().set_workspace(layout);
                                this.apply(result);
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .id("patch-canvas")
                    .debug_selector(|| "patch-canvas".into())
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &MouseDownEvent, window, cx| {
                            this.begin_marquee(event, window, cx)
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Middle,
                        cx.listener(|this, event: &MouseDownEvent, window, cx| {
                            this.finish_gesture();
                            this.drag = Some(Drag::Pan(this.pan, position(event.position)));
                            window.focus(&this.focus);
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|this, event: &MouseDownEvent, window, cx| {
                            window.focus(&this.focus);
                            this.open_insert(position(event.position));
                            cx.notify();
                        }),
                    )
                    .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _, cx| {
                        let delta = position(event.delta.pixel_delta(px(20.0)));
                        if event.modifiers.control || event.modifiers.platform {
                            this.zoom_at(position(event.position), (delta.y * 0.002).exp());
                        } else {
                            this.pan.x += delta.x;
                            this.pan.y += delta.y;
                        }
                        cx.stop_propagation();
                        cx.notify();
                    }))
                    .child(self.patch_wires(&positions))
                    .when(nodes.is_empty(), |e| {
                        e.child(
                            div()
                                .absolute()
                                .left(px(18.0))
                                .top(px(16.0))
                                .text_xs()
                                .text_color(rgb(MUTED))
                                .child("Add nodes from the browser."),
                        )
                    })
                    .children(nodes)
                    .child(self.selection_overlay()),
            )
            .into_any_element()
    }
}
