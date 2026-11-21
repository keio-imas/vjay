use super::*;
use crate::gpu::presentation::PreviewBounds;

pub(super) struct OutputView {
    preview: SharedPreview,
    _editor: Entity<Editor>,
    extent: std::rc::Rc<std::cell::Cell<vjay_application::ports::Extent>>,
}
impl Editor {
    pub(super) fn open_output(&mut self, cx: &mut Context<Self>) {
        let source = self
            .selection
            .primary()
            .filter(|id| self.session().window_output(*id))
            .or(self
                .session()
                .project()
                .output
                .filter(|id| self.session().window_output(*id)));
        let Some(source) = source else {
            self.message = "Add an Output node first".into();
            return;
        };
        self.open_output_node(source, cx);
    }
    pub(super) fn open_output_node(&mut self, source: NodeId, cx: &mut Context<Self>) {
        if let Some((_, handle)) = self.output_windows.iter().find(|(id, _)| *id == source)
            && handle
                .update(cx, |_, window, _| window.activate_window())
                .is_ok()
        {
            return;
        }
        self.output_windows.retain(|(id, _)| *id != source);
        let Some(factory) = self.outputs.clone() else {
            self.message = "Output windows are not available on this platform".into();
            cx.notify();
            return;
        };
        let editor = cx.entity();
        let extent = self.output_extent.clone();
        let settings = self.session().preferences().video.clone();
        let display = cx
            .displays()
            .into_iter()
            .find(|d| d.uuid().is_ok_and(|id| id.to_string() == settings.display));
        let display_id = display.as_ref().map(|d| d.id());
        let bounds = Bounds::centered(display_id, size(px(960.0), px(540.0)), cx);
        let opened = cx.open_window(
            WindowOptions {
                window_bounds: Some(if settings.fullscreen {
                    WindowBounds::Fullscreen(bounds)
                } else {
                    WindowBounds::Windowed(bounds)
                }),
                display_id,
                titlebar: Some(TitlebarOptions {
                    title: Some(format!("VJAY · Output #{}", source.0).into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            move |window, cx| {
                let preview = factory.create(window, source);
                cx.new(|_| OutputRoot {
                    view: preview.map(|preview| OutputView {
                        preview,
                        _editor: editor,
                        extent,
                    }),
                })
            },
        );
        match opened {
            Ok(handle) => self.output_windows.push((source, handle)),
            Err(error) => self.message = error.to_string(),
        }
        cx.notify();
    }
}
pub(super) struct OutputRoot {
    view: PortResult<OutputView>,
}
impl Render for OutputRoot {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let Ok(view) = &self.view else {
            return div()
                .p_4()
                .child(self.view.as_ref().err().cloned().unwrap_or_default())
                .into_any_element();
        };
        let preview = view.preview.clone();
        let extent = view.extent.get();
        window.request_animation_frame();
        div()
            .size_full()
            .bg(rgb(0x000000))
            .child(
                canvas(
                    move |_, _, _| (),
                    move |bounds, _, window, _| {
                        let available_w: f32 = bounds.size.width.into();
                        let available_h: f32 = bounds.size.height.into();
                        let ratio = extent.width as f32 / extent.height as f32;
                        let width = available_w.min(available_h * ratio);
                        let height = width / ratio;
                        preview.borrow_mut().set_bounds(PreviewBounds {
                            x: f32::from(bounds.origin.x) + (available_w - width) * 0.5,
                            y: f32::from(bounds.origin.y) + (available_h - height) * 0.5,
                            width,
                            height,
                            window_height: window.viewport_size().height.into(),
                            scale: window.scale_factor(),
                        });
                    },
                )
                .size_full(),
            )
            .into_any_element()
    }
}
