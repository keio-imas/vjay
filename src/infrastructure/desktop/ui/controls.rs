use super::*;
use vjay_application::project::{ControlBinding, WidgetKind};

mod parameter;

impl Editor {
    pub(super) fn focus_control(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        self.selection.clear();
        self.selected_control = Some(id);
        window.focus(&self.focus);
        cx.notify();
    }

    pub(super) fn deck(&self, cx: &Context<Self>) -> gpui::AnyElement {
        let cards = self
            .session()
            .project()
            .controls
            .iter()
            .map(|control| self.control_card(control, cx))
            .collect::<Vec<_>>();
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .border_l_1()
            .border_color(rgb(BORDER))
            .overflow_hidden()
            .child(
                div()
                    .h(px(30.0))
                    .px_3()
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .child(
                        div()
                            .flex_1()
                            .font_weight(FontWeight::MEDIUM)
                            .child("Control Panel"),
                    )
                    .child(
                        button("add-control", "+ Add")
                            .text_xs()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.control_menu = !this.control_menu;
                                cx.notify();
                            })),
                    ),
            )
            .when(self.control_menu, |element| {
                element.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_1()
                        .p_2()
                        .bg(rgb(SURFACE))
                        .children(WidgetKind::ALL.into_iter().map(|widget| {
                            button(
                                SharedString::from(format!("add-{:?}", widget)),
                                widget.name(),
                            )
                            .text_xs()
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    match this.session_mut().add_control(widget) {
                                        Ok(id) => {
                                            this.selected_control = Some(id);
                                            this.selection.clear();
                                        }
                                        Err(error) => this.message = error,
                                    }
                                    this.control_menu = false;
                                    cx.notify();
                                },
                            ))
                        })),
                )
            })
            .child(
                div()
                    .id("performance-canvas")
                    .relative()
                    .w_full()
                    .overflow_scroll()
                    .flex_1()
                    .min_h_0()
                    .when(cards.is_empty(), |element| {
                        element.child(
                            div()
                                .p_5()
                                .text_xs()
                                .text_color(rgb(MUTED))
                                .child("Add a control or pin a node parameter."),
                        )
                    })
                    .children(cards),
            )
            .into_any_element()
    }
    fn control_card(&self, control: &ControlBinding, cx: &Context<Self>) -> gpui::AnyElement {
        let id = control.id;
        let origin = control.position;
        let size = control.size;
        let node = self
            .session()
            .project()
            .graph
            .node(control.node)
            .expect("validated binding");
        let title = if control.label.is_empty() {
            format!(
                "{} · {}",
                self.session().schema(node.id).name,
                control.widget.name()
            )
        } else {
            control.label.clone()
        };
        div()
            .id(SharedString::from(format!("control-{id}")))
            .absolute()
            .left(px(origin.x))
            .top(px(origin.y))
            .w(px(size.x))
            .h(px(size.y))
            .flex()
            .flex_col()
            .bg(rgb(SURFACE))
            .border_1()
            .border_color(rgb(if self.selected_control == Some(id) {
                ACCENT
            } else {
                BORDER
            }))
            .rounded_sm()
            .overflow_hidden()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, window, cx| {
                    this.focus_control(id, window, cx);
                    cx.stop_propagation();
                }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .h(px(24.0))
                    .flex_shrink_0()
                    .px_2()
                    .gap_1()
                    .child(
                        div()
                            .id(SharedString::from(format!("move-{id}")))
                            .flex_1()
                            .text_xs()
                            .cursor_move()
                            .child(title)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                                    window.focus(&this.focus);
                                    this.selected_control = Some(id);
                                    this.selection.clear();
                                    this.drag =
                                        Some(Drag::Control(id, origin, position(event.position)));
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            ),
                    )
                    .child(
                        button(SharedString::from(format!("unpin-{id}")), "×")
                            .px_1()
                            .text_xs()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.session_mut().remove_control(id);
                                this.selected_control = None;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .p_2()
                    .flex_1()
                    .min_h_0()
                    .child(self.widget(control, cx)),
            )
            .child(
                div()
                    .id(SharedString::from(format!("resize-{id}")))
                    .absolute()
                    .bottom_0()
                    .right_0()
                    .size(px(12.0))
                    .bg(rgb(BORDER))
                    .cursor_nwse_resize()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                            window.focus(&this.focus);
                            this.selection.clear();
                            this.selected_control = Some(id);
                            this.drag = Some(Drag::Resize(id, size, position(event.position)));
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            )
            .into_any_element()
    }
}
