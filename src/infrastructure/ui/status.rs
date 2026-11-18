use super::*;
impl Editor {
    pub(super) fn status_bar(&self, status: String) -> gpui::AnyElement {
        let extent = self.session().extent();
        let project = self.session().project();
        let text = if status.is_empty() {
            self.session()
                .store_location()
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        } else {
            status.clone()
        };
        div()
            .id("status-bar")
            .debug_selector(|| "status-bar".into())
            .h(px(24.0))
            .px_3()
            .flex()
            .items_center()
            .gap_3()
            .bg(rgb(SURFACE))
            .border_t_1()
            .border_color(rgb(BORDER))
            .text_xs()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(rgb(if status.is_empty() { MUTED } else { 0xe3b879 }))
                    .child(text),
            )
            .child(div().text_color(rgb(MUTED)).child(format!(
                "{} nodes · {} connections",
                project.graph.nodes().len(),
                project.graph.connection_count()
            )))
            .child(
                div()
                    .text_color(rgb(MUTED))
                    .child(format!("{} × {}", extent.width, extent.height)),
            )
            .child(
                div()
                    .text_color(rgb(MUTED))
                    .max_w(px(230.0))
                    .truncate()
                    .child(self.session().backend_description().to_string()),
            )
            .into_any_element()
    }
}
