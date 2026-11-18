use super::*;
use vjay_domain::port::SignalType;
impl Session {
    pub fn monitor_device(&self) -> &str {
        let name = self.audio.selected_device();
        if name.is_empty() {
            "System default"
        } else {
            name
        }
    }
    pub fn monitor_devices(&self) -> Vec<String> {
        self.audio.devices()
    }
    pub fn select_monitor_device(&mut self, name: &str) -> PortResult<()> {
        self.audio.select_device(name)?;
        self.audio_error = None;
        self.invalidate(true);
        Ok(())
    }
    pub fn monitored_video(&self, node: NodeId) -> bool {
        self.project.graph.connections().any(|c| {
            c.from == node
                && self
                    .project
                    .graph
                    .schema(c.to)
                    .is_ok_and(|s| s.capabilities.audio_cue)
        })
    }
    pub fn monitor_video(&mut self, node: NodeId, on: bool) -> PortResult<()> {
        let output = self
            .project
            .graph
            .schema(node)
            .map_err(|e| e.to_string())?
            .outputs
            .iter()
            .position(|p| p.signal == SignalType::Audio)
            .ok_or("Video has no audio output")?;
        let cue = self
            .project
            .graph
            .nodes()
            .iter()
            .find(|n| self.schema(n.id).capabilities.audio_cue)
            .map(|n| n.id);
        if !on {
            if let Some(cue) = cue {
                let input = match self.schema(cue).capabilities.audio {
                    Some(vjay_domain::capabilities::AudioRecipe::Sink { input, .. }) => input,
                    _ => return Err("Invalid cue sink".into()),
                };
                if self
                    .project
                    .graph
                    .connections()
                    .any(|c| c.to == cue && c.from == node)
                {
                    self.disconnect(cue, input);
                }
            }
            return Ok(());
        }
        let cue = if let Some(cue) = cue {
            cue
        } else {
            let ty = self
                .catalog()
                .iter()
                .find(|d| d.capabilities.audio_cue)
                .ok_or("Cue output unavailable")?
                .type_id;
            self.add_node(ty, Point { x: 30.0, y: 500.0 })?
        };
        let input = match self.schema(cue).capabilities.audio {
            Some(vjay_domain::capabilities::AudioRecipe::Sink { input, .. }) => input,
            _ => return Err("Invalid cue sink".into()),
        };
        self.connect(Connection {
            from: node,
            output,
            to: cue,
            input,
        })
    }
    pub fn video_sync(&self, video: NodeId) -> Option<NodeId> {
        let schema = self.project.graph.schema(video).ok()?;
        let speed = schema.inputs.len() + schema.capabilities.media?.speed;
        self.project
            .graph
            .connections()
            .find(|c| {
                c.to == video
                    && c.input == speed
                    && self
                        .project
                        .graph
                        .schema(c.from)
                        .is_ok_and(|s| s.capabilities.tempo_sync.is_some())
            })
            .map(|c| c.from)
    }
    pub fn setup_video_sync(&mut self, video: NodeId) -> PortResult<NodeId> {
        if let Some(node) = self.video_sync(video) {
            return Ok(node);
        }
        let schema = self
            .project
            .graph
            .schema(video)
            .map_err(|e| e.to_string())?;
        let media = schema.capabilities.media.ok_or("Choose a video node")?;
        let offset = schema.inputs.len();
        let ports = [
            offset + media.speed,
            offset + media.position,
            offset + media.seek.ok_or("Video has no seek input")?,
        ];
        if self
            .project
            .graph
            .connections()
            .any(|c| c.to == video && ports.contains(&c.input))
        {
            return Err("Speed, Position, or Seek already has a connection; disconnect it before setting up BPM Sync".into());
        }
        let ty = self
            .catalog()
            .iter()
            .find(|d| d.capabilities.tempo_sync.is_some())
            .ok_or("BPM Sync unavailable")?
            .type_id;
        let node = self.add_node(ty, Point { x: 300.0, y: 500.0 })?;
        let binding = self
            .schema(node)
            .capabilities
            .tempo_sync
            .ok_or("Invalid BPM Sync binding")?;
        for (output, input) in binding.outputs.into_iter().zip(ports) {
            self.connect(Connection {
                from: node,
                output,
                to: video,
                input,
            })?;
        }
        self.sync_clip_duration(video, node)?;
        Ok(node)
    }
    pub fn sync_clip_duration(&mut self, video: NodeId, sync: NodeId) -> PortResult<()> {
        if let Some(duration) = self
            .renderer
            .media_duration(video)
            .or_else(|| self.audio.media_duration(video))
        {
            let n = self.project.graph.node(video).map_err(|e| e.to_string())?;
            let m = self.schema(video).capabilities.media.ok_or("Not a video")?;
            let range =
                m.end.map_or(1.0, |p| n.parameters[p]) - m.start.map_or(0.0, |p| n.parameters[p]);
            self.set_parameter(
                sync,
                self.schema(sync)
                    .capabilities
                    .tempo_sync
                    .ok_or("Invalid BPM Sync binding")?
                    .duration,
                (duration as f32 * range).clamp(0.01, 7200.0),
            )?;
        }
        Ok(())
    }
    pub fn tap_dj_bpm(&mut self, sync: NodeId) {
        let Some(binding) = self
            .project
            .graph
            .schema(sync)
            .ok()
            .and_then(|s| s.capabilities.tempo_sync)
        else {
            return;
        };
        if let Some(bpm) = self
            .sync_taps
            .entry(sync)
            .or_default()
            .tap(self.clock.seconds())
        {
            let _ = self.set_parameter(sync, binding.dj_bpm, bpm as f32);
        }
    }
    pub fn jog_sync_cue(&mut self, sync: NodeId, seconds: f32) -> PortResult<()> {
        let schema = self.project.graph.schema(sync).map_err(|e| e.to_string())?;
        let b = schema
            .capabilities
            .tempo_sync
            .ok_or("Invalid BPM Sync binding")?;
        let values = &self
            .project
            .graph
            .node(sync)
            .map_err(|e| e.to_string())?
            .parameters;
        let cue = (values[b.cue] + seconds / values[b.duration]).clamp(0.0, 1.0);
        self.set_parameter(sync, b.cue, cue)
    }
    pub fn align_video(&mut self, sync: NodeId) -> PortResult<()> {
        let parameter = self
            .project
            .graph
            .schema(sync)
            .map_err(|e| e.to_string())?
            .capabilities
            .tempo_sync
            .ok_or("Invalid BPM Sync binding")?
            .align;
        self.set_parameter(sync, parameter, 1.0)?;
        self.set_parameter(sync, parameter, 0.0)
    }
}
