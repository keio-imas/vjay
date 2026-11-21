use super::*;
impl Editor {
    pub(super) fn inspector_settings(&self, id: NodeId, cx: &Context<Self>) -> gpui::AnyElement {
        let node = self
            .session()
            .project()
            .graph
            .node(id)
            .expect("selected node");
        let descriptor = self.session().schema(id);
        let mut panel = div().flex().flex_col().gap_2();
        if let Some(error) = self.session().node_error(id) {
            panel = panel.child(
                div()
                    .text_xs()
                    .text_color(rgb(0xe79791))
                    .child(error.to_owned()),
            );
        }
        let capabilities = descriptor.capabilities;
        if self.session().is_subpatch(id) {
            panel = panel.child(self.subpatch_inspector(id, cx));
        }
        if let Some(status) = self.session().asset_source_status(id) {
            panel = panel
                .child(section("Media source"))
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(MUTED))
                        .child(status.to_owned()),
                )
                .child(button("reload-url", "Reload source").on_click(cx.listener(
                    move |this, _, _, cx| {
                        this.session_mut().reload_asset_source(id);
                        cx.notify();
                    },
                )));
        }

        if (capabilities.media.is_some() || capabilities.still_image)
            && capabilities.asset_selection_parameter.is_none()
        {
            panel = panel.child(section("Source"));
            let name = node
                .asset
                .and_then(|id| self.session().project().assets.iter().find(|a| a.id == id))
                .map(|a| {
                    a.path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned()
                })
                .unwrap_or_else(|| "No media selected".into());
            panel = panel
                .child(div().text_xs().text_color(rgb(MUTED)).child(name))
                .child(
                    button(
                        "replace-video",
                        if capabilities.still_image {
                            "Choose image…"
                        } else {
                            "Choose video…"
                        },
                    )
                    .on_click(cx.listener(move |this, _, _, cx| this.choose_video(Some(id), cx))),
                )
                .when(capabilities.media.is_some(), |panel| {
                    panel.child(
                        div()
                            .text_xs()
                            .text_color(rgb(MUTED))
                            .child("Connect Audio to Audio Out to enable sound."),
                    )
                });
        }
        if let Some(parameter) = capabilities.asset_selection_parameter {
            panel = panel.child(self.collection_list(id, parameter, cx)).child(
                button("pin-files", "Pin Files to Control Panel").on_click(cx.listener(
                    move |this, _, _, cx| {
                        let r = this.session_mut().pin_files(id);
                        this.apply(r);
                        cx.notify();
                    },
                )),
            );
        }
        if capabilities.rate.is_some() || capabilities.tempo_sync.is_some() {
            panel = panel.child(self.rate_controller_panel(id, cx));
        }
        if capabilities.media.is_some() {
            panel = panel.child(self.video_sync_panel(id, cx));
        }
        if capabilities.annotation {
            panel = panel.child(button("edit-comment", "Edit text…").on_click(cx.listener(
                move |this, _, window, cx| this.open_configuration(Target::Comment(id), window, cx),
            )));
        }
        if !capabilities.settings.is_empty() || !node.settings.properties.is_empty() {
            panel = panel.child(section("Settings"));
        }
        for editor in capabilities.settings {
            match editor {
                SettingEditor::Fragment => {
                    panel = panel.child(self.uniform_editor(id, cx));
                    panel = panel.child(
                        button("edit-glsl", "Edit fragment shader…")
                            .bg(rgb(ELEVATED))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.open_configuration(Target::Shader(id), window, cx)
                            })),
                    )
                }
                SettingEditor::Keyboard => {
                    panel = panel.child(
                        button("edit-key", format!("Key: {}", node.settings.key)).on_click(
                            cx.listener(move |this, _, window, cx| {
                                this.open_configuration(Target::Key(id), window, cx)
                            }),
                        ),
                    )
                }
                SettingEditor::Midi => {
                    panel = panel.child(self.midi_inspector(id, cx));
                }
                SettingEditor::Osc => {
                    panel = panel.child(
                        button("edit-osc", node.settings.osc_address.clone()).on_click(
                            cx.listener(move |this, _, window, cx| {
                                this.open_configuration(Target::Osc(id), window, cx)
                            }),
                        ),
                    )
                }
            }
        }
        if capabilities.tap {
            panel = panel.child(button("node-tap", "Tap tempo").bg(rgb(ELEVATED)).on_click(
                cx.listener(move |this, _, _, cx| {
                    this.session_mut().tap_tempo(Some(id));
                    cx.notify();
                }),
            ))
        }
        if capabilities.program_output.is_some() {
            if self.session().window_output(id) {
                panel = panel
                    .child(button("open-output-node", "Open output window").on_click(
                        cx.listener(move |this, _, _, cx| this.open_output_node(id, cx)),
                    ));
            }
            panel = panel.child(
                button(
                    "set-program",
                    if Some(id) == self.session().project().output {
                        "Active Program output"
                    } else {
                        "Use as Program output"
                    },
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    let result = this.session_mut().set_program(id);
                    this.apply(result);
                    cx.notify();
                })),
            )
        }
        let declared = descriptor.properties.iter().filter_map(|p| {
            node.settings
                .properties
                .get(&p.key)
                .map(|value| (&p.key, value))
        });
        let extra = node
            .settings
            .properties
            .iter()
            .filter(|(key, _)| !descriptor.properties.iter().any(|p| p.key == **key));
        for (key, value) in declared.chain(extra) {
            if key == "port.key" {
                continue;
            }
            if let Some(provider) = descriptor
                .properties
                .iter()
                .find(|p| p.key == *key)
                .and_then(|p| p.source_picker)
            {
                let key = key.clone();
                panel = panel.child(
                    button(
                        "choose-video-source",
                        format!(
                            "Source: {} ▾",
                            if value.is_empty() {
                                "Choose…"
                            } else {
                                node.settings.property_labels.get(&key).unwrap_or(value)
                            }
                        ),
                    )
                    .text_xs()
                    .truncate()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_source_picker(id, key.clone(), provider, window, cx)
                    })),
                );
                continue;
            }
            if descriptor
                .properties
                .iter()
                .any(|p| p.key == *key && p.font_picker)
            {
                let key = key.clone();
                panel = panel.child(
                    button("choose-font", format!("Font: {value} ▾"))
                        .debug_selector(|| "choose-font".into())
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.open_font_picker(id, key.clone(), window, cx)
                        })),
                );
                continue;
            }
            if let Some(info) = descriptor
                .properties
                .iter()
                .find(|p| p.key == *key && p.path_picker)
            {
                let key = key.clone();
                panel = panel
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MUTED))
                            .child(format!("{}: {}", info.label, value)),
                    )
                    .child(
                        button("choose-collection", "Choose folder / playlist…")
                            .debug_selector(|| "choose-collection".into())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.choose_property_path(id, key.clone(), cx)
                            })),
                    );
                continue;
            }
            if let Some(info) = descriptor
                .properties
                .iter()
                .find(|p| p.key == *key && !p.choices.is_empty())
            {
                panel = panel.child(self.choice_property(id, key, &info.label, value, cx));
                continue;
            }

            let label = descriptor
                .properties
                .iter()
                .find(|p| p.key == *key)
                .map(|p| p.label.as_str())
                .unwrap_or(key);
            let text = format!("{label}: {value}");
            let key = key.clone();
            panel = panel.child(
                button(SharedString::from(format!("node-property-{key}")), text)
                    .truncate()
                    .debug_selector({
                        let key = key.clone();
                        move || format!("node-property-{key}")
                    })
                    .text_xs()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_configuration(Target::Property(id, key.clone()), window, cx)
                    })),
            );
        }
        panel.into_any_element()
    }
}
