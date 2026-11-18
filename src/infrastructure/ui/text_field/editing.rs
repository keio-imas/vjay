use super::*;
use gpui::{ClipboardItem, KeyDownEvent};
impl TextField {
    pub(super) fn key(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        let command = event.keystroke.modifiers.platform || event.keystroke.modifiers.control;
        if command {
            match key {
                "a" => self.select(0, self.text.len()),
                "c" => cx.write_to_clipboard(ClipboardItem::new_string(
                    self.text[self.selection.clone()].into(),
                )),
                "x" => {
                    cx.write_to_clipboard(ClipboardItem::new_string(
                        self.text[self.selection.clone()].into(),
                    ));
                    self.replace(self.selection.clone(), "", cx);
                }
                "v" => {
                    if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                        let text = if self.multiline {
                            text
                        } else {
                            text.replace('\n', " ")
                        };
                        self.replace(self.selection.clone(), &text, cx);
                    }
                }
                _ => return,
            }
            cx.stop_propagation();
            cx.notify();
            return;
        }
        let cursor = self.head;
        let previous = self.text[..cursor]
            .char_indices()
            .next_back()
            .map_or(0, |(i, _)| i);
        let next = self.text[cursor..]
            .chars()
            .next()
            .map_or(cursor, |c| cursor + c.len_utf8());
        match key {
            "backspace" => self.replace(
                if self.selection.is_empty() {
                    previous..cursor
                } else {
                    self.selection.clone()
                },
                "",
                cx,
            ),
            "delete" => self.replace(
                if self.selection.is_empty() {
                    cursor..next
                } else {
                    self.selection.clone()
                },
                "",
                cx,
            ),
            "left" | "right" | "up" | "down" | "home" | "end" => {
                let to = match key {
                    "left" if !event.keystroke.modifiers.shift && !self.selection.is_empty() => {
                        self.selection.start
                    }
                    "right" if !event.keystroke.modifiers.shift && !self.selection.is_empty() => {
                        self.selection.end
                    }
                    "left" => previous,
                    "right" => next,
                    "home" => self.text[..cursor].rfind('\n').map_or(0, |i| i + 1),
                    "end" => self.text[cursor..]
                        .find('\n')
                        .map_or(self.text.len(), |i| cursor + i),
                    "up" => self.vertical(cursor, false),
                    _ => self.vertical(cursor, true),
                };
                self.select(
                    if event.keystroke.modifiers.shift {
                        self.anchor
                    } else {
                        to
                    },
                    to,
                );
            }
            "enter" if self.multiline => self.replace(self.selection.clone(), "\n", cx),
            "tab" if self.multiline => self.replace(self.selection.clone(), "    ", cx),
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
    }
    fn vertical(&self, cursor: usize, down: bool) -> usize {
        let start = self.text[..cursor].rfind('\n').map_or(0, |i| i + 1);
        let column = self.text[start..cursor].chars().count();
        let (a, b) = if down {
            let Some(end) = self.text[cursor..].find('\n').map(|i| cursor + i) else {
                return self.text.len();
            };
            let a = end + 1;
            (
                a,
                self.text[a..].find('\n').map_or(self.text.len(), |i| a + i),
            )
        } else {
            if start == 0 {
                return 0;
            }
            let b = start - 1;
            (self.text[..b].rfind('\n').map_or(0, |i| i + 1), b)
        };
        self.text[a..b]
            .char_indices()
            .nth(column)
            .map_or(b, |(i, _)| a + i)
    }
}
