use crate::{
    compiler::RenderPlan,
    ports::PortResult,
    signals::{SignalEngine, ValueRef},
};
use std::collections::HashMap;
use vjay_domain::{
    graph::{Graph, NodeId},
    node::AssetId,
    transport::{MediaState, TransportAction},
};
mod commands;
struct Source {
    node: NodeId,
    asset: AssetId,
    speed: ValueRef,
    position: ValueRef,
    base: f64,
    last_position: f32,
    window: crate::media::PlaybackWindow,
    bounds: [Option<ValueRef>; 3],
    ready: bool,
    pending_seek: Option<f32>,
    seek: Option<ValueRef>,
    last_seek: bool,
    seek_revision: u64,
    commands: [Option<ValueRef>; 8],
    last_commands: [f32; 8],
    actions: Vec<TransportAction>,
    paused: bool,
    cue: f32,
    rate: f32,
    duration: f64,
    user_nudge: f32,
}
#[derive(Default)]
pub struct MediaTimeline {
    sources: Vec<Source>,
    indices: HashMap<NodeId, usize>,
    last_time: Option<f64>,
}
impl MediaTimeline {
    pub fn rebuild(&mut self, graph: &Graph, signals: &SignalEngine) -> PortResult<()> {
        let mut old: HashMap<_, _> = std::mem::take(&mut self.sources)
            .into_iter()
            .map(|s| ((s.node, s.asset), s))
            .collect();
        self.indices.clear();
        for node in graph.nodes().iter().filter(|n| {
            signals
                .capabilities(n.id)
                .is_some_and(|c| c.media.is_some())
        }) {
            let Some(asset) = node.asset else {
                continue;
            };
            let caps = signals
                .capabilities(node.id)
                .ok_or("Missing media capabilities")?;
            let b = caps.media.ok_or("Missing media binding")?;
            let speed = signals.parameter(node.id, b.speed).ok_or("Missing speed")?;
            let position = signals
                .parameter(node.id, b.position)
                .ok_or("Missing position")?;
            let mut s = old.remove(&(node.id, asset)).unwrap_or(Source {
                node: node.id,
                asset,
                speed,
                position,
                base: 0.0,
                last_position: position.value,
                window: Default::default(),
                bounds: [None; 3],
                ready: true,
                pending_seek: None,
                seek: None,
                last_seek: false,
                seek_revision: 0,
                commands: [None; 8],
                last_commands: [0.0; 8],
                actions: Vec::with_capacity(16),
                paused: false,
                cue: node
                    .settings
                    .transport
                    .filter(|b| b.asset == Some(asset))
                    .map_or(0.0, |b| b.cue),
                rate: 1.0,
                duration: 0.0,
                user_nudge: 0.0,
            });
            s.speed = speed;
            s.position = position;
            s.bounds =
                [b.looping, b.start, b.end].map(|p| p.and_then(|i| signals.parameter(node.id, i)));
            s.seek = b.seek.and_then(|p| signals.parameter(node.id, p));
            s.commands = caps
                .transport
                .map_or([None; 8], |c| c.inputs.map(|i| signals.input(node.id, i)));
            self.indices.insert(node.id, self.sources.len());
            self.sources.push(s);
        }
        Ok(())
    }
    pub fn tick(&mut self, seconds: f64, signals: &SignalEngine) {
        self.tick_ready(seconds, signals, |_| true);
    }
    pub fn tick_ready(
        &mut self,
        seconds: f64,
        signals: &SignalEngine,
        ready: impl Fn(NodeId) -> bool,
    ) {
        let delta = self.last_time.map_or(0.0, |last| (seconds - last).max(0.0));
        self.last_time = Some(seconds);
        for s in &mut self.sources {
            let position = s.position.read(&signals.values);
            let seek = s.seek.is_some_and(|v| v.read(&signals.values) > 0.5);
            let seek_now = seek && !s.last_seek || s.seek.is_some() && signals.triggered(s.node);
            s.last_seek = seek;
            let window = crate::media::PlaybackWindow {
                looping: s.bounds[0].is_none_or(|v| v.read(&signals.values) >= 0.5),
                start: s.bounds[1].map_or(0.0, |v| v.read(&signals.values)),
                end: s.bounds[2].map_or(1.0, |v| v.read(&signals.values)),
            };
            let moved = seek_now || position != s.last_position || window != s.window;
            if moved {
                s.base = 0.0;
                s.last_position = position;
                s.seek_revision = s.seek_revision.wrapping_add(1);
            }
            s.window = window;
            s.read_commands(signals);
            s.rate = s.speed.read(&signals.values);
            let available = ready(s.node);
            if available && s.ready && !moved && s.actions.is_empty() && s.pending_seek.is_none() {
                s.base += delta * f64::from(s.effective_speed());
            }
            s.ready = available;
        }
    }
    pub fn ready(&self, node: NodeId) -> bool {
        self.indices
            .get(&node)
            .is_none_or(|&i| self.sources[i].ready)
    }
    pub fn window(&self, node: NodeId) -> crate::media::PlaybackWindow {
        self.indices
            .get(&node)
            .map_or(Default::default(), |&i| self.sources[i].window)
    }
    pub fn base(&self, node: NodeId) -> f64 {
        self.indices
            .get(&node)
            .map_or(0.0, |&i| self.sources[i].base)
    }
    pub fn speed(&self, node: NodeId) -> Option<f32> {
        self.indices
            .get(&node)
            .map(|&i| self.sources[i].effective_speed())
    }
    pub fn seek_revision(&self, node: NodeId) -> u64 {
        self.indices
            .get(&node)
            .map_or(0, |&i| self.sources[i].seek_revision)
    }
    pub fn state(&self, node: NodeId) -> Option<MediaState> {
        self.indices.get(&node).map(|&i| self.sources[i].state())
    }
    pub fn bookmarks(
        &self,
    ) -> impl Iterator<Item = (NodeId, vjay_domain::transport::Bookmark)> + '_ {
        self.sources.iter().map(|s| {
            (
                s.node,
                vjay_domain::transport::Bookmark {
                    asset: Some(s.asset),
                    cue: s.cue,
                },
            )
        })
    }
    pub fn publish(&self, signals: &mut SignalEngine) {
        for s in &self.sources {
            signals.set_media_feedback(s.node, s.state());
        }
    }
    pub fn command(&mut self, node: NodeId, action: TransportAction) -> PortResult<()> {
        if !action.valid() {
            return Err("Invalid transport action".into());
        }
        let &index = self
            .indices
            .get(&node)
            .ok_or("Load the clip before using transport")?;
        let s = &mut self.sources[index];
        if matches!(action, TransportAction::SetCue) && s.ready && s.duration > 0.0 {
            s.cue = s.state().position;
            return Ok(());
        }
        if s.actions.len() >= 64 {
            return Err("Too many pending transport actions".into());
        }
        s.actions.push(action);
        Ok(())
    }
    pub fn nudge(&mut self, node: NodeId, value: f32) -> PortResult<()> {
        if !value.is_finite() {
            return Err("Invalid nudge".into());
        }
        let &i = self.indices.get(&node).ok_or("Load a clip first")?;
        self.sources[i].user_nudge = value.clamp(-1.0, 1.0);
        Ok(())
    }
    pub fn seek(&mut self, target: f32) {
        for s in &mut self.sources {
            s.pending_seek = Some(target);
        }
    }
    pub fn resolve_seeks(&mut self, duration: impl Fn(NodeId) -> Option<f64>) {
        for s in &mut self.sources {
            if let Some(d) = duration(s.node).filter(|v| v.is_finite() && *v > 0.0) {
                s.duration = d;
            }
            if s.ready && s.duration > 0.0 {
                if let Some(p) = s.pending_seek.take() {
                    s.seek_to(p);
                }
                s.apply_actions();
            }
        }
    }
    pub fn apply(&self, plan: &mut RenderPlan) {
        for step in &mut plan.steps {
            if self.indices.contains_key(&step.node) {
                step.media_time = Some(self.base(step.node));
            }
        }
    }
}
#[cfg(test)]
mod tests;
