use super::*;
use vjay_application::project::{ControlBinding, ControlTarget, WidgetKind};

impl Editor {
    pub(super) fn widget(&self, c: &ControlBinding, cx: &Context<Self>) -> gpui::AnyElement {
        let value = self.session().control_value(c);
        let range = self.session().control_range(c);
        let ratio = ((value - range[0]) / (range[1] - range[0])).clamp(0.0, 1.0);
        let control_id = c.id;
        let node = c.node;
        let parameter = c.parameter;
        let editable = c.target == ControlTarget::Parameter
            && !self.session().parameter_driven(node, parameter);
        let id = SharedString::from(format!("widget-{}", c.id));
        match c.widget {
            WidgetKind::Files => div()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, window, cx| {
                        this.focus_control(control_id, window, cx);
                        cx.stop_propagation();
                        cx.notify();
                    }),
                )
                .flex()
                .flex_col()
                .gap_1()
                .child(self.collection_list(node, parameter, cx))
                .child(
                    button(
                        SharedString::from(format!("control-files-dir-{control_id}")),
                        "Choose file / folder / playlist…",
                    )
                    .text_xs()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.choose_property_path(node, "collection.path".into(), cx)
                    })),
                )
                .into_any_element(),
            WidgetKind::Slider | WidgetKind::Meter => div()
                .flex()
                .flex_col()
                .gap_2()
                .child(self.session().control_readout(c, 3))
                .child(
                    div()
                        .id(id)
                        .debug_selector(move || format!("widget-{control_id}"))
                        .h(px(16.0))
                        .w_full()
                        .relative()
                        .child(
                            div()
                                .absolute()
                                .left_0()
                                .right_0()
                                .top(px(5.0))
                                .h(px(6.0))
                                .bg(rgb(BACKGROUND))
                                .child(div().h_full().w(relative(ratio)).bg(rgb(if editable {
                                    ACCENT
                                } else {
                                    VALUE
                                }))),
                        )
                        .when(editable && c.widget == WidgetKind::Slider, |element| {
                            element.on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                                    this.focus_control(control_id, window, cx);
                                    this.drag = Some(Drag::Value(parameter_drag::ValueDrag::new(
                                        node,
                                        parameter,
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
                .into_any_element(),
            WidgetKind::Number => button(id, self.session().control_readout(c, 4))
                .debug_selector(move || format!("widget-{control_id}"))
                .text_xl()
                .when(editable, |element| {
                    self.number_drag(
                        element,
                        parameter_drag::ValueDrag::new(
                            node,
                            parameter,
                            value,
                            Position { x: 0.0, y: 0.0 },
                            range,
                            true,
                        ),
                        Some(control_id),
                        cx,
                    )
                })
                .into_any_element(),
            WidgetKind::Label => div()
                .text_lg()
                .child(
                    (c.target == ControlTarget::Output && c.display.output.is_none())
                        .then(|| self.session().text_value(node, parameter))
                        .flatten()
                        .map_or_else(|| self.session().control_readout(c, 3), str::to_owned),
                )
                .into_any_element(),
            WidgetKind::Toggle | WidgetKind::Button => {
                let momentary = c.widget == WidgetKind::Button;
                let active = value > (range[0] + range[1]) * 0.5;
                button(
                    id,
                    if momentary {
                        "Trigger"
                    } else if active {
                        "On"
                    } else {
                        "Off"
                    },
                )
                .debug_selector(move || format!("widget-{control_id}"))
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .bg(rgb(if active { ACCENT } else { BACKGROUND }))
                .text_color(rgb(if active { BACKGROUND } else { TEXT }))
                .when(editable, |element| {
                    element
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, window, cx| {
                                this.focus_control(control_id, window, cx);
                                let result = this.session_mut().set_parameter(
                                    node,
                                    parameter,
                                    if momentary || !active {
                                        range[1]
                                    } else {
                                        range[0]
                                    },
                                );
                                this.apply(result);
                                cx.stop_propagation();
                                cx.notify();
                            }),
                        )
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                if momentary {
                                    let result =
                                        this.session_mut().set_parameter(node, parameter, range[0]);
                                    this.apply(result);
                                }
                                cx.notify();
                            }),
                        )
                        .on_mouse_up_out(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                if momentary {
                                    let result =
                                        this.session_mut().set_parameter(node, parameter, range[0]);
                                    this.apply(result);
                                }
                                cx.notify();
                            }),
                        )
                })
                .into_any_element()
            }
            WidgetKind::Knob => div()
                .id(id)
                .debug_selector(move || format!("widget-{control_id}"))
                .size_full()
                .flex()
                .flex_col()
                .gap_1()
                .items_center()
                .justify_center()
                .cursor_ns_resize()
                .child(
                    div()
                        .size(px(48.0))
                        .rounded_full()
                        .bg(rgb(BACKGROUND))
                        .border_1()
                        .border_color(rgb(BORDER))
                        .child(
                            canvas(
                                move |_, _, _| (),
                                move |bounds, _, window, _| {
                                    let angle = (ratio * 270.0 - 225.0).to_radians();
                                    let center = bounds.center();
                                    let mut line = PathBuilder::stroke(px(3.0));
                                    line.move_to(center);
                                    line.line_to(
                                        center
                                            + point(px(angle.cos() * 18.0), px(angle.sin() * 18.0)),
                                    );
                                    if let Ok(path) = line.build() {
                                        window.paint_path(path, rgb(ACCENT));
                                    }
                                },
                            )
                            .size_full(),
                        ),
                )
                .child(div().text_xs().child(self.session().control_readout(c, 3)))
                .when(editable, |element| {
                    element.on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                            this.focus_control(control_id, window, cx);
                            this.drag = Some(Drag::Value(parameter_drag::ValueDrag::new(
                                node,
                                parameter,
                                value,
                                position(event.position),
                                range,
                                true,
                            )));
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
                })
                .into_any_element(),
            WidgetKind::XYPad => {
                let mut second = c.clone();
                second.parameter += 1;
                second.range = None;
                let y_range = self.session().control_range(&second);
                let y = ((self.session().control_value(&second) - y_range[0])
                    / (y_range[1] - y_range[0]))
                    .clamp(0.0, 1.0);
                let bounds = std::rc::Rc::new(std::cell::Cell::new(Bounds::<Pixels>::default()));
                let hit = bounds.clone();
                div()
                    .id(id)
                    .debug_selector(move || format!("widget-{control_id}"))
                    .size_full()
                    .relative()
                    .bg(rgb(BACKGROUND))
                    .cursor_crosshair()
                    .child(
                        canvas(
                            move |actual, _, _| {
                                bounds.set(actual);
                            },
                            move |actual, _, window, _| {
                                let center = actual.origin
                                    + point(
                                        actual.size.width * ratio,
                                        actual.size.height * (1.0 - y),
                                    );
                                let mut lines = PathBuilder::stroke(px(1.0));
                                lines.move_to(point(actual.left(), center.y));
                                lines.line_to(point(actual.right(), center.y));
                                lines.move_to(point(center.x, actual.top()));
                                lines.line_to(point(center.x, actual.bottom()));
                                if let Ok(path) = lines.build() {
                                    window.paint_path(path, rgb(ACCENT));
                                }
                            },
                        )
                        .size_full(),
                    )
                    .when(editable, |element| {
                        element.on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                                this.focus_control(control_id, window, cx);
                                let b = hit.get();
                                let mut drag = parameter_drag::XYDrag::new(
                                    node,
                                    parameter,
                                    position(event.position),
                                    [value, this.session().control_value(&second)],
                                    [range, y_range],
                                    b,
                                );
                                let [x, y] = drag.begin(event.modifiers);
                                this.drag = Some(Drag::XY(drag));
                                let result = this.session_mut().set_parameter(node, parameter, x);
                                this.apply(result);
                                let result =
                                    this.session_mut().set_parameter(node, parameter + 1, y);
                                this.apply(result);
                                cx.stop_propagation();
                                cx.notify();
                            }),
                        )
                    })
                    .into_any_element()
            }
        }
    }
}
