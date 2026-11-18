use super::*;
use configuration::Target;
use vjay_application::project::{ControlTarget, WidgetKind};

impl Editor {
    pub(super) fn control_inspector(&self, id: u64, cx: &Context<Self>) -> gpui::AnyElement {
        let Some(control) = self
            .session()
            .project()
            .controls
            .iter()
            .find(|c| c.id == id)
        else {
            return div().into_any_element();
        };
        let panel = div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .font_weight(FontWeight::MEDIUM)
                    .child(format!("Control {}", id + 1)),
            )
            .child(
                button(
                    "control-label",
                    if control.label.is_empty() {
                        "Set label…".to_string()
                    } else {
                        control.label.clone()
                    },
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.open_configuration(Target::ControlLabel(id), window, cx)
                })),
            )
            .child(
                button("control-range", "Edit value range…").on_click(cx.listener(
                    move |this, _, window, cx| {
                        this.open_configuration(Target::ControlRange(id), window, cx)
                    },
                )),
            )
            .child(div().text_xs().text_color(rgb(MUTED)).child("Widget"))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .children(WidgetKind::ALL.into_iter().map(|widget| {
                        let mut binding = control.clone();
                        binding.widget = widget;
                        button(
                            SharedString::from(format!("style-{:?}", widget)),
                            widget.name(),
                        )
                        .text_xs()
                        .px_2()
                        .when(control.widget == widget, |e| {
                            e.bg(rgb(ELEVATED)).text_color(rgb(ACCENT))
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let result = this.session_mut().configure_control(binding.clone());
                            this.apply(result);
                            cx.notify();
                        }))
                    })),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(MUTED))
                    .child("Binding · parameter or output"),
            );
        let descriptor = self.session().schema(control.node);
        let name = match control.target {
            ControlTarget::Parameter => descriptor
                .parameters
                .get(control.parameter)
                .map(|p| p.name.as_str()),
            ControlTarget::Output => descriptor
                .outputs
                .get(control.parameter)
                .map(|p| p.name.as_str()),
        }
        .unwrap_or("Unavailable");
        panel
            .child(
                div()
                    .id("current-control-binding")
                    .debug_selector(|| "current-control-binding".into())
                    .p_2()
                    .bg(rgb(BACKGROUND))
                    .border_1()
                    .border_color(rgb(BORDER))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(format!("{} #{}", descriptor.name, control.node.0))
                    .child(div().text_color(rgb(ACCENT)).child(name.to_owned()))
                    .when(control.widget == WidgetKind::XYPad, |e| {
                        e.child(
                            descriptor
                                .parameters
                                .get(control.parameter + 1)
                                .map(|p| p.name.clone())
                                .unwrap_or_default(),
                        )
                    }),
            )
            .child(
                button("change-control-binding", "Change target…")
                    .debug_selector(|| "change-control-binding".into())
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_binding_picker(id, window, cx)
                    })),
            )
            .into_any_element()
    }
}
