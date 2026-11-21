use super::*;
impl Editor {
    pub(in crate::desktop::ui) fn parameter(
        &self,
        id: NodeId,
        index: usize,
        pinned: bool,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        let binding = ControlBinding {
            node: id,
            parameter: index,
            ..Default::default()
        };
        let node = self
            .session()
            .project()
            .graph
            .node(id)
            .expect("validated node");
        let parameter = self.session().schema(node.id).parameters[index].clone();
        let value = self.session().control_value(&binding);
        let range = self.session().control_range(&binding);
        let ratio = ((value - range[0]) / (range[1] - range[0])).clamp(0.0, 1.0);
        let driven = self.session().parameter_driven(id, index);
        let momentary = self.session().schema(id).capabilities.momentary_parameter == Some(index);
        if momentary || !parameter.choices.is_empty() {
            return div()
                .flex()
                .flex_col()
                .gap_1()
                .py_1()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .w(px(104.0))
                                .flex_shrink_0()
                                .text_xs()
                                .text_color(rgb(MUTED))
                                .child(parameter.name.clone()),
                        )
                        .child(div().flex_1().min_w_0().child(if momentary {
                            self.trigger_parameter(id, index, driven, cx)
                        } else {
                            self.choice_parameter(id, index, value, driven, cx)
                        }))
                        .when(!pinned, |e| {
                            e.child(
                                button(SharedString::from(format!("pin-{}-{index}", id.0)), "+")
                                    .px_1()
                                    .text_xs()
                                    .text_color(rgb(MUTED))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        let r = this.session_mut().pin_control(id, index);
                                        this.apply(r);
                                        cx.notify();
                                    })),
                            )
                        }),
                )
                .into_any_element();
        }
        div()
            .flex()
            .flex_col()
            .gap_1()
            .py_1()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_xs()
                            .text_color(rgb(MUTED))
                            .child(parameter.name.clone()),
                    )
                    .child(
                        button(
                            SharedString::from(format!("number-{}-{index}", id.0)),
                            format!("{value:.3}"),
                        )
                        .debug_selector(move || format!("number-{}-{index}", id.0))
                        .text_xs()
                        .text_right()
                        .min_w(px(64.0))
                        .px_2()
                        .py(px(2.0))
                        .bg(rgb(BACKGROUND))
                        .border_1()
                        .border_color(rgb(BORDER))
                        .when(driven, |e| e.text_color(rgb(VALUE)).cursor_default())
                        .when(!driven, |e| {
                            self.number_drag(
                                e,
                                parameter_drag::ValueDrag::new(
                                    id,
                                    index,
                                    value,
                                    Position { x: 0.0, y: 0.0 },
                                    range,
                                    true,
                                ),
                                None,
                                cx,
                            )
                        }),
                    )
                    .when(!pinned, |e| {
                        e.child(
                            button(SharedString::from(format!("pin-{}-{index}", id.0)), "+")
                                .text_xs()
                                .px_1()
                                .text_color(rgb(MUTED))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    let result = this.session_mut().pin_control(id, index);
                                    this.apply(result);
                                    cx.notify();
                                })),
                        )
                    }),
            )
            .child(
                div()
                    .id(SharedString::from(format!(
                        "parameter-{}-{index}-{pinned}",
                        id.0
                    )))
                    .debug_selector(move || format!("parameter-{}-{index}-{pinned}", id.0))
                    .relative()
                    .h(px(8.0))
                    .w_full()
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .right_0()
                            .top(px(3.0))
                            .h(px(2.0))
                            .bg(rgb(BACKGROUND))
                            .child(div().h_full().w(relative(ratio)).bg(rgb(if driven {
                                VALUE
                            } else {
                                ACCENT
                            }))),
                    )
                    .when(!driven, |e| {
                        e.cursor_ew_resize().on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                                window.focus(&this.focus);
                                this.drag = Some(Drag::Value(parameter_drag::ValueDrag::new(
                                    id,
                                    index,
                                    value,
                                    position(event.position),
                                    range,
                                    false,
                                )));
                                cx.stop_propagation();
                                cx.notify();
                            }),
                        )
                    }),
            )
            .into_any_element()
    }
}
