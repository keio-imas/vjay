use super::*;
use std::collections::HashMap;
use vjay_application::preferences::Preferences;
mod fields;
mod form;

pub(super) struct Settings {
    tab: usize,
    fields: HashMap<&'static str, Entity<text_field::TextField>>,
    devices: Vec<(String, String)>,
    displays: Vec<(String, String)>,
    menu: Option<Menu>,
    pub(super) error: String,
}
struct Menu {
    key: &'static str,
    choices: Vec<(String, String)>,
    point: Position,
}
impl Settings {
    pub(super) fn menu_open(&self) -> bool {
        self.menu.is_some()
    }
    pub(super) fn close_menu(&mut self) {
        self.menu = None;
    }
    pub(super) fn value(&self, key: &str, cx: &gpui::App) -> String {
        self.fields[key].read(cx).text.clone()
    }
    fn parse(&self, cx: &Context<Editor>) -> PortResult<Preferences> {
        let mut p = Preferences {
            default_project_name: self.value("name", cx).trim().into(),
            ..Default::default()
        };
        p.audio.device = self.value("device", cx);
        p.audio.sample_rate = self
            .value("sample_rate", cx)
            .parse()
            .map_err(|_| "Invalid sample rate")?;
        p.audio.buffer_frames = self
            .value("buffer", cx)
            .parse()
            .map_err(|_| "Invalid buffer size")?;
        p.audio.channel_pair = self
            .value("channels", cx)
            .parse()
            .map_err(|_| "Invalid channel pair")?;
        p.audio.volume = self
            .value("volume", cx)
            .parse::<f32>()
            .map_err(|_| "Volume must be 0–100")?
            * 0.01;
        p.video.width = self
            .value("width", cx)
            .parse()
            .map_err(|_| "Invalid output width")?;
        p.video.height = self
            .value("height", cx)
            .parse()
            .map_err(|_| "Invalid output height")?;
        p.video.frame_rate = self
            .value("frame_rate", cx)
            .parse()
            .map_err(|_| "Invalid frame rate")?;
        p.video.display = self.value("display", cx);
        p.video.fullscreen = self.value("fullscreen", cx) == "true";
        p.video.vsync = self.value("vsync", cx) == "true";
        p.validate()?;
        Ok(p)
    }
}
impl Editor {
    pub(super) fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.finish_gesture();
        self.palette = None;
        self.configuration = None;
        self.font_picker = None;
        self.parameter_menu = None;
        self.insert_menu = None;
        self.patch_menu = None;
        self.binding_picker = None;
        self.category_menu = false;
        self.control_menu = false;
        let p = self.session().preferences().clone();
        let values = [
            ("name", p.default_project_name),
            ("device", p.audio.device),
            ("sample_rate", p.audio.sample_rate.to_string()),
            ("buffer", p.audio.buffer_frames.to_string()),
            ("channels", p.audio.channel_pair.to_string()),
            ("volume", format!("{:.0}", p.audio.volume * 100.0)),
            ("width", p.video.width.to_string()),
            ("height", p.video.height.to_string()),
            ("frame_rate", p.video.frame_rate.to_string()),
            ("display", p.video.display),
            ("fullscreen", p.video.fullscreen.to_string()),
            ("vsync", p.video.vsync.to_string()),
        ];
        let fields = values
            .into_iter()
            .map(|(k, v)| {
                let field = cx.new(|cx| text_field::TextField::new(v, false, cx));
                cx.observe(&field, |_, _, cx| cx.notify()).detach();
                (k, field)
            })
            .collect();
        let mut devices = vec![(String::new(), "System default".into())];
        devices.extend(
            self.session()
                .monitor_devices()
                .into_iter()
                .map(|n| (n.clone(), n)),
        );
        let mut displays = vec![(String::new(), "Main display".into())];
        displays.extend(cx.displays().into_iter().enumerate().filter_map(|(i, d)| {
            d.uuid().ok().map(|id| {
                let size = d.bounds().size;
                (
                    id.to_string(),
                    format!(
                        "Display {} · {} × {}",
                        i + 1,
                        f32::from(size.width) as u32,
                        f32::from(size.height) as u32
                    ),
                )
            })
        }));
        self.settings = Some(Settings {
            tab: 0,
            fields,
            devices,
            displays,
            menu: None,
            error: String::new(),
        });
        window.focus(&self.focus);
        cx.notify();
    }
    fn apply_settings(&mut self, close: bool, cx: &mut Context<Self>) {
        let result = self
            .settings
            .as_ref()
            .ok_or_else(|| "Settings closed".to_string())
            .and_then(|s| s.parse(cx));
        let result = result.and_then(|p| self.session_mut().apply_preferences(p));
        match result {
            Ok(()) => {
                // Reopen the existing feeds on the selected destination / window mode.
                let sources: Vec<_> = self.output_windows.iter().map(|(id, _)| *id).collect();
                for (_, handle) in std::mem::take(&mut self.output_windows) {
                    let _ = handle.update(cx, |_, w, _| w.remove_window());
                }
                for id in sources {
                    self.open_output_node(id, cx);
                }
                if close {
                    self.settings = None;
                } else if let Some(s) = &mut self.settings {
                    s.error = "Settings saved".into();
                }
            }
            Err(e) => {
                if let Some(s) = &mut self.settings {
                    s.error = e;
                }
            }
        }
        cx.notify();
    }
    pub(super) fn settings_popup(
        &self,
        cx: &Context<Self>,
        width: f32,
        height: f32,
    ) -> Option<gpui::AnyElement> {
        let s = self.settings.as_ref()?;
        Some(
            div()
                .id("settings-overlay")
                .debug_selector(|| "settings-overlay".into())
                .absolute()
                .inset_0()
                .bg(gpui::rgba(0x00000088))
                .flex()
                .items_center()
                .justify_center()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .w(px(860.0_f32.min(width - 48.0)))
                        .h(px(640.0_f32.min(height - 48.0)))
                        .bg(rgb(SURFACE))
                        .border_1()
                        .border_color(rgb(BORDER))
                        .rounded_sm()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .h(px(50.0))
                                .flex_shrink_0()
                                .px_5()
                                .flex()
                                .items_center()
                                .border_b_1()
                                .border_color(rgb(BORDER))
                                .child(
                                    div()
                                        .flex_1()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child("Settings"),
                                )
                                .child(button("close-settings", "×").on_click(cx.listener(
                                    |this, _, w, cx| {
                                        this.settings = None;
                                        w.focus(&this.focus);
                                        cx.notify();
                                    },
                                ))),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_1()
                                .min_h_0()
                                .child(
                                    div()
                                        .w(px(150.0))
                                        .p_3()
                                        .gap_1()
                                        .flex()
                                        .flex_col()
                                        .bg(rgb(BACKGROUND))
                                        .children(
                                            ["Audio", "Video", "General"]
                                                .into_iter()
                                                .enumerate()
                                                .map(|(tab, name)| {
                                                    button(
                                                        SharedString::from(format!(
                                                            "settings-tab-{tab}"
                                                        )),
                                                        name,
                                                    )
                                                    .debug_selector(move || {
                                                        format!("settings-tab-{tab}")
                                                    })
                                                    .py_2()
                                                    .when(s.tab == tab, |d| {
                                                        d.bg(rgb(ELEVATED)).text_color(rgb(ACCENT))
                                                    })
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        if let Some(s) = &mut this.settings {
                                                            s.tab = tab;
                                                            s.menu = None;
                                                        }
                                                        cx.notify();
                                                    }))
                                                }),
                                        ),
                                )
                                .child(
                                    div()
                                        .id("settings-form")
                                        .flex_1()
                                        .min_w_0()
                                        .p_5()
                                        .overflow_y_scroll()
                                        .child(self.settings_form(cx)),
                                ),
                        )
                        .child(
                            div()
                                .h(px(56.0))
                                .px_5()
                                .flex_shrink_0()
                                .border_t_1()
                                .border_color(rgb(BORDER))
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .flex_1()
                                        .text_xs()
                                        .text_color(rgb(if s.error == "Settings saved" {
                                            VALUE
                                        } else {
                                            0xe79791
                                        }))
                                        .child(s.error.clone()),
                                )
                                .child(button("cancel-settings", "Cancel").on_click(cx.listener(
                                    |this, _, w, cx| {
                                        this.settings = None;
                                        w.focus(&this.focus);
                                        cx.notify();
                                    },
                                )))
                                .child(
                                    button("apply-settings", "Apply")
                                        .debug_selector(|| "apply-settings".into())
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.apply_settings(false, cx)
                                        })),
                                )
                                .child(button("done-settings", "Done").bg(rgb(ELEVATED)).on_click(
                                    cx.listener(|this, _, _, cx| this.apply_settings(true, cx)),
                                )),
                        ),
                )
                .children(self.settings_menu(cx, width, height))
                .into_any_element(),
        )
    }
}
