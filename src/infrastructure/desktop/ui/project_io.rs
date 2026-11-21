use super::*;
impl Editor {
    pub(super) fn start_save(
        &mut self,
        path: Option<std::path::PathBuf>,
        after: Option<projects::Change>,
        cx: &mut Context<Self>,
    ) {
        if self.project_io_busy {
            self.message = "A project file operation is in progress".into();
            return;
        }
        let request = match self.session().prepare_save(path) {
            Ok(request) => request,
            Err(error) => {
                self.message = error;
                cx.notify();
                return;
            }
        };
        self.project_io_busy = true;
        self.message = "Saving project…".into();
        let task = cx
            .background_executor()
            .spawn(async move { request.execute() });
        cx.spawn(async move |entity, cx| {
            let result = task.await;
            let _ = entity.update(cx, |this, cx| {
                this.project_io_busy = false;
                match result.and_then(|saved| this.session_mut().finish_save(saved)) {
                    Ok(true) => {
                        this.message = "Project saved".into();
                        if let Some(action) = after {
                            this.perform_project_change(action, cx);
                        }
                    }
                    Ok(false) => {
                        this.message = "Project saved; newer edits are still unsaved".into()
                    }
                    Err(error) => this.message = error,
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub(super) fn start_open(&mut self, path: std::path::PathBuf, cx: &mut Context<Self>) {
        if self.project_io_busy {
            self.message = "A project file operation is in progress".into();
            return;
        }
        let request = match self.session().prepare_open(path) {
            Ok(request) => request,
            Err(error) => {
                self.message = error;
                cx.notify();
                return;
            }
        };
        self.project_io_busy = true;
        self.message = "Opening project…".into();
        let task = cx
            .background_executor()
            .spawn(async move { request.execute() });
        cx.spawn(async move |entity, cx| {
            let result = task.await;
            let _ = entity.update(cx, |this, cx| {
                this.project_io_busy = false;
                match result.and_then(|loaded| this.session_mut().finish_open(loaded)) {
                    Ok(()) => {
                        this.project_changed(cx);
                        this.message = "Project opened".into();
                    }
                    Err(error) => this.message = error,
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}
