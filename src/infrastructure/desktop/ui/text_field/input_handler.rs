use super::*;
use gpui::{EntityInputHandler, UTF16Selection};

impl EntityInputHandler for TextField {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.byte(range.start)..self.byte(range.end);
        *actual = Some(self.utf16(range.start)..self.utf16(range.end));
        Some(self.text[range].into())
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.utf16(self.selection.start)..self.utf16(self.selection.end),
            reversed: self.head < self.anchor,
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked
            .as_ref()
            .map(|r| self.utf16(r.start)..self.utf16(r.end))
    }
    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.marked = None;
    }
    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range
            .map(|r| self.byte(r.start)..self.byte(r.end))
            .or(self.marked.clone())
            .unwrap_or(self.selection.clone());
        let text = if self.multiline {
            text.to_string()
        } else {
            text.replace('\n', " ")
        };
        self.replace(range, &text, cx);
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range
            .map(|r| self.byte(r.start)..self.byte(r.end))
            .or(self.marked.clone())
            .unwrap_or(self.selection.clone());
        let start = range.start;
        self.replace(range, text, cx);
        self.marked = Some(start..start + text.len());
        if let Some(selected) = selected {
            self.select(
                start + byte_offset(text, selected.start),
                start + byte_offset(text, selected.end),
            );
        }
    }
    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let offset = self.byte(range.start);
        self.lines
            .iter()
            .find(|(range, _, _)| range.contains(&offset) || range.end == offset)
            .map(|(range, line, bounds)| {
                Bounds::new(
                    point(
                        bounds.left() + line.x_for_index(offset - range.start),
                        bounds.top(),
                    ),
                    size(px(2.0), px(20.0)),
                )
            })
            .or(Some(bounds))
    }
    fn character_index_for_point(
        &mut self,
        position: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        self.lines
            .iter()
            .find(|(_, _, b)| position.y >= b.top() && position.y <= b.bottom())
            .map(|(range, line, b)| {
                self.utf16(range.start + line.closest_index_for_x(position.x - b.left()))
            })
    }
}
