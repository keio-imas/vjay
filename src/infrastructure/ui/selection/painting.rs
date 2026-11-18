use super::*;
impl Editor {
    pub(in crate::ui) fn selection_overlay(&self) -> gpui::AnyElement {
        let rectangle = self.marquee.as_ref().map(|m| (m.start, m.current));
        let pan = self.pan;
        let z = self.zoom;
        canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                let Some((a, b)) = rectangle else {
                    return;
                };
                if (a.x - b.x).abs() + (a.y - b.y).abs() < 3.0 {
                    return;
                }
                let origin = bounds.origin
                    + point(px(a.x.min(b.x) * z + pan.x), px(a.y.min(b.y) * z + pan.y));
                let area = Bounds::new(
                    origin,
                    size(px((a.x - b.x).abs() * z), px((a.y - b.y).abs() * z)),
                );
                window.paint_quad(gpui::fill(area, gpui::rgba(0x87b7e81c)));
                let mut path = PathBuilder::stroke(px(1.0));
                path.move_to(area.origin);
                path.line_to(point(area.right(), area.top()));
                path.line_to(point(area.right(), area.bottom()));
                path.line_to(point(area.left(), area.bottom()));
                path.close();
                if let Ok(path) = path.build() {
                    window.paint_path(path, rgb(ACCENT));
                }
            },
        )
        .absolute()
        .size_full()
        .into_any_element()
    }
}
