use super::*;
mod transport;

impl Editor {
    pub(super) fn video_sync_panel(&self, video: NodeId, cx: &Context<Self>) -> gpui::AnyElement {
        let monitored = self.session().monitored_video(video);
        let mut panel = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(inspector::section("Cue / Beat match"))
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(MUTED))
                    .child("Listen on headphones. Match speed, then hold Nudge to align the beat."),
            )
            .child(
                button(
                    "monitor-clip",
                    if monitored {
                        "Cue audio: On"
                    } else {
                        "Cue audio: Off"
                    },
                )
                .bg(rgb(if monitored { ELEVATED } else { BACKGROUND }))
                .on_click(cx.listener(move |this, _, _, cx| {
                    let r = this.session_mut().monitor_video(video, !monitored);
                    this.apply(r);
                    cx.notify();
                })),
            )
            .child(
                button(
                    "monitor-device",
                    format!("Audio: {} ▾", self.session().monitor_device()),
                )
                .text_xs()
                .truncate()
                .on_click(cx.listener(|this, _, w, cx| this.open_monitor_picker(w, cx))),
            );
        let Some(sync) = self.session().video_sync(video) else {
            return panel.child(button("setup-bpm-sync","Connect Deck Sync…").bg(rgb(ELEVATED))
                .debug_selector(||"setup-deck-sync".into())
                .on_click(cx.listener(move|this,_,_,cx|{
                    let r=this.session_mut().setup_video_sync(video);this.apply(r);cx.notify();
                })))
                .child(div().text_xs().text_color(rgb(MUTED)).child("Adds a controller and connects Speed, Position and Seek. Existing connections are preserved."))
                .into_any_element();
        };
        let binding = self
            .session()
            .schema(sync)
            .capabilities
            .tempo_sync
            .expect("Deck contract");
        if let Some(manual) = binding.manual {
            panel = panel.child(self.parameter(sync, manual.mode, true, cx));
            let node = self
                .session()
                .project()
                .graph
                .node(sync)
                .expect("controller");
            if node.parameters[manual.mode] < 0.5 {
                let rate = node.parameters[manual.rate];
                let range = node.parameters[manual.range];
                panel = panel
                    .child(self.deck_transport(video, rate, cx))
                    .child(self.deck_rate(sync, manual.rate, rate, range, cx))
                    .child(self.parameter(sync, manual.range, true, cx));
            } else {
                panel = panel.child(self.deck_bpm(sync, binding, cx));
            }
        } else {
            panel = panel.child(self.deck_bpm(sync, binding, cx));
        }
        panel=panel.child(div().flex().gap_2().children(
            [(-2.0,"Nudge −"),(2.0,"Nudge +")].into_iter().enumerate().map(|(i,(value,label))| {
                button(SharedString::from(format!("sync-nudge-{i}")),label).flex_1().text_center().bg(rgb(ELEVATED))
                    .on_mouse_down(MouseButton::Left,cx.listener(move|this,_,w,cx|{
                        w.focus(&this.focus);let r=this.session_mut().set_parameter(sync,binding.nudge,value);
                        this.apply(r);cx.stop_propagation();cx.notify();
                    }))
                    .on_mouse_up(MouseButton::Left,cx.listener(move|this,_,_,cx|{
                        let r=this.session_mut().set_parameter(sync,binding.nudge,0.0);this.apply(r);cx.notify();
                    }))
                    .on_mouse_up_out(MouseButton::Left,cx.listener(move|this,_,_,cx|{
                        let r=this.session_mut().set_parameter(sync,binding.nudge,0.0);this.apply(r);cx.notify();
                    }))
            })
        )).child(div().text_xs().text_color(rgb(MUTED)).child("Nudge changes speed by 2% while held. Release keeps the adjusted position and restores speed."));
        panel.into_any_element()
    }
    fn deck_bpm(
        &self,
        sync: NodeId,
        b: vjay_domain::capabilities::TempoSyncBinding,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(self.parameter(sync, b.clip_bpm, true, cx))
            .child(self.parameter(sync, b.dj_bpm, true, cx))
            .child(button("tap-dj-bpm", "Tap DJ tempo").on_click(cx.listener(
                move |this, _, _, cx| {
                    this.session_mut().tap_dj_bpm(sync);
                    cx.notify();
                },
            )))
            .child(self.parameter(sync, b.trim, true, cx))
            .child(
                button("align-video", "Align clip cue now").on_click(cx.listener(
                    move |this, _, _, cx| {
                        let r = this.session_mut().align_video(sync);
                        this.apply(r);
                        cx.notify();
                    },
                )),
            )
            .into_any_element()
    }
}
