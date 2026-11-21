use super::*;

#[derive(Clone, Copy)]
pub(super) enum Panel {
    Library,
    Inspector,
    Top,
    Preview,
}
impl Editor {
    pub(super) fn splitter(
        &self,
        id: &'static str,
        panel: Panel,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let layout = self.session().project().workspace;
        div()
            .id(id)
            .debug_selector(|| id.to_string())
            .flex_shrink_0()
            .bg(rgb(SURFACE))
            .hover(|style| style.bg(rgb(ACCENT)))
            .when(matches!(panel, Panel::Top), |element| {
                element.h(px(6.0)).w_full().cursor_row_resize()
            })
            .when(!matches!(panel, Panel::Top), |element| {
                element.w(px(6.0)).h_full().cursor_col_resize()
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.drag = Some(Drag::Panel(panel, layout, position(event.position)));
                    this.cable = None;
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
    }
    pub(super) fn resize_panel(
        &mut self,
        panel: Panel,
        mut layout: WorkspaceLayout,
        start: Position,
        pointer: Position,
        window: &Window,
    ) {
        let dx = pointer.x - start.x;
        let dy = pointer.y - start.y;
        let width: f32 = window.viewport_size().width.into();
        let height: f32 = window.viewport_size().height.into();
        match panel {
            Panel::Library => {
                layout.library_width = (layout.library_width + dx).clamp(
                    120.0,
                    (width - layout.inspector_width - 420.0).clamp(120.0, 600.0),
                )
            }
            Panel::Inspector => {
                layout.inspector_width = (layout.inspector_width - dx).clamp(
                    220.0,
                    (width - layout.library_width - 420.0).clamp(220.0, 650.0),
                )
            }
            Panel::Top => {
                layout.top_height =
                    (layout.top_height + dy).clamp(140.0, (height - 260.0).clamp(140.0, 1400.0))
            }
            Panel::Preview => {
                layout.preview_ratio = (layout.preview_ratio
                    + dx / (width - layout.library_width - layout.inspector_width - 18.0).max(1.0))
                .clamp(0.2, 0.8)
            }
        }
        let result = self.session_mut().set_workspace(layout);
        self.apply(result);
    }
}
