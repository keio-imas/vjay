use super::*;
pub(super) struct Palette {
    pub screen: Position,
    pub graph: Position,
    pub search: Entity<text_field::TextField>,
    pub selected: usize,
}
impl Editor {
    pub(super) fn open_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let cursor = position(window.mouse_position());
        let graph = self
            .world_pointer(cursor)
            .unwrap_or_else(|| self.next_node_position());
        let search = cx.new(|cx| text_field::TextField::new(String::new(), false, cx));
        cx.observe(&search, |this, _, cx| {
            if let Some(p) = this.palette.as_mut() {
                p.selected = 0;
            }
            cx.notify();
        })
        .detach();
        window.focus(&search.read(cx).focus.clone(), cx);
        self.insert_menu = None;
        self.patch_menu = None;
        self.parameter_menu = None;
        self.category_menu = false;
        self.control_menu = false;
        self.palette = Some(Palette {
            screen: Position {
                x: cursor.x + 12.0,
                y: cursor.y,
            },
            graph,
            search,
            selected: 0,
        });
        cx.notify();
    }
    fn palette_matches(
        &self,
        cx: &Context<Self>,
    ) -> Vec<&'static vjay_domain::node::NodeDescriptor> {
        let Some(p) = &self.palette else {
            return Vec::new();
        };
        let query = p.search.read(cx).text.to_lowercase();
        let mut entries = self
            .session()
            .catalog()
            .iter()
            .copied()
            .filter(|d| {
                query.is_empty()
                    || format!("{} {}", d.name, d.category)
                        .to_lowercase()
                        .contains(&query)
            })
            .collect::<Vec<_>>();
        entries.sort_by_key(|d| (d.category, d.name));
        entries
    }
    pub(super) fn palette_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.palette.is_none() {
            return false;
        }
        let entries = self.palette_matches(cx);
        match event.keystroke.key.as_str() {
            "enter" => {
                let p = self.palette.as_ref().unwrap();
                if let Some(d) = entries.get(p.selected) {
                    self.palette_add(d.type_id, window, cx);
                }
            }
            "up" | "down" => {
                let p = self.palette.as_mut().unwrap();
                p.selected = if event.keystroke.key == "up" {
                    p.selected.saturating_sub(1)
                } else {
                    (p.selected + 1).min(entries.len().saturating_sub(1))
                };
                cx.notify();
            }
            "tab" => {
                self.palette = None;
                window.focus(&self.focus, cx);
                cx.notify();
            }
            _ => return false,
        }
        cx.stop_propagation();
        true
    }
    fn palette_add(&mut self, kind: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(palette) = self.palette.as_ref() else {
            return;
        };
        let position = palette.graph;
        let result = self.session_mut().add_node(kind, position);
        if let Ok(id) = result.as_ref() {
            self.selection.only(*id);
            self.selected_control = None;
        }
        self.apply(result);
        self.palette = None;
        window.focus(&self.focus, cx);
        cx.notify();
    }
    pub(super) fn palette_popup(
        &self,
        cx: &Context<Self>,
        width: f32,
        height: f32,
    ) -> Option<gpui::AnyElement> {
        let p = self.palette.as_ref()?;
        let entries = self.palette_matches(cx);
        let mut list = div()
            .id("palette-results")
            .debug_selector(|| "palette-results".into())
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll();
        for (i, d) in entries.iter().enumerate() {
            let kind = d.type_id;
            list = list.child(
                button(SharedString::from(format!("palette-{kind}")), d.name)
                    .debug_selector(move || format!("palette-{kind}"))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .py_1()
                    .when(i == p.selected, |e| {
                        e.bg(rgb(ELEVATED)).text_color(rgb(ACCENT))
                    })
                    .child(div().flex_1())
                    .child(div().text_xs().text_color(rgb(MUTED)).child(d.category))
                    .on_click(
                        cx.listener(move |this, _, window, cx| this.palette_add(kind, window, cx)),
                    ),
            );
        }
        Some(
            div()
                .id("node-palette")
                .debug_selector(|| "node-palette".into())
                .absolute()
                .left(px(p.screen.x.min(width - 300.0).max(0.0)))
                .top(px(p.screen.y.min(height - 440.0).max(0.0)))
                .w(px(290.0))
                .h(px(420.0))
                .p_2()
                .gap_2()
                .flex()
                .flex_col()
                .bg(rgb(SURFACE))
                .border_1()
                .border_color(rgb(BORDER))
                .shadow_lg()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_down_out(cx.listener(|this, _, window, cx| {
                    this.palette = None;
                    window.focus(&this.focus, cx);
                    cx.notify();
                }))
                .on_scroll(|_, _, cx| cx.stop_propagation())
                .child(div().text_xs().text_color(rgb(MUTED)).child("Add node"))
                .child(p.search.clone())
                .child(list)
                .into_any_element(),
        )
    }
}
