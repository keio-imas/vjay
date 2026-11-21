use super::*;
impl Editor {
    fn patch_view_changed(&mut self, cx: &mut Context<Self>) {
        self.finish_gesture();
        self.selection.clear();
        self.selected_control = None;
        self.cable = None;
        self.patch_menu = None;
        self.insert_menu = None;
        self.parameter_menu = None;
        self.configuration = None;
        self.font_picker = None;
        self.palette = None;
        self.pan = Position { x: 0.0, y: 0.0 };
        self.zoom = self.session().project().workspace.patch_zoom;
        cx.notify();
    }
    pub(super) fn open_subpatch(&mut self, node: NodeId, cx: &mut Context<Self>) {
        let r = self.session_mut().enter_subpatch(node);
        if r.is_ok() {
            self.patch_view_changed(cx);
        }
        self.apply(r);
        cx.notify();
    }
    pub(super) fn back_subpatch(&mut self, cx: &mut Context<Self>) {
        let r = self.session_mut().leave_subpatch();
        if r.is_ok() {
            self.patch_view_changed(cx);
        }
        self.apply(r);
        cx.notify();
    }
    pub(super) fn encapsulate_selection(&mut self, cx: &mut Context<Self>) {
        let nodes = self.selection.ids.iter().copied().collect::<Vec<_>>();
        let r = self.session_mut().encapsulate(&nodes);
        match r {
            Ok(id) => {
                self.selection.only(id);
                self.message.clear();
            }
            Err(e) => self.message = e,
        }
        self.patch_menu = None;
        cx.notify();
    }
    pub(super) fn patch_breadcrumbs(&self, cx: &Context<Self>) -> gpui::AnyElement {
        div()
            .flex()
            .items_center()
            .gap_1()
            .children(
                self.session()
                    .patch_breadcrumbs()
                    .into_iter()
                    .enumerate()
                    .map(|(depth, name)| {
                        button(SharedString::from(format!("patch-depth-{depth}")), name)
                            .debug_selector(move || format!("patch-depth-{depth}"))
                            .text_xs()
                            .px_1()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let r = this.session_mut().leave_to(depth);
                                if r.is_ok() {
                                    this.patch_view_changed(cx);
                                }
                                this.apply(r);
                                cx.notify();
                            }))
                    }),
            )
            .when(self.session().patch_depth() > 0, |d| {
                d.child(
                    button("save-current-subpatch", "Save patch as…")
                        .text_xs()
                        .debug_selector(|| "save-current-subpatch".into())
                        .on_click(cx.listener(|this, _, _, cx| this.save_subpatch_file(None, cx))),
                )
            })
            .into_any_element()
    }
    pub(super) fn subpatch_inspector(&self, node: NodeId, cx: &Context<Self>) -> gpui::AnyElement {
        div().flex().flex_col().gap_2().child(inspector::section("Subpatch"))
   .child(button("open-subpatch","Open subpatch").debug_selector(||"open-subpatch".into()).on_click(cx.listener(move|this,_,_,cx|this.open_subpatch(node,cx))))
   .child(button("save-subpatch-as","Save subpatch as…").on_click(cx.listener(move|this,_,_,cx|this.save_subpatch_file(Some(node),cx))))
   .when_some(self.session().subpatch_source(node),|d,path|d.child(div().text_xs().text_color(rgb(MUTED)).child(path.file_name().unwrap_or_default().to_string_lossy().into_owned())).child(button("reload-subpatch","Reload from file").on_click(cx.listener(move|this,_,_,cx|{let r=this.session_mut().reload_subpatch(node);this.apply(r);cx.notify();}))))
   .child(div().text_xs().text_color(rgb(MUTED)).child("Use In / Out nodes inside this patch to expose typed ports. Saved patches keep a project snapshot; Reload applies file changes.")).into_any_element()
    }
    pub(super) fn choose_subpatch_file(&mut self, cx: &mut Context<Self>) {
        self.patch_menu = None;
        let anchor = self.next_node_position();
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Load VJAY subpatch".into()),
        });
        cx.spawn(async move |entity, cx| {
            let paths = paths.await;
            let _ = entity.update(cx, |this, cx| {
                match paths {
                    Ok(Ok(Some(paths))) => {
                        if let Some(path) = paths.first() {
                            let r = this.session_mut().load_subpatch_file(path.clone(), anchor);
                            match r {
                                Ok(id) => this.selection.only(id),
                                Err(e) => this.message = e,
                            }
                        }
                    }
                    Ok(Err(e)) => this.message = e.to_string(),
                    Err(e) => this.message = e.to_string(),
                    _ => (),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn save_subpatch_file(&mut self, node: Option<NodeId>, cx: &mut Context<Self>) {
        let path = cx.prompt_for_new_path(
            self.session().project_directory(),
            Some("Untitled.vjay.patch.json"),
        );
        cx.spawn(async move |entity, cx| {
            let path = path.await;
            let _ = entity.update(cx, |this, cx| {
                match path {
                    Ok(Ok(Some(path))) => {
                        let r = this.session_mut().save_subpatch_as(node, path);
                        this.apply(r);
                    }
                    Ok(Err(e)) => this.message = e.to_string(),
                    Err(e) => this.message = e.to_string(),
                    _ => (),
                }
                cx.notify();
            });
        })
        .detach();
    }
}
