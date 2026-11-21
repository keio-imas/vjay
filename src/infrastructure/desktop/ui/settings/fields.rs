use super::*;
impl Editor {
    pub(super) fn settings_field(
        &self,
        key: &'static str,
        label: &'static str,
        menu: bool,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        let s = self.settings.as_ref().expect("settings");
        let value = s.value(key, cx);
        let field = if menu {
            let label = s
                .choices(key)
                .into_iter()
                .find(|(id, _)| id == &value)
                .map_or(value, |(_, name)| name);
            button(
                SharedString::from(format!("setting-{key}")),
                format!("{label}  ▾"),
            )
            .debug_selector(move || format!("setting-{key}"))
            .w_full()
            .truncate()
            .py_2()
            .bg(rgb(BACKGROUND))
            .border_1()
            .border_color(rgb(BORDER))
            .on_click(cx.listener(move |this, _, w, cx| {
                if let Some(s) = &mut this.settings {
                    s.menu = Some(Menu {
                        key,
                        choices: s.choices(key),
                        point: position(w.mouse_position()),
                    });
                }
                cx.notify();
            }))
            .into_any_element()
        } else {
            s.fields[key].clone().into_any_element()
        };
        div()
            .debug_selector(move || format!("settings-row-{key}"))
            .flex()
            .items_center()
            .gap_4()
            .py_2()
            .child(
                div()
                    .w(px(150.0))
                    .flex_shrink_0()
                    .text_sm()
                    .text_color(rgb(MUTED))
                    .child(label),
            )
            .child(
                div()
                    .debug_selector(move || format!("settings-input-{key}"))
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(field),
            )
            .into_any_element()
    }
    pub(super) fn settings_menu(
        &self,
        cx: &Context<Self>,
        width: f32,
        height: f32,
    ) -> Option<gpui::AnyElement> {
        let m = self.settings.as_ref()?.menu.as_ref()?;
        let key = m.key;
        let choices = m.choices.clone();
        Some(
            div()
                .id("settings-menu")
                .absolute()
                .left(px(m.point.x.min(width - 320.0).max(8.0)))
                .top(px((m.point.y + 8.0).min(height - 280.0).max(8.0)))
                .w(px(300.0))
                .max_h(px(260.0))
                .bg(rgb(SURFACE))
                .border_1()
                .border_color(rgb(BORDER))
                .shadow_lg()
                .p_1()
                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                    if let Some(s) = &mut this.settings {
                        s.menu = None;
                    }
                    cx.notify();
                }))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_scroll(|_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .id("settings-choices")
                        .max_h(px(248.0))
                        .overflow_y_scroll()
                        .children(choices.into_iter().enumerate().map(|(i, (value, label))| {
                            button(SharedString::from(format!("settings-choice-{i}")), label)
                                .w_full()
                                .py_2()
                                .text_sm()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if let Some(s) = &mut this.settings {
                                        s.fields[key].update(cx, |f, cx| {
                                            f.text = value.clone();
                                            cx.notify();
                                        });
                                        s.menu = None;
                                    }
                                    cx.notify();
                                }))
                        })),
                )
                .into_any_element(),
        )
    }
}
impl Settings {
    fn choices(&self, key: &str) -> Vec<(String, String)> {
        let named = |items: &[(&str, &str)]| {
            items
                .iter()
                .map(|(v, n)| (v.to_string(), n.to_string()))
                .collect()
        };
        match key {
            "device" => self.devices.clone(),
            "display" => self.displays.clone(),
            "sample_rate" => named(&[
                ("0", "Device default"),
                ("44100", "44,100 Hz"),
                ("48000", "48,000 Hz"),
                ("96000", "96,000 Hz"),
                ("192000", "192,000 Hz"),
            ]),
            "buffer" => named(&[
                ("0", "Device default"),
                ("64", "64 frames"),
                ("128", "128 frames"),
                ("256", "256 frames"),
                ("512", "512 frames"),
                ("1024", "1,024 frames"),
            ]),
            "channels" => (0..8)
                .map(|i| {
                    (
                        i.to_string(),
                        format!("Channels {}–{}", i * 2 + 1, i * 2 + 2),
                    )
                })
                .collect(),
            "frame_rate" => [24, 25, 30, 50, 60, 120]
                .into_iter()
                .map(|v| (v.to_string(), format!("{v} Hz")))
                .collect(),
            "fullscreen" => named(&[("false", "Windowed"), ("true", "Fullscreen")]),
            "vsync" => named(&[("true", "On"), ("false", "Off")]),
            _ => vec![],
        }
    }
}
