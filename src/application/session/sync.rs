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
        let mut prefs = self.preferences.clone();
        prefs.audio.device = name.into();
        self.apply_preferences(prefs)?;
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
    /// Find a rate source for setup idempotence; transport never edits it.
    pub fn video_sync(&self, video: NodeId) -> Option<NodeId> {
        let schema = self.project().graph.schema(video).ok()?;
        let input = schema.inputs.len() + schema.capabilities.media?.speed;
        self.project().graph.connections().find_map(|e| {
            if e.to != video || e.input != input {
                return None;
            }
            let caps = self.project().graph.schema(e.from).ok()?.capabilities;
            let output = caps
                .rate
                .map(|b| b.speed_output)
                .or_else(|| caps.tempo_sync.map(|b| b.outputs[0]))?;
            (e.output == output).then_some(e.from)
        })
    }
    pub fn setup_video_sync(&mut self, video: NodeId) -> PortResult<NodeId> {
        if let Some(id) = self.video_sync(video) {
            return Ok(id);
        }
        let s = self.schema(video);
        let input = s.inputs.len() + s.capabilities.media.ok_or("Choose a video node")?.speed;
        if self
            .project()
            .graph
            .connections()
            .any(|e| e.to == video && e.input == input)
        {
            return Err(
                "Speed already has a connection; disconnect it before adding a rate controller"
                    .into(),
            );
        }
        let ty = self
            .catalog()
            .iter()
            .find(|d| d.capabilities.rate.is_some())
            .ok_or("No rate controller registered")?
            .type_id;
        let controller = self.add_node(ty, Point { x: 300.0, y: 500.0 })?;
        let output = self
            .schema(controller)
            .capabilities
            .rate
            .ok_or("Missing rate output")?
            .speed_output;
        self.connect(Connection {
            from: controller,
            output,
            to: video,
            input,
        })?;
        Ok(controller)
    }
    pub fn tap_dj_bpm(&mut self, sync: NodeId) {
        let caps = self.schema(sync).capabilities;
        let Some(parameter) = caps
            .rate
            .map(|b| b.dj_bpm)
            .or_else(|| caps.tempo_sync.map(|b| b.dj_bpm))
        else {
            return;
        };
        if let Some(bpm) = self
            .sync_taps
            .entry(sync)
            .or_default()
            .tap(self.clock.seconds())
        {
            let _ = self.set_parameter(sync, parameter, bpm as f32);
        }
    }
}
