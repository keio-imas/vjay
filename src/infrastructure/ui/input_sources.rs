use super::*;
use vjay_application::control_io::InputSettings;
impl Editor {
    pub(super) fn input_sources(&self, cx: &Context<Self>) -> gpui::AnyElement {
        let midi = self.session().input_settings.midi;
        let osc = self.session().input_settings.osc_port;
        div()
            .id("input-sources")
            .debug_selector(|| "input-sources".into())
            .flex_shrink_0()
            .min_w_0()
            .border_b_1()
            .border_color(rgb(BORDER))
            .px_3()
            .h(px(34.0))
            .items_center()
            .flex()
            .gap_1()
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(MUTED))
                    .mr_2()
                    .child("Inputs"),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .child(
                        button("midi-input", if midi { "MIDI on" } else { "MIDI off" })
                            .text_xs()
                            .px_2()
                            .when(midi, |e| e.text_color(rgb(ACCENT)))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let settings = InputSettings {
                                    midi: !midi,
                                    ..this.session().input_settings.clone()
                                };
                                let result = this.session_mut().configure_inputs(settings);
                                this.apply(result);
                                cx.notify();
                            })),
                    )
                    .child(
                        button(
                            "osc-input",
                            if osc.is_some() { "OSC on" } else { "OSC off" },
                        )
                        .text_xs()
                        .px_2()
                        .when(osc.is_some(), |e| e.text_color(rgb(ACCENT)))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let settings = InputSettings {
                                osc_port: if osc.is_some() { None } else { Some(9000) },
                                ..this.session().input_settings.clone()
                            };
                            let result = this.session_mut().configure_inputs(settings);
                            this.apply(result);
                            cx.notify();
                        })),
                    )
                    .child(
                        button(
                            "osc-settings",
                            format!(
                                "{}:{} ▾",
                                self.session().input_settings.osc_host,
                                osc.unwrap_or(9000)
                            ),
                        )
                        .text_xs()
                        .px_2()
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.open_configuration(configuration::Target::OscEndpoint, window, cx)
                        })),
                    ),
            )
            .when(midi || osc.is_some(), |e| {
                e.child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .px_2()
                        .text_xs()
                        .text_color(rgb(MUTED))
                        .child(self.session().input_status.clone()),
                )
            })
            .into_any_element()
    }
}
