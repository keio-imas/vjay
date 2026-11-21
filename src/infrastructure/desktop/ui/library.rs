use super::*;
impl Editor {
    pub(super) fn library(&self, cx: &Context<Self>) -> gpui::AnyElement {
        let query = self
            .search
            .as_ref()
            .map(|input| input.read(cx).text.to_lowercase())
            .unwrap_or_default();
        let mut categories = self
            .session()
            .catalog()
            .iter()
            .map(|d| d.category)
            .collect::<Vec<_>>();
        categories.sort_unstable();
        categories.dedup();
        let mut matches = self
            .session()
            .catalog()
            .iter()
            .copied()
            .filter(|descriptor| {
                self.category.is_none_or(|c| c == descriptor.category)
                    && (query.is_empty()
                        || format!("{} {}", descriptor.name, descriptor.category)
                            .to_lowercase()
                            .contains(&query))
            })
            .collect::<Vec<_>>();
        matches.sort_by_key(|descriptor| (descriptor.category, descriptor.name));
        let mut list = div()
            .id("node-results")
            .pb_3()
            .debug_selector(|| "node-results".into())
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll();
        let mut category = "";
        for &descriptor in &matches {
            let type_id = descriptor.type_id;
            if descriptor.category != category {
                category = descriptor.category;
                list = list.child(
                    div()
                        .flex_shrink_0()
                        .mt_2()
                        .mb_1()
                        .px_2()
                        .text_xs()
                        .text_color(rgb(MUTED))
                        .child(category),
                );
            }
            let color = descriptor
                .outputs
                .first()
                .or(descriptor.inputs.first())
                .map_or(MUTED, |p| cables::signal_color(p.signal));
            list = list.child(
                div()
                    .id(SharedString::from(format!("browser-{type_id}")))
                    .debug_selector(move || format!("browser-{type_id}"))
                    .h(px(26.0))
                    .flex_shrink_0()
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded_sm()
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(ELEVATED)))
                    .child(div().w(px(3.0)).h(px(10.0)).bg(rgb(color)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_xs()
                            .child(descriptor.name),
                    )
                    .child(div().text_xs().text_color(rgb(MUTED)).child(format!(
                        "{} → {}",
                        descriptor.input_count(),
                        descriptor.outputs.len()
                    )))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.create_node(type_id);
                        cx.notify();
                    })),
            );
        }
        if matches.is_empty() {
            list = list.child(
                div()
                    .px_2()
                    .py_3()
                    .text_xs()
                    .text_color(rgb(MUTED))
                    .child("No matching nodes"),
            );
        }
        div()
            .w(px(self.session().project().workspace.library_width))
            .flex_shrink_0()
            .min_h_0()
            .overflow_hidden()
            .flex()
            .flex_col()
            .bg(rgb(SURFACE))
            .border_r_1()
            .border_color(rgb(BORDER))
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
                            .child("Nodes"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MUTED))
                            .child(matches.len().to_string()),
                    ),
            )
            .child(
                div()
                    .p_2()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().text_xs().text_color(rgb(MUTED)).child("Search nodes"))
                    .children(self.search.clone())
                    .child(
                        button("category-menu", self.category.unwrap_or("All categories"))
                            .text_xs()
                            .px_2()
                            .flex()
                            .items_center()
                            .bg(rgb(BACKGROUND))
                            .border_1()
                            .border_color(rgb(BORDER))
                            .child(div().flex_1())
                            .child(div().text_color(rgb(MUTED)).child("▾"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.category_menu = !this.category_menu;
                                cx.notify();
                            })),
                    )
                    .when(self.category_menu, |element| {
                        element.child(
                            div()
                                .id("categories")
                                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                                    this.category_menu = false;
                                    cx.notify();
                                }))
                                .flex()
                                .flex_col()
                                .max_h(px(220.0))
                                .overflow_y_scroll()
                                .bg(rgb(ELEVATED))
                                .children(
                                    std::iter::once(None)
                                        .chain(categories.into_iter().map(Some))
                                        .map(|category| {
                                            button(
                                                SharedString::from(format!(
                                                    "category-{}",
                                                    category.unwrap_or("all")
                                                )),
                                                category.unwrap_or("All categories"),
                                            )
                                            .text_xs()
                                            .when(self.category == category, |e| {
                                                e.text_color(rgb(ACCENT))
                                            })
                                            .on_click(
                                                cx.listener(move |this, _, _, cx| {
                                                    this.category = category;
                                                    this.category_menu = false;
                                                    cx.notify();
                                                }),
                                            )
                                        }),
                                ),
                        )
                    }),
            )
            .child(list)
            .into_any_element()
    }
    pub(super) fn next_node_position(&self) -> Position {
        let count = self.session().project().graph.nodes().len();
        if let Some(bounds) = self.patch_bounds.get() {
            Position {
                x: (f32::from(bounds.size.width) * 0.35 - self.pan.x) / self.zoom
                    + (count % 4) as f32 * 24.0,
                y: (60.0 - self.pan.y) / self.zoom + (count % 5) as f32 * 28.0,
            }
        } else {
            Position { x: 40.0, y: 60.0 }
        }
    }
    pub(super) fn create_node(&mut self, type_id: &str) {
        let position = self.next_node_position();
        let result = self.session_mut().add_node(type_id, position);
        if let Ok(id) = &result {
            self.selection.only(*id);
            self.selected_control = None;
        }
        self.apply(result);
    }
    pub(super) fn choose_video(&mut self, target: Option<NodeId>, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Choose media file".into()),
        });
        cx.spawn(async move |entity, cx| {
            let result = paths.await;
            let _ = entity.update(cx, |this, cx| {
                match result {
                    Ok(Ok(Some(paths))) => {
                        if let Some(path) = paths.into_iter().next() {
                            this.import_video(path, target);
                        }
                    }
                    Ok(Ok(None)) => (),
                    Ok(Err(error)) => this.message = error.to_string(),
                    Err(error) => this.message = error.to_string(),
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn import_video(&mut self, path: std::path::PathBuf, target: Option<NodeId>) {
        let position = self.next_node_position();
        let still = target.is_some_and(|id| self.session().schema(id).capabilities.still_image)
            || (target.is_none()
                && path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
                    ["png", "jpg", "jpeg", "webp", "gif", "bmp", "tif", "tiff"]
                        .iter()
                        .any(|s| e.eq_ignore_ascii_case(s))
                }));
        let result = if still {
            self.session_mut().import_image(path, target)
        } else {
            self.session_mut().import_video(path, target)
        };
        if let Ok(id) = &result {
            if target.is_none() {
                self.session_mut().move_node(*id, position);
            }
            self.selection.only(*id);
            self.selected_control = None;
            self.message.clear();
        }
        self.apply(result);
    }
}
