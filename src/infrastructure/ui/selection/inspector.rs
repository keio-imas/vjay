use super::*;
impl Editor {
    pub(in crate::ui) fn group_inspector(&self, cx: &Context<Self>) -> gpui::AnyElement {
        let nodes = self
            .session()
            .project()
            .graph
            .nodes()
            .iter()
            .filter(|n| self.selection.contains(n.id));
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .font_weight(FontWeight::MEDIUM)
                    .child(format!("{} nodes selected", self.selection.len())),
            )
            .child(div().flex().flex_col().gap_1().children(nodes.map(|node| {
                let title = if node.settings.label.is_empty() {
                    self.session().schema(node.id).name.to_string()
                } else {
                    node.settings.label.clone()
                };
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_xs()
                    .child(div().flex_1().min_w_0().truncate().child(title))
                    .child(
                        div()
                            .text_color(rgb(MUTED))
                            .child(format!("#{}", node.id.0)),
                    )
            })))
            .child(
                button("delete-selection", "Delete selected")
                    .px_0()
                    .text_xs()
                    .text_color(rgb(0xe79791))
                    .on_click(cx.listener(|this, _, _, cx| this.delete_selection(cx))),
            )
            .into_any_element()
    }
}
