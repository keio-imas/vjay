use super::*;

#[derive(Clone, Copy)]
pub(super) struct PatchMenu {
    pub screen: Position,
    pub world: Position,
}
impl Editor {
    pub(super) fn copy_patch(&mut self, cx: &mut Context<Self>) -> bool {
        let ids: Vec<_> = self.selection.ids.iter().copied().collect();
        match self
            .session()
            .copy_nodes(&ids)
            .and_then(|f| crate::clipboard::encode(&f))
        {
            Ok(text) => {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
                true
            }
            Err(e) => {
                self.message = e;
                false
            }
        }
    }
    pub(super) fn paste_patch(&mut self, position: Option<Position>, cx: &mut Context<Self>) {
        let result = cx
            .read_from_clipboard()
            .and_then(|i| i.text())
            .ok_or_else(|| "Clipboard has no VJAY nodes".to_owned())
            .and_then(|s| crate::clipboard::decode(&s));
        let anchor = position.unwrap_or_else(|| self.next_node_position());
        match result.and_then(|f| self.session_mut().paste_nodes(&f, anchor)) {
            Ok(ids) => {
                self.selection.set(&ids);
                self.selected_control = None;
                self.message.clear();
            }
            Err(e) => self.message = e,
        }
        self.patch_menu = None;
        cx.notify();
    }
    pub(super) fn patch_popup(
        &self,
        cx: &Context<Self>,
        width: f32,
        height: f32,
    ) -> Option<gpui::AnyElement> {
        let menu = self.patch_menu?;
        Some(
            div()
                .id("patch-menu")
                .debug_selector(|| "patch-menu".into())
                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                    this.patch_menu = None;
                    cx.notify();
                }))
                .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
                .absolute()
                .left(px(menu.screen.x.min(width - 180.0).max(0.0)))
                .top(px(menu.screen.y.min(height - 300.0).max(0.0)))
                .w(px(225.0))
                .p_1()
                .flex()
                .flex_col()
                .bg(rgb(ELEVATED))
                .border_1()
                .border_color(rgb(BORDER))
                .shadow_lg()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    button("copy-nodes", "Copy     ⌘/Ctrl+C")
                        .text_xs()
                        .flex_shrink_0()
                        .debug_selector(|| "copy-nodes".into())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.copy_patch(cx);
                            this.patch_menu = None;
                            cx.notify();
                        })),
                )
                .child(
                    button("paste-nodes", "Paste    ⌘/Ctrl+V")
                        .text_xs()
                        .flex_shrink_0()
                        .debug_selector(|| "paste-nodes".into())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.paste_patch(Some(menu.world), cx)
                        })),
                )
                .child(button("duplicate-nodes", "Duplicate").on_click(cx.listener(
                    |this, _, _, cx| {
                        if this.copy_patch(cx) {
                            this.paste_patch(None, cx);
                        } else {
                            this.patch_menu = None;
                            cx.notify();
                        }
                    },
                )))
                .child(
                    button("encapsulate-nodes", "Create subpatch   ⇧⌘/Ctrl+E")
                        .debug_selector(|| "encapsulate-nodes".into())
                        .text_xs()
                        .on_click(cx.listener(|this, _, _, cx| this.encapsulate_selection(cx))),
                )
                .child(
                    button("load-subpatch", "Load subpatch…")
                        .debug_selector(|| "load-subpatch".into())
                        .text_xs()
                        .on_click(cx.listener(|this, _, _, cx| this.choose_subpatch_file(cx))),
                )
                .child(
                    button("delete-nodes", "Delete").on_click(cx.listener(|this, _, _, cx| {
                        this.delete_selection(cx);
                        this.patch_menu = None;
                    })),
                )
                .into_any_element(),
        )
    }
    pub(super) fn open_patch_menu(&mut self, pointer: Position) {
        if let Some(world) = self.world_pointer(pointer) {
            self.patch_menu = Some(PatchMenu {
                screen: pointer,
                world,
            });
            self.insert_menu = None;
        }
    }
}
