use super::*;
use vjay_domain::transport::TransportAction;
#[derive(Clone, Copy, Debug)]
pub struct ClipPosition {
    pub seconds: f64,
    pub duration: f64,
    pub start: f64,
    pub end: f64,
}
impl ClipPosition {
    pub fn fraction(self) -> f32 {
        ((self.seconds - self.start) / (self.end - self.start).max(0.001)).clamp(0.0, 1.0) as f32
    }
}
impl Session {
    pub fn clip_position(&self, video: NodeId) -> Option<ClipPosition> {
        let s = self.timeline.state(video).filter(|s| s.duration > 0.0)?;
        let (start, end) = self.timeline.window(video).bounds(s.duration);
        Some(ClipPosition {
            seconds: s.seconds,
            duration: s.duration,
            start,
            end,
        })
    }
    pub fn nudge_clip(&mut self, node: NodeId, value: f32) -> PortResult<()> {
        self.timeline.nudge(node, value)
    }
    pub fn media_state(&self, node: NodeId) -> Option<vjay_domain::transport::MediaState> {
        self.timeline.state(node)
    }
    pub fn transport(&mut self, node: NodeId, action: TransportAction) -> PortResult<()> {
        if matches!(action, TransportAction::SetCue) {
            self.mark_modified();
        }
        self.timeline.command(node, action)
    }
    pub fn seek_clip(&mut self, video: NodeId, fraction: f32) -> PortResult<()> {
        self.transport(video, TransportAction::Seek(fraction))
    }
    pub fn jog_clip(&mut self, video: NodeId, seconds: f64) -> PortResult<()> {
        self.transport(video, TransportAction::Jog(seconds))
    }
    pub fn mark_cue(&mut self, video: NodeId) -> PortResult<()> {
        self.transport(video, TransportAction::SetCue)
    }
    pub fn return_to_cue(&mut self, video: NodeId) -> PortResult<()> {
        self.transport(video, TransportAction::RecallCue)
    }
    pub fn toggle_clip_playback(&mut self, video: NodeId) -> PortResult<()> {
        self.transport(video, TransportAction::Toggle)
    }
    pub fn adjust_deck_rate(&mut self, sync: NodeId, delta: f32) -> PortResult<()> {
        let caps = self.schema(sync).capabilities;
        let rate = caps
            .rate
            .map(|b| b.rate)
            .or_else(|| caps.tempo_sync.and_then(|b| b.manual).map(|b| b.rate))
            .ok_or("No rate controller binding")?;
        let value = self
            .project()
            .graph
            .node(sync)
            .map_err(|e| e.to_string())?
            .parameters[rate];
        self.set_parameter(sync, rate, (value + delta).clamp(0.0, 400.0))
    }
}
impl Session {
    pub fn setup_seek_control(&mut self, video: NodeId) -> PortResult<NodeId> {
        let schema = self.schema(video);
        let input = schema.inputs.len()
            + schema
                .capabilities
                .media
                .ok_or("Choose a media node")?
                .position;
        let transport = schema
            .capabilities
            .transport
            .ok_or("Media has no playhead output")?;
        let output = transport.outputs[0];
        let seek_input = schema
            .capabilities
            .media
            .and_then(|m| m.seek)
            .map(|p| schema.inputs.len() + p);
        if self
            .project
            .graph
            .connections()
            .any(|e| e.to == video && (e.input == input || Some(e.input) == seek_input))
        {
            return Err(
                "Seek already has a connection; disconnect it before adding a seek control".into(),
            );
        }
        let ty = self
            .catalog()
            .iter()
            .find(|d| d.capabilities.value_control.is_some())
            .ok_or("No seek control registered")?
            .type_id;
        let control = self.add_node(ty, Point { x: 30.0, y: 500.0 })?;
        let binding = self
            .schema(control)
            .capabilities
            .value_control
            .ok_or("Missing value control binding")?;
        self.connect(Connection {
            from: video,
            output,
            to: control,
            input: binding.feedback_input,
        })?;
        self.connect(Connection {
            from: control,
            output: binding.request_output,
            to: video,
            input,
        })?;
        if let (Some(output), Some(input)) = (binding.request_trigger_output, seek_input) {
            self.connect(Connection {
                from: control,
                output,
                to: video,
                input,
            })?;
        }
        if let Some(readout) = binding.readout {
            for (output, input) in transport.outputs[1..3].iter().copied().zip(readout.inputs) {
                self.connect(Connection {
                    from: video,
                    output,
                    to: control,
                    input,
                })?;
            }
        }
        self.pin_control(control, binding.parameter)?;
        if let Some(readout) = binding.readout {
            let c = self
                .project
                .controls
                .last_mut()
                .expect("pinned seek control");
            c.display = crate::value_display::ControlDisplay {
                format: crate::value_display::ValueFormat::Timecode,
                output: Some(readout.outputs[0]),
                total_output: Some(readout.outputs[1]),
            };
        }
        Ok(control)
    }
}
