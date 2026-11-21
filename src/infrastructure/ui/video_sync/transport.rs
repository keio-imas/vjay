use super::*;
impl Editor {
    pub(super) fn deck_transport(
        &self,
        video: NodeId,
        rate: f32,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        let playhead = self.session().clip_position(video);
        let seek_bounds = std::rc::Rc::new(std::cell::Cell::new(None::<Bounds<Pixels>>));
        let paint_bounds = seek_bounds.clone();
        let label = playhead.map_or_else(
            || "Load a clip to seek".into(),
            |p| {
                format!(
                    "{:02}:{:06.3} / {:02}:{:06.3}",
                    p.seconds as u64 / 60,
                    p.seconds % 60.0,
                    p.duration as u64 / 60,
                    p.duration % 60.0
                )
            },
        );
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        button("deck-play", if rate > 0.0 { "Pause" } else { "Play" })
                            .bg(rgb(ELEVATED))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let r = this.session_mut().toggle_clip_playback(video);
                                this.apply(r);
                                cx.notify();
                            })),
                    )
                    .child(
                        button("deck-mark", "Set cue")
                            .text_xs()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let r = this.session_mut().mark_cue(video);
                                this.apply(r);
                                cx.notify();
                            })),
                    )
                    .child(button("deck-cue", "Cue").text_xs().on_click(cx.listener(
                        move |this, _, _, cx| {
                            let r = this.session_mut().return_to_cue(video);
                            this.apply(r);
                            cx.notify();
                        },
                    ))),
            )
            .child(div().text_xs().text_color(rgb(MUTED)).child(label))
            .child(
                div()
                    .id("deck-seek")
                    .debug_selector(|| "deck-seek".into())
                    .h(px(18.0))
                    .w_full()
                    .bg(rgb(BACKGROUND))
                    .relative()
                    .child(
                        div()
                            .h_full()
                            .w(relative(playhead.map_or(0.0, |p| p.fraction())))
                            .bg(rgb(ACCENT)),
                    )
                    .child(
                        canvas(
                            move |bounds, _, _| {
                                paint_bounds.set(Some(bounds));
                            },
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .inset_0(),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                            let Some(bounds) = seek_bounds.get() else {
                                return;
                            };
                            window.focus(&this.focus);
                            this.drag = Some(Drag::MediaSeek(
                                video,
                                f32::from(bounds.origin.x),
                                f32::from(bounds.size.width),
                            ));
                            let fraction = f32::from(event.position.x - bounds.origin.x)
                                / f32::from(bounds.size.width);
                            let r = this.session_mut().seek_clip(video, fraction);
                            this.apply(r);
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            )
            .child(
                div()
                    .id("deck-jog")
                    .debug_selector(|| "deck-jog".into())
                    .h(px(34.0))
                    .w_full()
                    .bg(rgb(BACKGROUND))
                    .border_1()
                    .border_color(rgb(BORDER))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_ew_resize()
                    .text_xs()
                    .text_color(rgb(MUTED))
                    .child("Jog ↔   ·   Shift for fine adjustment")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, e: &MouseDownEvent, w, cx| {
                            w.focus(&this.focus);
                            this.drag = Some(Drag::MediaJog(video, position(e.position)));
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            )
            .child(
                div().flex().gap_1().children(
                    [
                        (-0.1, "−100 ms"),
                        (-0.01, "−10 ms"),
                        (0.01, "+10 ms"),
                        (0.1, "+100 ms"),
                    ]
                    .into_iter()
                    .enumerate()
                    .map(|(i, (seconds, label))| {
                        button(SharedString::from(format!("deck-jog-{i}")), label)
                            .px_1()
                            .flex_1()
                            .text_xs()
                            .text_center()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let r = this.session_mut().jog_clip(video, seconds);
                                this.apply(r);
                                cx.notify();
                            }))
                    }),
                ),
            )
            .into_any_element()
    }
    pub(super) fn deck_rate(
        &self,
        sync: NodeId,
        parameter: usize,
        value: f32,
        range: f32,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        let range = [100.0 - range, 100.0 + range];
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(div().flex_1().text_xs().child("Playback rate"))
                    .child(
                        self.number_drag(
                            button("deck-rate", format!("{value:.2}%"))
                                .text_xs()
                                .bg(rgb(BACKGROUND))
                                .border_1()
                                .border_color(rgb(BORDER)),
                            parameter_drag::ValueDrag::new(
                                sync,
                                parameter,
                                value,
                                Position { x: 0.0, y: 0.0 },
                                range,
                                true,
                            ),
                            None,
                            cx,
                        ),
                    ),
            )
            .child(
                div()
                    .id("deck-pitch")
                    .debug_selector(|| "deck-pitch".into())
                    .h(px(16.0))
                    .bg(rgb(BACKGROUND))
                    .cursor_ew_resize()
                    .child(
                        div()
                            .h_full()
                            .w(relative(
                                ((value - range[0]) / (range[1] - range[0])).clamp(0.0, 1.0),
                            ))
                            .bg(rgb(ACCENT)),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, e: &MouseDownEvent, w, cx| {
                            w.focus(&this.focus);
                            this.drag = Some(Drag::Value(parameter_drag::ValueDrag::new(
                                sync,
                                parameter,
                                value,
                                position(e.position),
                                range,
                                false,
                            )));
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            )
            .child(
                div().flex().gap_1().children(
                    [
                        (-0.1, "−0.1"),
                        (-0.01, "−0.01"),
                        (0.01, "+0.01"),
                        (0.1, "+0.1"),
                    ]
                    .into_iter()
                    .enumerate()
                    .map(|(i, (delta, label))| {
                        button(SharedString::from(format!("deck-rate-step-{i}")), label)
                            .text_xs()
                            .px_1()
                            .flex_1()
                            .text_center()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let r = this.session_mut().adjust_deck_rate(sync, delta);
                                this.apply(r);
                                cx.notify();
                            }))
                    }),
                ),
            )
            .child(
                button("deck-reset-rate", "Reset to 100%")
                    .text_xs()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let r = this.session_mut().set_parameter(sync, parameter, 100.0);
                        this.apply(r);
                        cx.notify();
                    })),
            )
            .into_any_element()
    }
}
