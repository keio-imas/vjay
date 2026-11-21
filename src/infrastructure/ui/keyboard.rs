use super::*;
use vjay_application::control_io::ControlEvent;

impl Editor {
    pub(super) fn key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.keystroke.key == "escape" {
            if let Some(s) = &mut self.settings {
                if s.menu_open() {
                    s.close_menu();
                } else {
                    self.settings = None;
                }
                window.focus(&self.focus);
                cx.stop_propagation();
                cx.notify();
                return;
            }
            self.cable = None;
            self.cancel_gesture();
            self.insert_menu = None;
            self.patch_menu = None;
            self.configuration = None;
            self.control_menu = false;
            self.category_menu = false;
            self.palette = None;
            self.parameter_menu = None;
            self.binding_picker = None;
            self.font_picker = None;
            window.focus(&self.focus);
            cx.stop_propagation();
            cx.notify();
            return;
        }
        if event.keystroke.key == ","
            && (event.keystroke.modifiers.platform || event.keystroke.modifiers.control)
        {
            self.open_settings(window, cx);
            cx.stop_propagation();
            return;
        }
        if self.settings.is_some() {
            return;
        }
        if self.project_key(event, window, cx) {
            return;
        }
        if self.palette_key(event, window, cx) {
            return;
        }
        if self.font_search_focused(window, cx)
            || self.binding_search_focused(window, cx)
            || self.text_editing(window, cx)
            || self.configuration.is_some()
        {
            return;
        }
        if !self.perform && event.keystroke.key == "tab" {
            self.open_palette(window, cx);
            cx.stop_propagation();
            return;
        }
        if !self.perform
            && (event.keystroke.modifiers.platform || event.keystroke.modifiers.control)
        {
            match event.keystroke.key.as_str() {
                "c" => {
                    self.copy_patch(cx);
                    cx.stop_propagation();
                    return;
                }
                "v" => {
                    self.paste_patch(None, cx);
                    cx.stop_propagation();
                    return;
                }
                "x" => {
                    if self.copy_patch(cx) {
                        self.delete_selection(cx);
                    } else {
                        cx.notify();
                    }
                    cx.stop_propagation();
                    return;
                }
                "d" => {
                    if self.copy_patch(cx) {
                        self.paste_patch(None, cx);
                    } else {
                        cx.notify();
                    }
                    cx.stop_propagation();
                    return;
                }
                _ => (),
            }
        }
        if !self.perform
            && event.keystroke.key == "a"
            && (event.keystroke.modifiers.platform || event.keystroke.modifiers.control)
        {
            self.select_all_nodes();
            cx.stop_propagation();
            cx.notify();
            return;
        }
        if matches!(event.keystroke.key.as_str(), "backspace" | "delete") {
            self.delete_selection(cx);
            cx.stop_propagation();
            return;
        }
        self.session_mut().input_event(ControlEvent::Key {
            key: event.keystroke.key.to_lowercase(),
            down: true,
        });
    }
    pub(super) fn delete_selection(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.selected_control.take() {
            self.session_mut().remove_control(id);
        } else if !self.perform {
            let ids: Vec<_> = self.selection.ids.iter().copied().collect();
            match self.session_mut().remove_nodes(&ids) {
                Ok(()) => {
                    self.selection.clear();
                    self.message.clear();
                }
                Err(error) => self.message = error,
            }
        }
        self.cable = None;
        self.finish_gesture();
        self.insert_menu = None;
        self.parameter_menu = None;
        self.patch_menu = None;
        self.palette = None;
        self.binding_picker = None;
        cx.notify();
    }
}
