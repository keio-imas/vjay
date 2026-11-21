use super::*;
use configuration::Target;
mod settings;
use vjay_domain::{capabilities::SettingEditor, port::SignalType};

impl Editor {
    pub(super) fn inspector(&self, cx: &Context<Self>) -> gpui::AnyElement {
        div()
            .id("inspector")
            .w(px(self.session().project().workspace.inspector_width))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .bg(rgb(SURFACE))
            .border_l_1()
            .border_color(rgb(BORDER))
            .child(
                div()
                    .h(px(30.0))
                    .px_3()
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .font_weight(FontWeight::MEDIUM)
                    .child("Inspector"),
            )
            .child(self.inspector_contents(cx))
            .into_any_element()
    }
    fn inspector_contents(&self, cx: &Context<Self>) -> gpui::AnyElement {
        let mut panel = div()
            .id("inspector-contents")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .gap_1()
            .px_3()
            .py_2()
            .overflow_y_scroll();
        if let Some(id) = self.selected_control {
            return panel
                .child(self.control_inspector(id, cx))
                .into_any_element();
        }
        if self.selection.len() > 1 {
            return panel.child(self.group_inspector(cx)).into_any_element();
        }
        let Some(id) = self.selection.primary() else {
            return panel
                .child(div().text_color(rgb(MUTED)).child("No selection"))
                .into_any_element();
        };
        let Ok(node) = self.session().project().graph.node(id) else {
            return panel.into_any_element();
        };
        let descriptor = self.session().schema(node.id);
        panel = panel.child(
            div()
                .pb_3()
                .pt_1()
                .border_b_1()
                .border_color(rgb(BORDER))
                .child(
                    button(
                        "node-label",
                        if node.settings.label.is_empty() {
                            descriptor.name.to_string()
                        } else {
                            node.settings.label.clone()
                        },
                    )
                    .px_0()
                    .w_full()
                    .font_weight(FontWeight::SEMIBOLD)
                    .truncate()
                    .on_click(cx.listener(move |this, _, w, cx| {
                        this.open_configuration(Target::NodeLabel(id), w, cx)
                    })),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(MUTED))
                        .child(format!("{}  /  #{}", descriptor.category, id.0)),
                ),
        );
        panel = panel.child(self.inspector_settings(id, cx));
        if !descriptor.parameters.is_empty() {
            panel = panel.child(section("Parameters"));
        }
        for index in 0..descriptor.parameters.len() {
            if descriptor
                .capabilities
                .media
                .is_some_and(|m| m.speed == index)
            {
                continue;
            }
            if descriptor.capabilities.rate.is_some_and(|r| {
                [r.mode, r.rate, r.range, r.clip_bpm, r.dj_bpm, r.trim].contains(&index)
            }) {
                continue;
            }

            panel = panel.child(self.parameter(id, index, false, cx));
        }
        if !descriptor.outputs.is_empty() {
            panel = panel.child(section("Outputs"));
        }
        for (port, output) in descriptor.outputs.iter().enumerate() {
            let text = if output.signal == SignalType::Text {
                format!(
                    "{}: {}",
                    output.name,
                    self.session().text_value(id, port).unwrap_or("")
                )
            } else if output.signal == SignalType::Scalar {
                format!(
                    "{}  {:.3}",
                    output.name,
                    self.session().signal_value(id, port).unwrap_or(0.0)
                )
            } else {
                format!("{} · {:?}", output.name, output.signal)
            };
            panel = panel.child(
                div()
                    .flex()
                    .items_center()
                    .child(div().flex_1().text_xs().child(text))
                    .when(
                        matches!(output.signal, SignalType::Scalar | SignalType::Text),
                        |element| {
                            element.child(
                                button(
                                    SharedString::from(format!("meter-{port}")),
                                    if output.signal == SignalType::Text {
                                        "Label"
                                    } else {
                                        "Meter"
                                    },
                                )
                                .text_xs()
                                .px_2()
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        // Reuse the binding allocator, then explicitly select a scalar output.
                                        let result = this.session_mut().pin_output(id, port);
                                        this.apply(result);
                                        cx.notify();
                                    },
                                )),
                            )
                        },
                    ),
            );
        }
        let connected = self
            .session()
            .project()
            .graph
            .connections()
            .any(|c| c.to == id);
        if connected {
            panel = panel.child(section("Connections"));
        }
        for input in 0..descriptor.input_count() {
            let port = descriptor.input(input).expect("declared port");
            let source = self
                .session()
                .project()
                .graph
                .connections()
                .find(|e| e.to == id && e.input == input);
            if let Some(edge) = source {
                panel = panel.child(
                    div()
                        .text_xs()
                        .flex()
                        .items_center()
                        .child(
                            div()
                                .flex_1()
                                .child(format!("{} ← #{}:{}", port.name, edge.from.0, edge.output)),
                        )
                        .child(
                            button(SharedString::from(format!("disconnect-{input}")), "×")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.session_mut().disconnect(id, input);
                                    cx.notify();
                                })),
                        ),
                );
            }
        }
        panel = panel.child(
            button("delete", "Remove node · ⌫")
                .mt_3()
                .text_color(rgb(0xe79791))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.delete_selection(cx);
                })),
        );
        panel.into_any_element()
    }
}

pub(super) fn section(label: &'static str) -> Div {
    div()
        .mt_3()
        .pt_2()
        .pb_1()
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(MUTED))
        .border_t_1()
        .border_color(rgb(BORDER))
        .child(label)
}
