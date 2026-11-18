use gpui::{
    Bounds, Context, ElementInputHandler, FocusHandle, MouseButton, Pixels, Point, ShapedLine,
    TextRun, Window, canvas, div, fill, point, prelude::*, px, rgb, size,
};
use std::ops::Range;
mod editing;
mod input_handler;

pub(super) struct TextField {
    pub text: String,
    pub focus: FocusHandle,
    multiline: bool,
    selection: Range<usize>,
    anchor: usize,
    head: usize,
    marked: Option<Range<usize>>,
    lines: Vec<(Range<usize>, ShapedLine, Bounds<Pixels>)>,
}
impl TextField {
    pub fn new(text: String, multiline: bool, cx: &Context<Self>) -> Self {
        let end = text.len();
        Self {
            text,
            focus: cx.focus_handle(),
            multiline,
            selection: end..end,
            anchor: end,
            head: end,
            marked: None,
            lines: Vec::new(),
        }
    }
    fn byte(&self, utf16: usize) -> usize {
        byte_offset(&self.text, utf16)
    }
    fn utf16(&self, byte: usize) -> usize {
        self.text[..byte.min(self.text.len())]
            .encode_utf16()
            .count()
    }
    fn replace(&mut self, range: Range<usize>, text: &str, cx: &mut Context<Self>) {
        if self.text.len() + text.len() - range.len() > 256 * 1024 {
            return;
        }
        self.text.replace_range(range.clone(), text);
        let end = range.start + text.len();
        self.select(end, end);
        self.marked = None;
        cx.notify();
    }
    fn select(&mut self, anchor: usize, head: usize) {
        self.anchor = anchor;
        self.head = head;
        self.selection = anchor.min(head)..anchor.max(head);
    }
    fn index_at(&self, p: Point<Pixels>) -> Option<usize> {
        self.lines
            .iter()
            .min_by_key(|(_, _, b)| ((f32::from(p.y - b.center().y)).abs() * 100.0) as u64)
            .map(|(range, line, b)| range.start + line.closest_index_for_x(p.x - b.left()))
    }
}
fn byte_offset(text: &str, utf16: usize) -> usize {
    let mut units = 0;
    for (index, c) in text.char_indices() {
        if units >= utf16 {
            return index;
        }
        units += c.len_utf16();
    }
    text.len()
}

impl Render for TextField {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let focus = self.focus.clone();
        let text = self.text.clone();
        let selection = self.selection.clone();
        let line_count = if self.multiline {
            text.split('\n').count().max(1)
        } else {
            1
        };
        div()
            .id("text-input")
            .debug_selector(|| "text-input".into())
            .track_focus(&self.focus)
            .relative()
            .w_full()
            .bg(rgb(super::BACKGROUND))
            .border_1()
            .border_color(rgb(super::BORDER))
            .rounded_sm()
            .cursor_text()
            .overflow_y_scroll()
            .on_key_down(cx.listener(Self::key))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                    window.focus(&this.focus);
                    if let Some(byte) = this.index_at(event.position) {
                        this.select(
                            if event.modifiers.shift {
                                this.anchor
                            } else {
                                byte
                            },
                            byte,
                        );
                    }
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
                if event.dragging()
                    && let Some(byte) = this.index_at(event.position)
                {
                    this.select(this.anchor, byte);
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .child(
                canvas(
                    move |_, _, _| (),
                    move |bounds, _, window, cx| {
                        window.handle_input(
                            &focus,
                            ElementInputHandler::new(bounds, entity.clone()),
                            cx,
                        );
                        let mut lines = Vec::new();
                        let mut offset = 0;
                        for (row, text) in text.split('\n').enumerate() {
                            let run = TextRun {
                                len: text.len(),
                                font: window.text_style().font(),
                                color: rgb(super::TEXT).into(),
                                background_color: None,
                                underline: None,
                                strikethrough: None,
                            };
                            let line = window.text_system().shape_line(
                                text.to_string().into(),
                                px(13.0),
                                &[run],
                                None,
                            );
                            let origin =
                                bounds.origin + point(px(6.0), px(4.0 + row as f32 * 20.0));
                            let row_bounds = Bounds::new(origin, size(bounds.size.width, px(20.0)));
                            if focus.is_focused(window)
                                && selection.start <= offset + text.len()
                                && selection.end >= offset
                            {
                                let start = selection.start.saturating_sub(offset).min(text.len());
                                let end = selection.end.saturating_sub(offset).min(text.len());
                                if start != end {
                                    window.paint_quad(fill(
                                        Bounds::new(
                                            origin + point(line.x_for_index(start), px(0.0)),
                                            size(
                                                line.x_for_index(end) - line.x_for_index(start),
                                                px(20.0),
                                            ),
                                        ),
                                        rgb(0x39445b),
                                    ));
                                } else if selection.is_empty() {
                                    window.paint_quad(fill(
                                        Bounds::new(
                                            origin + point(line.x_for_index(start), px(0.0)),
                                            size(px(1.0), px(18.0)),
                                        ),
                                        rgb(super::ACCENT),
                                    ));
                                }
                            }
                            let _ = line.paint(origin, px(20.0), window, cx);
                            lines.push((offset..offset + text.len(), line, row_bounds));
                            offset += text.len() + 1;
                        }
                        entity.update(cx, |this, _| this.lines = lines);
                    },
                )
                .w_full()
                .h(px(line_count as f32 * 20.0 + 8.0)),
            )
    }
}
