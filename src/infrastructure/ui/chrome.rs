use super::*;
use crate::gpu::presentation::PreviewBounds;
use vjay_application::ports::Extent;

impl Editor {
    pub(super) fn toolbar(&self, cx: &Context<Self>) -> gpui::AnyElement {
        let seconds = self.session().seconds();
        let clock = format!(
            "{:02}:{:02}.{:02}",
            seconds as u64 / 60,
            seconds as u64 % 60,
            (seconds.fract() * 100.0) as u64
        );
        div()
            .h(px(40.0))
            .flex()
            .items_center()
            .gap_1()
            .px_3()
            .bg(rgb(SURFACE))
            .border_b_1()
            .border_color(rgb(BORDER))
            .text_xs()
            .child(div().font_weight(FontWeight::SEMIBOLD).child("Vjay"))
            .child(
                div()
                    .text_color(rgb(MUTED))
                    .mr_3()
                    .child(format!("v{}", env!("CARGO_PKG_VERSION"))),
            )
            .child(button("new-project", "New").px_2().on_click(cx.listener(
                |this, _, window, cx| {
                    this.request_project_change(projects::Change::New, window, cx)
                },
            )))
            .child(
                button("load", "Open…")
                    .px_2()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.request_project_change(projects::Change::Open, window, cx)
                    })),
            )
            .child(
                button("save", "Save")
                    .px_2()
                    .on_click(cx.listener(|this, _, _, cx| this.save_project_file(false, cx))),
            )
            .child(
                button("save-as", "Save As…")
                    .px_2()
                    .on_click(cx.listener(|this, _, _, cx| this.save_project_file(true, cx))),
            )
            .child(
                div()
                    .max_w(px(160.0))
                    .truncate()
                    .text_color(rgb(MUTED))
                    .child(format!(
                        "{}{}",
                        self.session().project_name(),
                        if self.session().project_modified() {
                            " *"
                        } else {
                            ""
                        }
                    )),
            )
            .child(div().flex_1())
            .child(div().text_color(rgb(MUTED)).mr_2().child(clock))
            .child(
                button("transport-start", "|< Start")
                    .px_2()
                    .debug_selector(|| "transport-start".into())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.session_mut().seek_start();
                        cx.notify();
                    })),
            )
            .child(
                button("transport-end", "End >|")
                    .px_2()
                    .debug_selector(|| "transport-end".into())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.session_mut().seek_end();
                        cx.notify();
                    })),
            )
            .child(
                button(
                    "pause",
                    if self.session().paused {
                        "Play"
                    } else {
                        "Pause"
                    },
                )
                .px_2()
                .when(self.session().paused, |e| {
                    e.bg(rgb(ELEVATED)).text_color(rgb(ACCENT))
                })
                .on_click(cx.listener(|this, _, _, cx| {
                    let session = this.session_mut();
                    session.paused = !session.paused;
                    cx.notify();
                })),
            )
            .child(
                button("tap-tempo", "Tap")
                    .px_2()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.session_mut().tap_tempo(None);
                        cx.notify();
                    })),
            )
            .child(div().w(px(1.0)).h(px(16.0)).mx_2().bg(rgb(BORDER)))
            .child(
                button("output-window", "Open output")
                    .px_2()
                    .on_click(cx.listener(|this, _, _, cx| this.open_output(cx))),
            )
            .child(div().w(px(1.0)).h(px(16.0)).mx_2().bg(rgb(BORDER)))
            .children(
                [(false, "patch-mode", "Patch"), (true, "mode", "Perform")].map(
                    |(perform, id, label)| {
                        button(id, label)
                            .px_2()
                            .when(self.perform == perform, |e| {
                                e.bg(rgb(ELEVATED)).text_color(rgb(ACCENT))
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if this.perform != perform {
                                    this.cancel_gesture();
                                    this.selection.clear();
                                    this.binding_picker = None;
                                    this.zoom = this.session().project().workspace.patch_zoom;
                                    this.pan = Position { x: 0.0, y: 0.0 };
                                    this.selected_control = None;
                                    this.perform = perform;
                                }
                                cx.notify();
                            }))
                    },
                ),
            )
            .into_any_element()
    }

    pub(super) fn preview_element(&self, cx: &Context<Self>) -> impl IntoElement {
        let preview = self.preview.clone();
        let extent = self.session().extent();
        let empty = self.session().plan().is_some_and(|p| !p.program_active);
        div()
            .flex()
            .flex_col()
            .w(relative(self.session().project().workspace.preview_ratio))
            .flex_shrink_0()
            .min_w_0()
            .bg(rgb(BACKGROUND))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .h(px(30.0))
                    .px_2()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .child(
                        div()
                            .flex_1()
                            .font_weight(FontWeight::MEDIUM)
                            .child("Program"),
                    )
                    .children(
                        [(960, 540), (1280, 720), (1920, 1080)].map(|(width, height)| {
                            button(
                                SharedString::from(format!("res-{height}")),
                                format!("{height}p"),
                            )
                            .text_xs()
                            .px_1()
                            .when(extent.width == width, |element| {
                                element.text_color(rgb(ACCENT)).bg(rgb(ELEVATED))
                            })
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    let result =
                                        this.session_mut().set_extent(Extent { width, height });
                                    this.apply(result);
                                    cx.notify();
                                },
                            ))
                        }),
                    ),
            )
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .bg(rgb(0x101214))
                    .child(
                        canvas(
                            move |_, _, _| (),
                            move |bounds, _, window, _| {
                                if let Some(preview) = &preview {
                                    let available_w: f32 = bounds.size.width.into();
                                    let available_h: f32 = bounds.size.height.into();
                                    let ratio = extent.width as f32 / extent.height as f32;
                                    let width = available_w.min(available_h * ratio);
                                    let height = width / ratio;
                                    preview.borrow_mut().set_bounds(PreviewBounds {
                                        x: f32::from(bounds.origin.x) + (available_w - width) * 0.5,
                                        y: f32::from(bounds.origin.y)
                                            + (available_h - height) * 0.5,
                                        width,
                                        height,
                                        window_height: window.viewport_size().height.into(),
                                        scale: window.scale_factor(),
                                    });
                                }
                            },
                        )
                        .size_full(),
                    )
                    .when(empty, |e| {
                        e.child(
                            div()
                                .absolute()
                                .left(px(12.0))
                                .top(px(12.0))
                                .right(px(12.0))
                                .text_xs()
                                .text_color(rgb(MUTED))
                                .child(if self.session().project().output.is_some() {
                                    "Connect an image source to Output."
                                } else {
                                    "Add an Output node."
                                }),
                        )
                    }),
            )
            .when(self.preview.is_none(), |element| {
                element.child(
                    div()
                        .p_2()
                        .text_xs()
                        .text_color(rgb(MUTED))
                        .child("Native preview unavailable on this platform"),
                )
            })
    }
}
