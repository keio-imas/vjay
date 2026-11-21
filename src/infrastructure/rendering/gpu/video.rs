pub(super) mod source;
use source::{GpuVideoDecoder, GpuVideoRuntime, UploadedVideo};
use std::{collections::HashMap, path::PathBuf};
use vjay_application::{compiler::RenderPlan, media::VideoDecoderFactory, ports::*};
use vjay_domain::node::AssetId;

struct Input {
    step: usize,
    node: vjay_domain::graph::NodeId,
    asset: AssetId,
    path: PathBuf,
    extent: Extent,
    decoder: Box<dyn GpuVideoDecoder>,
    last_time: Option<f64>,
    playhead: f64,
    position: f32,
    requested: f64,
}
pub(super) struct VideoInputs {
    streams: Vec<Input>,
    pub views: Vec<Option<wgpu::Texture>>,
    pub errors: HashMap<vjay_domain::graph::NodeId, String>,
}

impl VideoInputs {
    pub fn metrics(&self) -> vjay_application::media::MediaMetrics {
        let mut total = vjay_application::media::MediaMetrics::default();
        for stream in &self.streams {
            total += stream.decoder.metrics();
        }
        total
    }
    pub fn prepare(
        context: (&wgpu::Device, &wgpu::Queue),
        plan: &RenderPlan,
        extent: Extent,
        assets: &HashMap<AssetId, PathBuf>,
        factory: &dyn VideoDecoderFactory,
        mut previous: Option<Self>,
        mut native: Option<&mut (dyn GpuVideoRuntime + 'static)>,
    ) -> PortResult<Self> {
        let (device, queue) = context;
        let mut result = Self {
            streams: Vec::new(),
            errors: HashMap::new(),
            views: vec![None; plan.steps.len()],
        };
        let mut order = plan
            .steps
            .iter()
            .enumerate()
            // An unselected media source samples the shared empty texture. Its
            // Ready output stays zero, allowing a patch to select a placeholder.
            .filter(|(_, n)| n.operation.media.is_some() && n.asset.is_some())
            .collect::<Vec<_>>();
        order.sort_by_key(|(_, n)| !n.program_required);
        for (step, node) in order {
            if result.streams.len() >= 8 {
                let error = "At most eight simultaneous video sources are supported".to_string();
                if node.program_required {
                    return Err(error);
                }
                result.errors.insert(node.node, error);
                continue;
            }
            let asset = node
                .asset
                .and_then(|id| assets.get(&id))
                .ok_or("Video asset is missing")?;
            if let Some(old) = previous.as_mut()
                && let Some(index) = old.streams.iter().position(|input| {
                    input.node == node.node
                        && Some(input.asset) == node.asset
                        && input.path == *asset
                        && input.extent == extent
                        && !input.decoder.failed()
                })
            {
                let mut input = old.streams.swap_remove(index);
                input.step = step;
                result.views[step] = Some(input.decoder.texture().clone());
                result.streams.push(input);
                continue;
            }
            let opened = if let Some(native) = native.as_mut() {
                native.open(asset, extent)
            } else {
                factory.open(asset, extent).map(|d| {
                    Box::new(UploadedVideo::new(device, d, extent)) as Box<dyn GpuVideoDecoder>
                })
            };
            let mut decoder = match opened {
                Ok(decoder) => decoder,
                Err(error) => {
                    if node.program_required {
                        return Err(error);
                    }
                    result.errors.insert(node.node, error);
                    continue;
                }
            };
            match decoder.frame_at(0.0, vjay_application::media::FrameDelivery::Latest, queue) {
                Ok(_) => (),
                Err(error) => {
                    if node.program_required {
                        return Err(error);
                    }
                    result.errors.insert(node.node, error);
                    continue;
                }
            }
            result.views[step] = Some(decoder.texture().clone());
            result.streams.push(Input {
                step,
                node: node.node,
                asset: node.asset.ok_or("Video asset is missing")?,
                path: asset.clone(),
                extent,
                decoder,
                last_time: None,
                playhead: 0.0,
                position: -1.0,
                requested: 0.0,
            });
        }
        Ok(result)
    }
    pub fn duration(&self, node: vjay_domain::graph::NodeId) -> Option<f64> {
        self.streams
            .iter()
            .find(|s| s.node == node)
            .map(|s| s.decoder.duration())
    }
    pub fn ready(&self, node: vjay_domain::graph::NodeId) -> bool {
        if self.errors.contains_key(&node) {
            return true;
        }
        self.streams
            .iter()
            .find(|s| s.node == node)
            .is_none_or(|s| s.decoder.ready())
    }
    pub fn status(&self) -> Option<&str> {
        self.streams
            .iter()
            .find(|s| !s.decoder.ready())
            .and_then(|s| s.decoder.status())
            .or_else(|| self.streams.iter().find_map(|s| s.decoder.status()))
    }
    pub fn update(
        &mut self,
        queue: &wgpu::Queue,
        plan: &RenderPlan,
        frame: FrameContext,
    ) -> PortResult<()> {
        for input in &mut self.streams {
            if let Some(error) = self.errors.get(&input.node) {
                if plan.steps[input.step].program_required {
                    return Err(error.clone());
                }
                continue;
            }

            let playback = plan.steps[input.step]
                .playback
                .as_ref()
                .ok_or("Media plan changed without prepare")?;
            let (speed, position) = (playback.speed, playback.position);
            if input.position != position {
                input.playhead = 0.0;
                input.position = position;
            }
            if let Some(last) = input.last_time {
                input.playhead += (frame.timeline_seconds - last).max(0.0) * speed as f64;
            }
            input.last_time = Some(frame.timeline_seconds);
            let seconds = plan.steps[input.step].media_time.unwrap_or(input.playhead);
            let duration = input.decoder.duration();
            let seconds = if duration > 0.0 {
                playback.window.resolve(seconds, position, duration).0
            } else {
                0.0
            };
            input.requested = seconds;
            if let Err(error) = input.decoder.request_frame(seconds) {
                self.errors.insert(input.node, error);
            }
        }
        // Every decoder has its target before the first exact-delivery wait.
        for input in &mut self.streams {
            if let Some(error) = self.errors.get(&input.node) {
                if plan.steps[input.step].program_required {
                    return Err(error.clone());
                }
                continue;
            }
            match input
                .decoder
                .frame_at(input.requested, frame.delivery, queue)
            {
                Ok(_) => (),
                Err(error) => {
                    if plan.steps[input.step].program_required {
                        return Err(error);
                    }
                    self.errors.insert(input.node, error);
                    continue;
                }
            }
        }
        Ok(())
    }
}
