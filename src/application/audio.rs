use crate::bindings::PortBindings;
use crate::{
    media::MediaAsset,
    media_timeline::MediaTimeline,
    ports::PortResult,
    signals::{SignalEngine, ValueRef},
};
use vjay_domain::{
    capabilities::AudioRecipe,
    graph::{Graph, NodeId},
    node::AssetId,
};

pub struct AudioRoute {
    pub node: NodeId,
    pub asset: AssetId,
    pub gain: f32,
    pub speed: f32,
    pub position: f32,
    pub seconds: f64,
    pub window: crate::media::PlaybackWindow,
    pub ready: bool,
    pub seek_revision: u64,
    gains: Vec<ValueRef>,
    rate: ValueRef,
    seek: ValueRef,
}
#[derive(Default)]
pub struct AudioPlan {
    pub routes: Vec<AudioRoute>,
}
impl AudioPlan {
    pub fn compile(graph: &Graph, signals: &SignalEngine) -> PortResult<Self> {
        let inputs = PortBindings::new(graph)?;
        let mut result = Self::default();
        for (output_index, output) in graph.nodes().iter().enumerate() {
            if !signals
                .capabilities(output.id)
                .is_some_and(|c| matches!(c.audio, Some(AudioRecipe::Sink { .. })))
            {
                continue;
            }
            let mut stack = vec![(output_index, Vec::new())];
            while let Some((node_index, gains)) = stack.pop() {
                let node = &graph.nodes()[node_index];
                let id = node.id;
                let capabilities = signals
                    .capabilities(id)
                    .ok_or("Missing audio capabilities")?;
                match capabilities
                    .audio
                    .ok_or("Node has no audio routing recipe")?
                {
                    AudioRecipe::Source => {
                        let Some(asset) = node.asset else {
                            continue;
                        };
                        if result.routes.len() >= 64 {
                            return Err("Audio route limit exceeded".into());
                        }
                        let media = capabilities
                            .media
                            .ok_or("Audio source needs a media binding")?;
                        result.routes.push(AudioRoute {
                            node: id,
                            asset,
                            gain: 0.0,
                            speed: 1.0,
                            position: 0.0,
                            seconds: 0.0,
                            window: Default::default(),
                            ready: true,
                            seek_revision: 0,
                            gains,
                            rate: signals
                                .parameter(id, media.speed)
                                .ok_or("Media speed missing")?,
                            seek: signals
                                .parameter(id, media.position)
                                .ok_or("Media position missing")?,
                        });
                    }
                    AudioRecipe::Sink { input, gain } => {
                        if let Some(edge) = inputs.source(node_index, input) {
                            let mut path = gains;
                            path.push(signals.parameter(id, gain).ok_or("Audio volume missing")?);
                            stack.push((edge.node, path));
                        }
                    }
                    AudioRecipe::Mix(routes) => {
                        for route in routes {
                            if let Some(edge) = inputs.source(node_index, route.input) {
                                let mut path = gains.clone();
                                if let Some(index) = route.gain {
                                    path.push(
                                        signals.parameter(id, index).ok_or("Audio gain missing")?,
                                    );
                                }
                                stack.push((edge.node, path));
                            }
                        }
                    }
                }
            }
        }
        Ok(result)
    }
    pub fn update(&mut self, signals: &SignalEngine, timeline: &MediaTimeline) {
        for route in &mut self.routes {
            route.gain = route
                .gains
                .iter()
                .map(|gain| gain.read(&signals.values))
                .product();
            route.speed = timeline
                .speed(route.node)
                .unwrap_or_else(|| route.rate.read(&signals.values));
            route.position = route.seek.read(&signals.values);
            route.seconds = timeline.base(route.node);
            route.window = timeline.window(route.node);
            route.ready = timeline.ready(route.node);
            route.seek_revision = timeline.seek_revision(route.node);
        }
    }
}
pub trait AudioBackend {
    fn media_position(&self, _node: NodeId) -> Option<f64> {
        None
    }
    fn configure(&mut self, settings: &crate::preferences::AudioSettings) -> PortResult<()> {
        if *settings == crate::preferences::AudioSettings::default() {
            Ok(())
        } else {
            Err("Audio output configuration is unavailable".into())
        }
    }
    fn selected_device(&self) -> &str {
        ""
    }
    fn devices(&self) -> Vec<String> {
        Vec::new()
    }
    fn select_device(&mut self, _: &str) -> PortResult<()> {
        Err("Audio device selection unavailable".into())
    }
    fn media_duration(&self, _node: NodeId) -> Option<f64> {
        None
    }
    fn prepare(&mut self, plan: &AudioPlan, assets: &[MediaAsset]) -> PortResult<()>;
    fn update(&mut self, plan: &AudioPlan, paused: bool) -> PortResult<()>;
}
pub trait AudioFactory {
    fn create(&self) -> Box<dyn AudioBackend>;
}
pub struct DisabledAudio;
impl AudioBackend for DisabledAudio {
    fn configure(&mut self, _: &crate::preferences::AudioSettings) -> PortResult<()> {
        Ok(())
    }
    fn prepare(&mut self, plan: &AudioPlan, _: &[MediaAsset]) -> PortResult<()> {
        if plan.routes.is_empty() {
            Ok(())
        } else {
            Err("Audio output is unavailable in this runtime".into())
        }
    }
    fn update(&mut self, _: &AudioPlan, _: bool) -> PortResult<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
