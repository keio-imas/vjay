use super::*;

impl Editor {
    pub(super) fn midi_inspector(&self, id: NodeId, cx: &Context<Self>) -> gpui::AnyElement {
        let node = self
            .session()
            .project()
            .graph
            .node(id)
            .expect("selected MIDI node");
        let selected = node.settings.midi_source.as_deref();
        let enabled = self.session().input_settings.midi;
        let learning = self.session().midi_activity().learning == Some(id);
        let mut panel = div().flex().flex_col().gap_2().child(
            div()
                .text_xs()
                .text_color(rgb(MUTED))
                .child("MIDI input source"),
        );
        if !enabled {
            panel = panel.child(
                button("midi-start-node", "Start receiving MIDI")
                    .bg(rgb(ELEVATED))
                    .on_click(cx.listener(|this, _, _, cx| {
                        let result = this.session_mut().refresh_midi_inputs();
                        this.apply(result);
                        cx.notify();
                    })),
            );
        }
        panel = panel.child(
            button("midi-source-all", "All inputs")
                .text_xs()
                .when(selected.is_none(), |e| {
                    e.bg(rgb(ELEVATED)).text_color(rgb(ACCENT))
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    let result = this.session_mut().set_midi_source(id, None);
                    this.apply(result);
                    cx.notify();
                })),
        );
        let rows = self
            .session()
            .midi_ports()
            .iter()
            .enumerate()
            .map(|(index, port)| {
                let source = port.id.clone();
                let chosen = selected == Some(source.as_str());
                button(
                    SharedString::from(format!("midi-source-{index}")),
                    port.name.clone(),
                )
                .text_xs()
                .px_2()
                .when(chosen, |e| e.bg(rgb(ELEVATED)).text_color(rgb(ACCENT)))
                .when(port.error.is_some(), |e| e.text_color(rgb(0xe79791)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    let result = this.session_mut().set_midi_source(id, Some(source.clone()));
                    this.apply(result);
                    cx.notify();
                }))
                .children(
                    port.error
                        .as_ref()
                        .map(|error| div().text_xs().text_color(rgb(MUTED)).child(error.clone())),
                )
            });
        panel = panel.child(
            div()
                .id("midi-source-list")
                .flex()
                .flex_col()
                .gap_1()
                .max_h(px(160.0))
                .overflow_y_scroll()
                .children(rows),
        );
        if selected.is_some_and(|id| !self.session().midi_ports().iter().any(|p| p.id == id)) {
            panel = panel.child(div().text_xs().text_color(rgb(0xe79791)).child(
                "Selected input is unavailable. Reconnect it and refresh, or choose another input.",
            ));
        }
        panel =
            panel.child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .child(button("midi-refresh", "Refresh inputs").text_xs().on_click(
                        cx.listener(|this, _, _, cx| {
                            let result = this.session_mut().refresh_midi_inputs();
                            this.apply(result);
                            cx.notify();
                        }),
                    ))
                    .child(
                        button(
                            "midi-learn",
                            if learning {
                                "Cancel learn"
                            } else {
                                "Learn CC / note"
                            },
                        )
                        .debug_selector(|| "midi-learn".into())
                        .text_xs()
                        .when(learning, |e| e.text_color(rgb(ACCENT)))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if learning {
                                this.session_mut().cancel_midi_learn();
                            } else {
                                let result = this.session_mut().learn_midi(id);
                                this.apply(result);
                            }
                            cx.notify();
                        })),
                    ),
            );
        if learning {
            panel = panel.child(
                div()
                    .text_xs()
                    .text_color(rgb(ACCENT))
                    .child("Move a controller or play a note on the selected input."),
            );
        }
        let activity = self.session().midi_activity();
        panel=panel.child(div().text_xs().text_color(rgb(MUTED)).child(if activity.received==0{"No MIDI received yet".into()}else{format!("Received {} · {}",activity.received,activity.message)}))
            .child(div().text_xs().text_color(rgb(MUTED)).child(self.session().input_status.clone()))
             .when(self.session().midi_ports().iter().any(|p|p.virtual_input&&p.error.is_none()),|e|e.child(div().text_xs().text_color(rgb(MUTED)).child("From another app, select “VJAY MIDI In” as its MIDI output.")))
            .child(div().text_xs().text_color(rgb(MUTED)).child("For a hardware controller, choose its input above. Use Learn to assign its channel and CC / note."));
        panel.into_any_element()
    }
}
