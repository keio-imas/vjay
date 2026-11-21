use super::*;
use vjay_application::project::ControlBinding;
use vjay_domain::graph::NodeRecord;

impl Editor {
    pub(super) fn patch_node(
        &self,
        node: &NodeRecord,
        origin: Position,
        stack: std::rc::Rc<Vec<crate::gpu::presentation::PreviewRegion>>,
        index: usize,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        let z = self.zoom;
        let id = node.id;
        let descriptor = self.session().schema(node.id);
        let selected = self.selection.contains(id);
        let title = if node.settings.label.is_empty() {
            descriptor.name.to_string()
        } else {
            node.settings.label.clone()
        };
        let signal = descriptor
            .outputs
            .first()
            .or(descriptor.inputs.first())
            .map_or(MUTED, |p| cables::signal_color(p.signal));
        div()
            .id(SharedString::from(format!("node-{}", id.0)))
            .absolute()
            .left(px(origin.x * z + self.pan.x))
            .top(px(origin.y * z + self.pan.y))
            .w(px(NODE_WIDTH * z))
            .h(px(self.node_height(node) * z))
            .bg(rgb(SURFACE))
            .rounded_sm()
            .border_1()
            .border_color(rgb(if selected { ACCENT } else { BORDER }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    if event.modifiers.alt {
                        this.begin_node_move(id, event, window, cx);
                    } else {
                        this.select_node(id, event, window, cx);
                    }
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    window.focus(&this.focus, cx);
                    if !this.selection.contains(id) {
                        this.select_node(id, event, window, cx);
                    }
                    this.open_patch_menu(position(event.position));
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .child(
                div()
                    .id(SharedString::from(format!("header-{}", id.0)))
                    .debug_selector(move || format!("header-{}", id.0))
                    .h(px(27.0 * z))
                    .px(px(8.0 * z))
                    .gap(px(8.0 * z))
                    .flex()
                    .items_center()
                    .bg(rgb(ELEVATED))
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .cursor_move()
                    .child(div().w(px(3.0 * z)).h(px(12.0 * z)).bg(rgb(signal)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(12.0 * z))
                            .font_weight(FontWeight::MEDIUM)
                            .truncate()
                            .child(title),
                    )
                    .child(
                        div()
                            .text_size(px(12.0 * z))
                            .text_color(rgb(MUTED))
                            .child(format!("#{}", id.0)),
                    )
                    .when(
                        descriptor
                            .outputs
                            .iter()
                            .any(|p| p.signal == SignalType::Texture),
                        |header| {
                            header.child(
                                button(
                                    SharedString::from(format!("preview-{}", id.0)),
                                    if node.settings.preview { "▣" } else { "□" },
                                )
                                .px(px(4.0 * z))
                                .py(px(4.0 * z))
                                .text_size(px(12.0 * z))
                                .debug_selector(move || format!("preview-{}", id.0))
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .on_click(cx.listener(
                                    move |this, _, _, cx| this.toggle_node_preview(id, cx),
                                )),
                            )
                        },
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                            if event.click_count >= 2 && this.session().is_subpatch(id) {
                                this.open_subpatch(id, cx);
                                cx.stop_propagation();
                            } else {
                                this.begin_node_move(id, event, window, cx)
                            }
                        }),
                    ),
            )
            .when(descriptor.capabilities.annotation, |card| {
                card.child(
                    div()
                        .absolute()
                        .top(px(36.0 * z))
                        .bottom(px(8.0 * z))
                        .overflow_hidden()
                        .left(px(8.0 * z))
                        .right(px(8.0 * z))
                        .text_size(px(14.0 * z))
                        .child(node.settings.comment.clone())
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                                if event.modifiers.alt {
                                    this.begin_node_move(id, event, window, cx);
                                    return;
                                }
                                this.select_node(id, event, window, cx);
                                if event.click_count >= 2 {
                                    this.open_configuration(
                                        configuration::Target::Comment(id),
                                        window,
                                        cx,
                                    );
                                }
                            }),
                        )
                        .on_mouse_down(
                            MouseButton::Right,
                            cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                                window.focus(&this.focus, cx);
                                if !this.selection.contains(id) {
                                    this.select_node(id, event, window, cx);
                                }
                                this.open_patch_menu(position(event.position));
                                cx.stop_propagation();
                                cx.notify();
                            }),
                        ),
                )
            })
            .when(
                node.settings.preview
                    && descriptor
                        .outputs
                        .iter()
                        .any(|p| p.signal == SignalType::Texture),
                |card| card.child(self.node_preview(node, stack.clone(), index, cx)),
            )
            .children((0..descriptor.input_count()).map(|input| {
                let port = descriptor.input(input).expect("declared input");
                div()
                    .absolute()
                    .left(px(-12.0 * z))
                    .top(px((port_y(input) - 12.0) * z))
                    .flex()
                    .items_center()
                    .gap(px(4.0 * z))
                    .child(self.socket(
                        SharedString::from(format!("in-{}-{input}", id.0)),
                        Socket::Input(id, input),
                        cx,
                    ))
                    .child(
                        div()
                            .max_w(px(92.0 * z))
                            .text_size(px(12.0 * z))
                            .truncate()
                            .text_color(rgb(if port.signal == SignalType::Scalar {
                                VALUE
                            } else {
                                MUTED
                            }))
                            .child(port.name.clone()),
                    )
            }))
            .children(descriptor.outputs.iter().enumerate().map(|(output, port)| {
                let selector = if output == 0 {
                    format!("out-{}", id.0)
                } else {
                    format!("out-{}-{output}", id.0)
                };
                div()
                    .absolute()
                    .right(px(-12.0 * z))
                    .top(px((port_y(output) - 12.0) * z))
                    .flex()
                    .items_center()
                    .gap(px(4.0 * z))
                    .child(
                        div()
                            .max_w(px(72.0 * z))
                            .text_size(px(12.0 * z))
                            .truncate()
                            .text_color(rgb(cables::signal_color(port.signal)))
                            .child(port.name.clone()),
                    )
                    .child(self.socket(
                        SharedString::from(selector),
                        Socket::Output(id, output),
                        cx,
                    ))
            }))
            .children(
                (descriptor.outputs.len().max(descriptor.inputs.len())..descriptor.input_count())
                    .map(|input| {
                        let parameter = input - descriptor.inputs.len();
                        let value = self.session().control_value(&ControlBinding {
                            node: id,
                            parameter,
                            ..Default::default()
                        });
                        div()
                            .absolute()
                            .right(px(11.0 * z))
                            .top(px((port_y(input) - 8.0) * z))
                            .text_size(px(12.0 * z))
                            .text_color(rgb(MUTED))
                            .child(format!("{value:.2}"))
                    }),
            )
            .into_any_element()
    }
}
