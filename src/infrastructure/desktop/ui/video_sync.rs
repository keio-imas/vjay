use super::*;
mod transport;
impl Editor {
    pub(super) fn video_sync_panel(&self, video: NodeId, cx: &Context<Self>) -> gpui::AnyElement {
        let monitored = self.session().monitored_video(video);
        let speed = self.session().media_state(video).map_or(1.0, |s| s.speed);
        div().flex().flex_col().gap_2().child(inspector::section("Transport / Cue"))
   .child(button("monitor-clip",if monitored {"Cue audio: On"}else{"Cue audio: Off"}).bg(rgb(BACKGROUND)).on_click(cx.listener(move|this,_,_,cx|{let r=this.session_mut().monitor_video(video,!monitored);this.apply(r);cx.notify();})))
   .child(button("monitor-device",format!("Audio: {} ▾",self.session().monitor_device())).text_xs().truncate().on_click(cx.listener(|this,_,w,cx|this.open_monitor_picker(w,cx))))
   .child(self.deck_transport(video,speed,cx))
   .child(div().flex().gap_2().children([(-1.0,"Nudge −"),(1.0,"Nudge +")].into_iter().enumerate().map(|(i,(value,label))|
    button(SharedString::from(format!("source-nudge-{i}")),label).debug_selector(move||format!("source-nudge-{i}")).flex_1().text_center().bg(rgb(ELEVATED))
     .on_mouse_down(MouseButton::Left,cx.listener(move|this,_,w,cx|{w.focus(&this.focus);this.drag=Some(Drag::MediaNudge(video));let r=this.session_mut().nudge_clip(video,value);this.apply(r);cx.stop_propagation();cx.notify();}))
     .on_mouse_up(MouseButton::Left,cx.listener(move|this,_,_,cx|{let r=this.session_mut().nudge_clip(video,0.0);this.apply(r);cx.notify();}))
     .on_mouse_up_out(MouseButton::Left,cx.listener(move|this,_,_,cx|{let r=this.session_mut().nudge_clip(video,0.0);this.apply(r);cx.notify();})))))
   .child(div().text_xs().text_color(rgb(MUTED)).child("Connect buttons, Keyboard, MIDI or OSC to this node's transport inputs. Nudge is ±2% while held."))
   .child(self.parameter(video,self.session().schema(video).capabilities.media.expect("media").speed,false,cx))
   .child(div().flex().gap_2()
    .child(button("add-rate-controller","Add rate control…").text_xs().on_click(cx.listener(move|this,_,_,cx|{let r=this.session_mut().setup_video_sync(video);match r {Ok(id)=>{this.selection.only(id);},Err(e)=>this.message=e}cx.notify();})))
    .child(button("add-seek-control","Add seek control…").text_xs().on_click(cx.listener(move|this,_,_,cx|{let r=this.session_mut().setup_seek_control(video);match r {Ok(id)=>{this.selection.only(id);},Err(e)=>this.message=e}cx.notify();}))))
   .into_any_element()
    }
    pub(super) fn rate_controller_panel(
        &self,
        node: NodeId,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        let caps = self.session().schema(node).capabilities;
        let b = caps
            .rate
            .or_else(|| {
                caps.tempo_sync.and_then(|b| {
                    b.manual.map(|m| vjay_domain::capabilities::RateBinding {
                        mode: m.mode,
                        rate: m.rate,
                        range: m.range,
                        clip_bpm: b.clip_bpm,
                        dj_bpm: b.dj_bpm,
                        trim: b.trim,
                        speed_output: b.outputs[0],
                    })
                })
            })
            .expect("rate metadata");
        let values = &self
            .session()
            .project()
            .graph
            .node(node)
            .expect("controller")
            .parameters;
        let mut panel = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(inspector::section("Rate control"))
            .child(self.parameter(node, b.mode, true, cx));
        if values[b.mode] < 0.5 {
            panel = panel
                .child(self.deck_rate(node, b.rate, values[b.rate], values[b.range], cx))
                .child(self.parameter(node, b.range, true, cx));
        } else {
            panel = panel
                .child(self.parameter(node, b.clip_bpm, true, cx))
                .child(self.parameter(node, b.dj_bpm, true, cx))
                .child(self.parameter(node, b.trim, true, cx))
                .child(button("tap-dj-bpm", "Tap tempo").on_click(cx.listener(
                    move |this, _, _, cx| {
                        this.session_mut().tap_dj_bpm(node);
                        cx.notify();
                    },
                )));
        }
        panel.child(div().text_xs().text_color(rgb(MUTED)).child("Connect Speed to a media node. Playhead, Jog, Cue and Pause belong to the media node.")).into_any_element()
    }
}
