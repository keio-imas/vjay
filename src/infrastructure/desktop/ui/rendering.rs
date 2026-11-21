use super::*;

impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(session) = self.session.as_mut() else {
            return div()
                .size_full()
                .p_8()
                .bg(rgb(0x0e1016))
                .text_color(rgb(0xfca5a5))
                .child(format!("VJAY initialization failed: {}", self.message))
                .into_any_element();
        };
        let mut layout = session.project().workspace;
        let width = f32::from(window.viewport_size().width);
        let height = f32::from(window.viewport_size().height);
        layout.library_width = layout
            .library_width
            .min((width - 640.0).clamp(120.0, 600.0));
        layout.inspector_width = layout
            .inspector_width
            .min((width - layout.library_width - 420.0).clamp(220.0, 650.0));
        layout.top_height = layout.top_height.min((height - 260.0).clamp(140.0, 1400.0));
        let main_width = (width - layout.library_width - layout.inspector_width - 18.0).max(1.0);
        let minimum_ratio = (200.0 / main_width).clamp(0.2, 0.5);
        layout.preview_ratio = layout
            .preview_ratio
            .clamp(minimum_ratio, 1.0 - minimum_ratio);
        if layout != session.project().workspace
            && let Err(error) = session.set_workspace(layout)
        {
            self.message = error;
        }
        let status = self
            .render_error
            .as_deref()
            .or(session.audio_error.as_deref())
            .or((!self.message.is_empty()).then_some(self.message.as_str()))
            .or(session.media_status())
            .unwrap_or("")
            .to_owned();
        if let Some(preview) = &self.preview {
            preview.borrow_mut().set_nodes_hidden(
                self.perform
                    || self.settings.is_some()
                    || self.configuration.is_some()
                    || self.insert_menu.is_some()
                    || self.patch_menu.is_some()
                    || self.category_menu
                    || self.control_menu
                    || self.palette.is_some()
                    || self.parameter_menu.is_some()
                    || self.binding_picker.is_some()
                    || self.font_picker.is_some(),
            );
            preview.borrow_mut().begin_nodes_frame();
            preview.borrow_mut().set_hidden(
                self.settings.is_some()
                    || self.configuration.is_some()
                    || self.palette.is_some()
                    || self.binding_picker.is_some()
                    || self.font_picker.is_some()
                    || session.plan().is_some_and(|p| !p.program_active),
            );
        }
        let node_previews = self.preview.clone();
        div()
            .id("studio")
            .relative()
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(BACKGROUND))
            .text_color(rgb(TEXT))
            .text_sm()
            .on_mouse_move(cx.listener(Self::mouse_move))
            .on_key_down(cx.listener(Self::key_down))
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if this.palette.is_some() {
                    this.key_down(event, window, cx);
                }
            }))
            .on_key_up(cx.listener(|this, event: &KeyUpEvent, _, _| {
                this.session_mut()
                    .input_event(vjay_application::control_io::ControlEvent::Key {
                        key: event.keystroke.key.to_lowercase(),
                        down: false,
                    })
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    window.focus(&this.focus, cx);
                    if this.insert_menu.take().is_some() || this.patch_menu.take().is_some() {
                        cx.notify();
                    }
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.finish_gesture();
                    this.cable = None;
                    cx.notify();
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.finish_gesture();
                    this.cable = None;
                    cx.notify();
                }),
            )
            .on_mouse_up(
                MouseButton::Middle,
                cx.listener(|this, _, _, cx| {
                    this.finish_gesture();
                    cx.notify();
                }),
            )
            .on_mouse_up_out(
                MouseButton::Middle,
                cx.listener(|this, _, _, cx| {
                    this.finish_gesture();
                    cx.notify();
                }),
            )
            .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                if let Some(path) = paths.paths().first() {
                    this.import_video(path.clone(), None);
                    cx.notify();
                }
            }))
            .child(self.toolbar(cx))
            .child(self.input_sources(cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.library(cx))
                    .child(self.splitter("library-divider", Panel::Library, cx))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .flex()
                                    .h(px(layout
                                        .top_height
                                        .min(f32::from(window.viewport_size().height) - 260.0)
                                        .max(140.0)))
                                    .border_b_1()
                                    .border_color(rgb(BORDER))
                                    .when(self.perform, |element| element.flex_1())
                                    .child(self.preview_element(cx))
                                    .child(self.splitter("preview-divider", Panel::Preview, cx))
                                    .child(self.deck(cx)),
                            )
                            .when(!self.perform, |element| {
                                element
                                    .child(self.splitter("patch-divider", Panel::Top, cx))
                                    .child(self.patch(cx))
                            }),
                    )
                    .child(self.splitter("inspector-divider", Panel::Inspector, cx))
                    .child(self.inspector(cx)),
            )
            .child(self.status_bar(status))
            .children(self.insert_popup(cx, width, height))
            .children(self.patch_popup(cx, width, height))
            .children(self.palette_popup(cx, width, height))
            .children(self.parameter_popup(cx, width, height))
            .children(self.binding_popup(cx, width, height))
            .children(self.font_popup(cx, width, height))
            .children(self.configuration_popup(cx, width, height))
            .children(self.settings_popup(cx, width, height))
            .child(
                canvas(
                    |_, _, _| (),
                    move |_, _, _, _| {
                        if let Some(p) = &node_previews {
                            p.borrow_mut().finish_nodes_frame();
                        }
                    },
                )
                .absolute()
                .size_full(),
            )
            .into_any_element()
    }
}
