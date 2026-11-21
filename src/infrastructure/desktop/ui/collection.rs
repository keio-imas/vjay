use super::*;
impl Editor {
    pub(super) fn choose_property_path(&mut self, id: NodeId, key: String, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: true,
            multiple: false,
            prompt: Some("Choose file, folder, or UTF-8 M3U playlist".into()),
        });
        cx.spawn(async move |entity, cx| {
            let result = paths.await;
            let _ = entity.update(cx, |this, cx| {
                match result {
                    Ok(Ok(Some(paths))) => {
                        if let Some(path) = paths.first()
                            && let Ok(node) = this.session().project().graph.node(id)
                        {
                            let mut settings = node.settings.clone();
                            settings
                                .properties
                                .insert(key, path.to_string_lossy().into_owned());
                            if let Some(parameter) = this
                                .session()
                                .schema(id)
                                .capabilities
                                .asset_selection_parameter
                            {
                                let r = this.session_mut().set_parameter(id, parameter, 0.0);
                                this.apply(r);
                            }
                            let result = this.session_mut().set_settings(id, settings);
                            this.apply(result);
                        }
                    }
                    Ok(Ok(None)) => (),
                    Ok(Err(e)) => this.message = e.to_string(),
                    Err(e) => this.message = e.to_string(),
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn collection_list(
        &self,
        id: NodeId,
        parameter: usize,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        let count = self.session().asset_source_items(id).len();
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(MUTED))
                    .child(format!("Clips · {count} · index starts at 0")),
            )
            .when(count > 0, |e| {
                e.child(
                    gpui::uniform_list(
                        ("clip-collection", id.0 as usize),
                        count,
                        cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                            let selected = this
                                .session()
                                .control_value(&vjay_application::project::ControlBinding {
                                    node: id,
                                    parameter,
                                    ..Default::default()
                                })
                                .round() as usize;
                            range
                                .map(|index| {
                                    let label = this
                                        .session()
                                        .asset_source_items(id)
                                        .get(index)
                                        .map(|v| v.label.clone())
                                        .unwrap_or_default();
                                    div()
                                        .id(index)
                                        .debug_selector(move || format!("collection-clip-{index}"))
                                        .h(px(28.0))
                                        .px_2()
                                        .flex()
                                        .items_center()
                                        .text_xs()
                                        .truncate()
                                        .cursor_pointer()
                                        .when(index == selected, |e| {
                                            e.bg(rgb(ELEVATED)).text_color(rgb(ACCENT))
                                        })
                                        .hover(|s| s.bg(rgb(ELEVATED)))
                                        .child(format!("{index}  {label}"))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            if this.session().parameter_driven(id,parameter){this.message="Clip index is signal controlled; disconnect it to choose a clip manually".into();cx.notify();return;}
                                            let result = this.session_mut().set_parameter(
                                                id,
                                                parameter,
                                                index as f32,
                                            );
                                            this.apply(result);
                                            cx.notify();
                                        }))
                                })
                                .collect::<Vec<_>>()
                        }),
                    )
                    .h(px(168.0))
                    .on_scroll_wheel(|_, _, cx| cx.stop_propagation()),
                )
            })
            .into_any_element()
    }
}
