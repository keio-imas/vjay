use crate::{
    compiler::RenderPlan,
    ports::PortResult,
    signals::{SignalEngine, ValueRef},
};
use std::collections::HashMap;
use vjay_domain::{
    graph::{Graph, NodeId},
    node::AssetId,
};

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
            .map(|source| ((source.node, source.asset), source))
            .collect();
        self.indices.clear();
        for node in graph.nodes().iter().filter(|node| {
            signals
                .capabilities(node.id)
                .is_some_and(|c| c.media.is_some())
        }) {
            let Some(asset) = node.asset else {
                continue;
            };
            let binding = signals
                .capabilities(node.id)
                .and_then(|c| c.media)
                .ok_or("Missing media binding")?;
            let speed = signals
                .parameter(node.id, binding.speed)
                .ok_or("Video speed parameter missing")?;
            let position = signals
                .parameter(node.id, binding.position)
                .ok_or("Video position parameter missing")?;
            let mut source = old.remove(&(node.id, asset)).unwrap_or(Source {
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
            });
            source.bounds = [binding.looping, binding.start, binding.end]
                .map(|p| p.and_then(|i| signals.parameter(node.id, i)));
            source.speed = speed;
            source.position = position;
            source.seek = binding.seek.and_then(|p| signals.parameter(node.id, p));
            self.indices.insert(node.id, self.sources.len());
            self.sources.push(source);
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
        for source in &mut self.sources {
            let position = source.position.read(&signals.values);
            let seek = source.seek.is_some_and(|v| v.read(&signals.values) > 0.5);
            let seek_now = seek && !source.last_seek
                || source.seek.is_some() && signals.triggered(source.node);
            source.last_seek = seek;
            let window = crate::media::PlaybackWindow {
                looping: source.bounds[0].is_none_or(|v| v.read(&signals.values) >= 0.5),
                start: source.bounds[1].map_or(0.0, |v| v.read(&signals.values)),
                end: source.bounds[2].map_or(1.0, |v| v.read(&signals.values)),
            };
            let moved = seek_now || position != source.last_position || window != source.window;
            if moved {
                source.base = 0.0;
                source.last_position = position;
            }
            source.window = window;
            let available = ready(source.node);
            if available && source.ready && !moved {
                source.base += delta * source.speed.read(&signals.values) as f64;
            }
            source.ready = available;
        }
    }
    pub fn window(&self, node: NodeId) -> crate::media::PlaybackWindow {
        self.indices
            .get(&node)
            .map_or(Default::default(), |&i| self.sources[i].window)
    }
    pub fn ready(&self, node: NodeId) -> bool {
        self.indices
            .get(&node)
            .is_none_or(|&i| self.sources[i].ready)
    }
    pub fn seek(&mut self, target: f32) {
        for source in &mut self.sources {
            source.pending_seek = Some(target);
        }
    }
    pub fn resolve_seeks(&mut self, duration: impl Fn(NodeId) -> Option<f64>) {
        for source in &mut self.sources {
            if source.ready
                && let Some(target) = source.pending_seek
                && let Some(duration) = duration(source.node)
            {
                let (start, end) = source.window.bounds(duration);
                source.base =
                    (target as f64 * (1.0 - 1e-7) - source.last_position as f64) * (end - start);
                source.pending_seek = None;
            }
        }
    }
    pub fn base(&self, node: NodeId) -> f64 {
        self.indices
            .get(&node)
            .map_or(0.0, |&i| self.sources[i].base)
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
mod tests {
    use super::*;
    use vjay_domain::{
        builtins::{ids, registry},
        node::AssetId,
    };
    #[test]
    fn pending_video_freezes_shared_timeline_and_range_edits_restart_it() {
        let factory = registry();
        let mut graph = Graph::default();
        let video = graph.add(ids::VIDEO, &factory).unwrap();
        graph.bind_asset(video, AssetId(0), &factory).unwrap();
        let mut signals = SignalEngine::default();
        signals.rebuild(&graph, &factory).unwrap();
        signals.tick(0.0);
        let mut timeline = MediaTimeline::default();
        timeline.rebuild(&graph, &signals).unwrap();
        timeline.tick_ready(0.0, &signals, |_| false);
        timeline.tick_ready(6.0, &signals, |_| false);
        assert_eq!(timeline.base(video), 0.0);
        assert!(!timeline.ready(video));
        timeline.tick_ready(6.1, &signals, |_| true);
        assert_eq!(timeline.base(video), 0.0);
        timeline.tick_ready(6.6, &signals, |_| true);
        assert!((timeline.base(video) - 0.5).abs() < 1e-6);
        graph.set_parameter(video, 3, 0.25, &factory).unwrap();
        signals.rebuild(&graph, &factory).unwrap();
        timeline.rebuild(&graph, &signals).unwrap();
        timeline.tick_ready(6.6, &signals, |_| true);
        assert_eq!(timeline.base(video), 0.0);
        assert_eq!(timeline.window(video).start, 0.25);
    }
}

#[cfg(test)]
mod seek_tests {
    use super::*;
    use vjay_domain::{
        builtins::{ids, registry},
        node::AssetId,
    };
    #[test]
    fn seek_waits_for_ready_media_and_end_does_not_wrap_to_start() {
        let factory = registry();
        let mut graph = Graph::default();
        let video = graph.add(ids::VIDEO, &factory).unwrap();
        graph.bind_asset(video, AssetId(0), &factory).unwrap();
        for (p, v) in [(1, 0.5), (3, 0.25), (4, 0.75)] {
            graph.set_parameter(video, p, v, &factory).unwrap();
        }
        let mut signals = SignalEngine::default();
        signals.rebuild(&graph, &factory).unwrap();
        signals.tick(0.0);
        let mut timeline = MediaTimeline::default();
        timeline.rebuild(&graph, &signals).unwrap();
        timeline.tick_ready(0.0, &signals, |_| false);
        timeline.seek(1.0);
        timeline.resolve_seeks(|_| Some(20.0));
        assert_eq!(timeline.base(video), 0.0);
        timeline.tick_ready(1.0, &signals, |_| true);
        timeline.resolve_seeks(|_| Some(20.0));
        let (time, _) = timeline
            .window(video)
            .resolve(timeline.base(video), 0.5, 20.0);
        assert!((time - 15.0).abs() < 0.00001);
        timeline.seek(0.0);
        timeline.resolve_seeks(|_| Some(20.0));
        assert_eq!(
            timeline
                .window(video)
                .resolve(timeline.base(video), 0.5, 20.0)
                .0,
            5.0
        );
    }
}
