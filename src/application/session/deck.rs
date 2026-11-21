use super::*;

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
        let duration = self
            .renderer
            .media_duration(video)
            .or_else(|| self.audio.media_duration(video))?;
        let media = self.project.graph.schema(video).ok()?.capabilities.media?;
        let node = self.project.graph.node(video).ok()?;
        let position = self
            .signals
            .value(video, media.position)
            .unwrap_or(node.parameters[media.position]);
        let window = self.timeline.window(video);
        let (start, end) = window.bounds(duration);
        let seconds = if self.monitored_video(video) {
            self.audio.media_position(video)
        } else {
            None
        }
        .unwrap_or_else(|| {
            window
                .resolve(self.timeline.base(video), position, duration)
                .0
        });
        Some(ClipPosition {
            seconds,
            duration,
            start,
            end,
        })
    }
    pub fn seek_clip(&mut self, video: NodeId, fraction: f32) -> PortResult<()> {
        if !fraction.is_finite() {
            return Err("Invalid seek position".into());
        }
        if let Some(sync) = self.video_sync(video) {
            let b = self
                .schema(sync)
                .capabilities
                .tempo_sync
                .ok_or("No deck binding")?;
            self.set_parameter(sync, b.cue, fraction.clamp(0.0, 1.0 - 1e-6))?;
            self.align_video(sync)
        } else {
            let b = self
                .schema(video)
                .capabilities
                .media
                .ok_or("Choose a video node")?;
            self.set_parameter(video, b.position, fraction.clamp(0.0, 1.0 - 1e-6))?;
            if let Some(seek) = b.seek {
                self.set_parameter(video, seek, 1.0)?;
                self.set_parameter(video, seek, 0.0)?;
            }
            Ok(())
        }
    }
    pub fn jog_clip(&mut self, video: NodeId, seconds: f64) -> PortResult<()> {
        let p = self
            .clip_position(video)
            .ok_or("Load the clip before jogging")?;
        self.seek_clip(
            video,
            (p.fraction() as f64 + seconds / (p.end - p.start).max(0.001)).clamp(0.0, 1.0 - 1e-6)
                as f32,
        )
    }
    pub fn mark_cue(&mut self, video: NodeId) -> PortResult<()> {
        let position = self
            .clip_position(video)
            .ok_or("Load a clip first")?
            .fraction();
        let sync = self.video_sync(video).ok_or("Connect Deck Sync first")?;
        let b = self
            .schema(sync)
            .capabilities
            .tempo_sync
            .and_then(|b| b.manual)
            .ok_or("No manual deck binding")?;
        self.set_parameter(sync, b.marker, position)
    }
    pub fn return_to_cue(&mut self, video: NodeId) -> PortResult<()> {
        let sync = self.video_sync(video).ok_or("Connect Deck Sync first")?;
        let b = self
            .schema(sync)
            .capabilities
            .tempo_sync
            .and_then(|b| b.manual)
            .ok_or("No manual deck binding")?;
        self.seek_clip(
            video,
            self.project
                .graph
                .node(sync)
                .map_err(|e| e.to_string())?
                .parameters[b.marker],
        )
    }
    pub fn toggle_clip_playback(&mut self, video: NodeId) -> PortResult<()> {
        let sync = self.video_sync(video).ok_or("Connect Deck Sync first")?;
        let b = self
            .schema(sync)
            .capabilities
            .tempo_sync
            .and_then(|b| b.manual)
            .ok_or("No manual deck binding")?;
        if self
            .project
            .graph
            .node(sync)
            .map_err(|e| e.to_string())?
            .parameters[b.mode]
            >= 0.5
        {
            return Err("Use By ear mode for independent clip transport".into());
        }
        let rate = self
            .project
            .graph
            .node(sync)
            .map_err(|e| e.to_string())?
            .parameters[b.rate];
        if rate > 0.0 {
            self.deck_rates.retain(|(id, _)| *id != sync);
            self.deck_rates.push((sync, rate));
            self.set_parameter(sync, b.rate, 0.0)
        } else {
            let previous = self
                .deck_rates
                .iter()
                .find(|(id, _)| *id == sync)
                .map_or(100.0, |(_, r)| *r);
            self.set_parameter(sync, b.rate, previous)
        }
    }
    pub fn adjust_deck_rate(&mut self, sync: NodeId, delta: f32) -> PortResult<()> {
        let b = self
            .schema(sync)
            .capabilities
            .tempo_sync
            .and_then(|b| b.manual)
            .ok_or("No manual deck binding")?;
        let value = self
            .project
            .graph
            .node(sync)
            .map_err(|e| e.to_string())?
            .parameters[b.rate];
        self.set_parameter(sync, b.rate, (value + delta).clamp(0.0, 400.0))
    }
}
