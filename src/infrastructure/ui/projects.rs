use super::*;
#[derive(Clone, Copy)]
pub(super) enum Change {
    New,
    Open,
}
impl Editor {
    fn perform_project_change(&mut self, action: Change, cx: &mut Context<Self>) {
        match action {
            Change::Open => self.open_project_file(cx),
            Change::New => {
                let r = self.session_mut().new_project();
                if r.is_ok() {
                    self.project_changed(cx);
                }
                self.apply(r);
                cx.notify();
            }
        }
    }
    pub(super) fn request_project_change(
        &mut self,
        action: Change,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.session().project_modified() {
            self.perform_project_change(action, cx);
            return;
        }
        let answer = window.prompt(
            gpui::PromptLevel::Warning,
            "Save changes before closing this project?",
            None,
            &["Save", "Discard", "Cancel"],
            cx,
        );
        cx.spawn(async move |entity, cx| {
            let answer = answer.await;
            let _ = entity.update(cx, |this, cx| match answer {
                Ok(0) => this.save_then_change(action, cx),
                Ok(1) => this.perform_project_change(action, cx),
                _ => (),
            });
        })
        .detach();
    }
    fn save_then_change(&mut self, action: Change, cx: &mut Context<Self>) {
        if !self.session().is_untitled() {
            let r = self.session().save();
            if r.is_ok() {
                self.perform_project_change(action, cx);
            }
            self.apply(r);
            return;
        }
        let path = cx.prompt_for_new_path(
            self.session().project_directory(),
            Some("Untitled.vjay.json"),
        );
        cx.spawn(async move |entity, cx| {
            let result = path.await;
            let _ = entity.update(cx, |this, cx| {
                match result {
                    Ok(Ok(Some(path))) => {
                        let r = this.session_mut().save_project_as(path);
                        if r.is_ok() {
                            this.perform_project_change(action, cx);
                        }
                        this.apply(r);
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
    pub(super) fn project_changed(&mut self, cx: &mut Context<Self>) {
        for (_, handle) in std::mem::take(&mut self.output_windows) {
            let _ = handle.update(cx, |_, window, _| window.remove_window());
        }
        self.font_picker = None;
        self.configuration = None;
        self.category_menu = false;
        self.control_menu = false;
        self.selection.clear();
        self.binding_picker = None;
        self.zoom = self.session().project().workspace.patch_zoom;
        self.pan = Position { x: 0.0, y: 0.0 };
        self.finish_gesture();
        self.selected_control = None;
        self.cable = None;
        self.insert_menu = None;
        self.patch_menu = None;
        self.palette = None;
        self.parameter_menu = None;
    }
    pub(super) fn open_project_file(&mut self, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Open VJAY project".into()),
        });
        cx.spawn(async move |entity, cx| {
            let result = paths.await;
            let _ = entity.update(cx, |this, cx| {
                match result {
                    Ok(Ok(Some(paths))) => {
                        if let Some(path) = paths.first() {
                            let result = this.session_mut().open_project(path.clone());
                            if result.is_ok() {
                                this.project_changed(cx);
                                this.message = "Project opened".into();
                            }
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
    pub(super) fn save_project_file(&mut self, save_as: bool, cx: &mut Context<Self>) {
        if !save_as && !self.session().is_untitled() {
            let result = self.session().save();
            if result.is_ok() {
                self.message = "Project saved".into();
            }
            self.apply(result);
            cx.notify();
            return;
        }
        let name = if self.session().is_untitled() {
            "Untitled.vjay.json".into()
        } else {
            self.session().project_name()
        };
        let path = cx.prompt_for_new_path(self.session().project_directory(), Some(&name));
        cx.spawn(async move |entity, cx| {
            let result = path.await;
            let _ = entity.update(cx, |this, cx| {
                match result {
                    Ok(Ok(Some(path))) => {
                        let result = this.session_mut().save_project_as(path);
                        if result.is_ok() {
                            this.message = "Project saved".into();
                        }
                        this.apply(result);
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
    pub(super) fn project_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !event.keystroke.modifiers.platform && !event.keystroke.modifiers.control {
            return false;
        }
        match event.keystroke.key.as_str() {
            "o" => self.request_project_change(Change::Open, window, cx),
            "s" => self.save_project_file(event.keystroke.modifiers.shift, cx),
            "n" => self.request_project_change(Change::New, window, cx),
            _ => return false,
        }
        cx.stop_propagation();
        true
    }
}
