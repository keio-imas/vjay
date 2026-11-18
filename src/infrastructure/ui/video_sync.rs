use super::*;
impl Editor {
    pub(super) fn video_sync_panel(&self, video: NodeId, cx: &Context<Self>) -> gpui::AnyElement {
        let monitored = self.session().monitored_video(video);
        let mut p = div().flex().flex_col().gap_2().child(
            div()
                .mt_2()
                .pt_2()
                .border_t_1()
                .border_color(rgb(BORDER))
                .font_weight(FontWeight::MEDIUM)
                .child("BPM Sync / Monitor"),
        );
        p = p
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(MUTED))
                    .child("Match the clip to the DJ. Monitor audio is local."),
            )
            .child(
                button(
                    "monitor-clip",
                    if monitored {
                        "Stop monitoring"
                    } else {
                        "Monitor clip / headphones"
                    },
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    let r = this.session_mut().monitor_video(video, !monitored);
                    this.apply(r);
                    cx.notify();
                })),
            );
        p = p.child(
            button(
                "monitor-device",
                format!("Monitor: {} ▾", self.session().monitor_device()),
            )
            .text_xs()
            .truncate()
            .on_click(cx.listener(|this, _, window, cx| this.open_monitor_picker(window, cx))),
        );
        if let Some(sync) = self.session().video_sync(video) {
            let binding = self
                .session()
                .schema(sync)
                .capabilities
                .tempo_sync
                .expect("BPM Sync contract");
            p = p
                .child(self.parameter(sync, binding.clip_bpm, false, cx))
                .child(self.parameter(sync, binding.dj_bpm, false, cx))
                .child(button("tap-dj-bpm", "Tap DJ BPM").on_click(cx.listener(
                    move |this, _, _, cx| {
                        this.session_mut().tap_dj_bpm(sync);
                        cx.notify();
                    },
                )))
                .child(self.parameter(sync, binding.cue, false, cx))
                .child(
                    button("align-video", "Align clip cue now").on_click(cx.listener(
                        move |this, _, _, cx| {
                            let r = this.session_mut().align_video(sync);
                            this.apply(r);
                            cx.notify();
                        },
                    )),
                )
                .child(self.parameter(sync, binding.trim, false, cx));
            p = p.child(
                div().flex().gap_2().children(
                    [(-0.01, "Cue −10 ms"), (0.01, "Cue +10 ms")]
                        .into_iter()
                        .enumerate()
                        .map(|(i, (seconds, label))| {
                            button(SharedString::from(format!("cue-jog-{i}")), label)
                                .text_xs()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    let r = this.session_mut().jog_sync_cue(sync, seconds);
                                    this.apply(r);
                                    cx.notify();
                                }))
                        }),
                ),
            );
            p = p.child(
                div().flex().gap_2().children(
                    [(-2.0, "Nudge −"), (2.0, "Nudge +")]
                        .into_iter()
                        .enumerate()
                        .map(|(i, (value, label))| {
                            button(SharedString::from(format!("sync-nudge-{i}")), label)
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        let r = this.session_mut().set_parameter(
                                            sync,
                                            binding.nudge,
                                            value,
                                        );
                                        this.apply(r);
                                        cx.stop_propagation();
                                        cx.notify();
                                    }),
                                )
                                .on_mouse_up(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        let r = this.session_mut().set_parameter(
                                            sync,
                                            binding.nudge,
                                            0.0,
                                        );
                                        this.apply(r);
                                        cx.notify();
                                    }),
                                )
                                .on_mouse_up_out(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        let r = this.session_mut().set_parameter(
                                            sync,
                                            binding.nudge,
                                            0.0,
                                        );
                                        this.apply(r);
                                        cx.notify();
                                    }),
                                )
                        }),
                ),
            );
            p = p.child(
                button("sync-clip-duration", "Use clip duration").on_click(cx.listener(
                    move |this, _, _, cx| {
                        let r = this.session_mut().sync_clip_duration(video, sync);
                        this.apply(r);
                        cx.notify();
                    },
                )),
            );
        } else {
            p = p.child(
                button("setup-bpm-sync", "Set up BPM Sync").on_click(cx.listener(
                    move |this, _, _, cx| {
                        let r = this.session_mut().setup_video_sync(video);
                        this.apply(r);
                        cx.notify();
                    },
                )),
            );
        }
        p.into_any_element()
    }
}
