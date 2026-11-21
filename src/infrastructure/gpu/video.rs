use std::{collections::HashMap, path::PathBuf};
use vjay_application::{
    compiler::RenderPlan,
    media::{VideoDecoder, VideoDecoderFactory},
    ports::*,
};
use vjay_domain::node::AssetId;

struct Input {
    step: usize,
    node: vjay_domain::graph::NodeId,
    asset: AssetId,
    path: PathBuf,
    extent: Extent,
    decoder: Box<dyn VideoDecoder>,
    texture: wgpu::Texture,
    last_time: Option<f64>,
    playhead: f64,
    position: f32,
}
pub(super) struct VideoInputs {
    streams: Vec<Input>,
    pub views: Vec<Option<wgpu::TextureView>>,
    pub errors: HashMap<vjay_domain::graph::NodeId, String>,
}

impl VideoInputs {
    pub fn prepare(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        plan: &RenderPlan,
        extent: Extent,
        assets: &HashMap<AssetId, PathBuf>,
        factory: &dyn VideoDecoderFactory,
        mut previous: Option<Self>,
    ) -> PortResult<Self> {
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
                result.views[step] = Some(input.texture.create_view(&Default::default()));
                result.streams.push(input);
                continue;
            }
            let mut decoder = match factory.open(asset, extent) {
                Ok(decoder) => decoder,
                Err(error) => {
                    if node.program_required {
                        return Err(error);
                    }
                    result.errors.insert(node.node, error);
                    continue;
                }
            };
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("decoded video input"),
                size: wgpu::Extent3d {
                    width: extent.width,
                    height: extent.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            match decoder.frame_at(0.0, vjay_application::media::FrameDelivery::Latest) {
                Ok(Some(frame)) => {
                    upload(queue, &texture, extent, &frame.rgba);
                    decoder.recycle(frame);
                }
                Ok(None) => (),
                Err(error) => {
                    if node.program_required {
                        return Err(error);
                    }
                    result.errors.insert(node.node, error);
                    continue;
                }
            }
            result.views[step] = Some(texture.create_view(&Default::default()));
            result.streams.push(Input {
                step,
                node: node.node,
                asset: node.asset.ok_or("Video asset is missing")?,
                path: asset.clone(),
                extent,
                decoder,
                texture,
                last_time: None,
                playhead: 0.0,
                position: -1.0,
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
            let (seconds, _) = playback
                .window
                .resolve(seconds, position, input.decoder.duration());
            let decoded = match input.decoder.frame_at(seconds, frame.delivery) {
                Ok(frame) => frame,
                Err(error) => {
                    if plan.steps[input.step].program_required {
                        return Err(error);
                    }
                    self.errors.insert(input.node, error);
                    continue;
                }
            };
            if let Some(decoded) = decoded {
                upload(queue, &input.texture, frame.extent, &decoded.rgba);
                input.decoder.recycle(decoded);
            }
        }
        Ok(())
    }
}

fn upload(queue: &wgpu::Queue, texture: &wgpu::Texture, extent: Extent, rgba: &[u8]) {
    queue.write_texture(
        texture.as_image_copy(),
        rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(extent.width * 4),
            rows_per_image: Some(extent.height),
        },
        wgpu::Extent3d {
            width: extent.width,
            height: extent.height,
            depth_or_array_layers: 1,
        },
    );
}
