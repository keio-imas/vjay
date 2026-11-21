use super::*;
#[derive(Clone)]
enum Target {
    Property(NodeId, String),
    Monitor,
}
pub(super) struct FontPicker {
    target: Target,
    title: String,
    search: Entity<text_field::TextField>,
    names: Vec<String>,
    values: Vec<String>,
    matches: Vec<usize>,
    screen: Position,
}
impl Editor {
    pub(super) fn open_font_picker(
        &mut self,
        node: NodeId,
        key: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut names = cx.text_system().all_font_names();
        names.extend(["sans-serif".into(), "serif".into(), "monospace".into()]);
        names.sort();
        names.dedup();
        self.open_choice_picker(
            Target::Property(node, key),
            "Font family",
            names,
            window,
            cx,
        );
    }
    pub(super) fn open_source_picker(
        &mut self,
        node: NodeId,
        key: String,
        provider: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let info = self.session().video_source_info(provider);
        let names = info.iter().map(|s| s.name.clone()).collect();
        let values = info.into_iter().map(|s| s.id).collect();
        self.open_choice_picker(
            Target::Property(node, key),
            "Video source (reopen to refresh)",
            names,
            window,
            cx,
        );
        if let Some(p) = &mut self.font_picker {
            p.values = values;
        }
    }
    pub(super) fn open_monitor_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut names = vec!["System default".into()];
        names.extend(self.session().monitor_devices());
        self.open_choice_picker(Target::Monitor, "Monitor output device", names, window, cx);
    }
    fn open_choice_picker(
        &mut self,
        target: Target,
        title: &str,
        names: Vec<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let matches = (0..names.len()).collect();
        let search = cx.new(|cx| text_field::TextField::new(String::new(), false, cx));
        cx.observe(&search, |this, _, cx| {
            if let Some(p) = &mut this.font_picker {
                let q = p.search.read(cx).text.to_lowercase();
                p.matches = p
                    .names
                    .iter()
                    .enumerate()
                    .filter(|(_, n)| n.to_lowercase().contains(&q))
                    .map(|(i, _)| i)
                    .collect();
            }
            cx.notify();
        })
        .detach();
        window.focus(&search.read(cx).focus);
        self.font_picker = Some(FontPicker {
            target,
            title: title.into(),
            search,
            values: names.clone(),
            names,
            matches,
            screen: position(window.mouse_position()),
        });
        cx.notify();
    }
    pub(super) fn font_search_focused(&self, window: &Window, cx: &Context<Self>) -> bool {
        self.font_picker
            .as_ref()
            .is_some_and(|p| p.search.read(cx).focus.is_focused(window))
    }
    pub(super) fn font_popup(
        &self,
        cx: &Context<Self>,
        width: f32,
        height: f32,
    ) -> Option<gpui::AnyElement> {
        let p = self.font_picker.as_ref()?;
        Some(
            div()
                .id("font-picker")
                .debug_selector(|| "font-picker".into())
                .absolute()
                .left(px((p.screen.x - 260.0).clamp(0.0, (width - 340.0).max(0.0))))
                .top(px(p.screen.y.clamp(0.0, (height - 380.0).max(0.0))))
                .w(px(320.0))
                .h(px(350.0))
                .p_2()
                .gap_2()
                .flex()
                .flex_col()
                .bg(rgb(SURFACE))
                .border_1()
                .border_color(rgb(BORDER))
                .shadow_lg()
                .on_mouse_down_out(cx.listener(|this, _, window, cx| {
                    this.font_picker = None;
                    window.focus(&this.focus);
                    cx.notify();
                }))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
                .child(p.title.clone())
                .child(p.search.clone())
                .child(
                    gpui::uniform_list(
                        "fonts",
                        p.matches.len(),
                        cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                            let Some(p) = &this.font_picker else {
                                return Vec::new();
                            };
                            range
                                .map(|i| {
                                    let name = p.names[p.matches[i]].clone();
                                    let value = p.values[p.matches[i]].clone();
                                    let target = p.target.clone();
                                    div()
                                        .id(i)
                                        .h(px(28.0))
                                        .px_2()
                                        .flex()
                                        .items_center()
                                        .text_xs()
                                        .truncate()
                                        .cursor_pointer()
                                        .hover(|s| s.bg(rgb(ELEVATED)))
                                        .child(name.clone())
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            match &target {
                                                Target::Property(node, key) => {
                                                    if let Ok(n) =
                                                        this.session().project().graph.node(*node)
                                                    {
                                                        let mut s = n.settings.clone();
                                                        s.properties
                                                            .insert(key.clone(), value.clone());
                                                        s.property_labels
                                                            .insert(key.clone(), name.clone());
                                                        let r = this
                                                            .session_mut()
                                                            .set_settings(*node, s);
                                                        this.apply(r);
                                                    }
                                                }
                                                Target::Monitor => {
                                                    let r =
                                                        this.session_mut().select_monitor_device(
                                                            if name == "System default" {
                                                                ""
                                                            } else {
                                                                &name
                                                            },
                                                        );
                                                    this.apply(r);
                                                }
                                            }
                                            this.font_picker = None;
                                            window.focus(&this.focus);
                                            cx.notify();
                                        }))
                                })
                                .collect::<Vec<_>>()
                        }),
                    )
                    .flex_1()
                    .min_h_0(),
                )
                .into_any_element(),
        )
    }
}
