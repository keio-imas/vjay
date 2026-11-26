mod preparation;
use super::presentation::GpuPresentation;
use super::resources::{Resources, create_layout};
use super::video::VideoInputs;
use std::{cell::RefCell, collections::HashMap, path::PathBuf, rc::Rc, sync::Arc};
use vjay_application::media::{MediaAsset, VideoDecoderFactory};
use vjay_application::{compiler::RenderPlan, ports::*};
use vjay_domain::node::AssetId;

pub(super) struct WgpuRenderer {
    pub(super) device: wgpu::Device,
    pub(super) queue: wgpu::Queue,
    layout: wgpu::BindGroupLayout,
    streams: super::streams::Streams,
    output_slots: Vec<(vjay_domain::graph::NodeId, usize)>,
    pub(super) resources: Option<Resources>,
    presentation: Option<Rc<RefCell<dyn GpuPresentation>>>,
    description: String,
    media_factory: Arc<dyn VideoDecoderFactory>,
    assets: HashMap<AssetId, PathBuf>,
    video: Option<VideoInputs>,
    native_video: Option<Box<dyn super::GpuVideoRuntime>>,
    images: super::images::ImageInputs,
    submitted_frames: u64,
    custom: super::programs::ShaderCache,
    pipelines: Vec<wgpu::RenderPipeline>,
    programs: Arc<super::programs::ShaderPrograms>,
}

impl WgpuRenderer {
    pub(super) fn new(
        device: wgpu::Device,
        queue: wgpu::Queue,
        presentation: Option<Rc<RefCell<dyn GpuPresentation>>>,
        description: String,
        media_factory: Arc<dyn VideoDecoderFactory>,
        programs: Arc<super::programs::ShaderPrograms>,
        image_loaders: super::RendererAdapters,
    ) -> PortResult<Self> {
        let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let layout = create_layout(&device);
        if let Some(error) = pollster::block_on(validation.pop()) {
            return Err(error.to_string());
        }
        let native_video = image_loaders
            .native_video
            .as_ref()
            .map(|f| f.create(&device, &queue))
            .transpose()?;
        Ok(Self {
            native_video,
            streams: super::streams::Streams::new(image_loaders.video_io.clone()),
            output_slots: Vec::new(),
            device,
            queue,
            programs,
            layout,
            resources: None,
            presentation,
            description,
            media_factory,
            assets: HashMap::new(),
            video: None,
            images: super::images::ImageInputs::new(
                image_loaders.files.clone(),
                image_loaders.generated.clone(),
            ),
            submitted_frames: 0,
            custom: Default::default(),
            pipelines: Vec::new(),
        })
    }
}

impl RenderBackend for WgpuRenderer {
    fn render_metrics(&self) -> vjay_application::ports::telemetry::RenderMetrics {
        let Some(r) = &self.resources else {
            return Default::default();
        };
        let forwarded = r.forwarded.iter().filter(|&&v| v).count();
        vjay_application::ports::telemetry::RenderMetrics {
            submitted_frames: self.submitted_frames,
            image_passes: r.forwarded.len() - forwarded,
            forwarded_sources: forwarded,
            pooled_texture_bytes: (r.textures.len() - forwarded) as u64
                * r.extent.width as u64
                * r.extent.height as u64
                * 4,
        }
    }

    fn wait_idle(&self) -> PortResult<()> {
        let submission = self.queue.submit([]);
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(std::time::Duration::from_secs(30)),
            })
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    fn media_metrics(&self) -> vjay_application::media::MediaMetrics {
        self.video
            .as_ref()
            .map_or_else(Default::default, |v| v.metrics())
    }
    fn set_vsync(&mut self, enabled: bool) -> PortResult<()> {
        if let Some(p) = &self.presentation {
            p.borrow_mut().set_vsync(enabled);
        }
        Ok(())
    }
    fn video_source_info(&self, provider: &str) -> Vec<vjay_application::preferences::VideoSource> {
        self.streams
            .factory
            .factory(provider)
            .map(|f| f.source_info())
            .unwrap_or_default()
    }
    fn video_sources(&self, provider: &str) -> Vec<String> {
        self.streams
            .factory
            .factory(provider)
            .map(|f| f.sources())
            .unwrap_or_default()
    }
    fn capture_output(&mut self, node: vjay_domain::graph::NodeId) -> PortResult<CapturedFrame> {
        let slot = self
            .output_slots
            .iter()
            .find(|(id, _)| *id == node)
            .map(|(_, slot)| *slot)
            .ok_or("Output is not connected")?;
        self.readback_slot(slot)
    }
    fn node_error(&self, node: vjay_domain::graph::NodeId) -> Option<&str> {
        self.streams
            .errors
            .get(&node)
            .or_else(|| self.images.errors.get(&node))
            .or_else(|| self.video.as_ref().and_then(|v| v.errors.get(&node)))
            .map(String::as_str)
    }
    fn media_duration(&self, node: vjay_domain::graph::NodeId) -> Option<f64> {
        self.video.as_ref().and_then(|v| v.duration(node))
    }
    fn media_ready(&self, node: vjay_domain::graph::NodeId) -> bool {
        self.video.as_ref().is_none_or(|v| v.ready(node))
    }
    fn media_status(&self) -> Option<&str> {
        self.video.as_ref().and_then(|v| v.status())
    }
    fn validate_shader(&self, source: &str) -> PortResult<()> {
        super::glsl::pipeline(&self.device, &self.layout, source).map(|_| ())
    }
    fn set_assets(&mut self, assets: &[MediaAsset]) {
        self.assets = assets
            .iter()
            .map(|asset| (asset.id, asset.path.clone()))
            .collect();
    }
    fn prepare(&mut self, plan: &RenderPlan, extent: Extent) -> PortResult<()> {
        self.prepare_plan(plan, extent)
    }
    fn render(&mut self, plan: &RenderPlan, frame: FrameContext) -> PortResult<()> {
        self.images.update(&self.queue, plan, frame.delivery)?;
        if let Some(native) = self.native_video.as_mut() {
            native.begin_frame()?;
        }
        let video_result = self
            .video
            .as_mut()
            .map_or(Ok(()), |video| video.update(&self.queue, plan, frame));
        // Commit accepted GPU video work even if another source failed. That
        // work can already own transfer slots or have registered fence waits.
        let batch_result = self
            .native_video
            .as_mut()
            .map_or(Ok(()), |native| native.end_frame());
        video_result?;
        batch_result?;
        let resources = self
            .resources
            .as_mut()
            .ok_or("Renderer has not been prepared")?;
        if resources.extent != frame.extent || resources.groups.len() != plan.steps.len() {
            return Err("Execution plan and prepared resources differ".into());
        }
        for (i, step) in plan.steps.iter().enumerate() {
            let viewport = [
                frame.extent.width as f32,
                frame.extent.height as f32,
                frame.seconds,
                step.operation.selector,
            ];
            let offset = i * resources.stride;
            resources.staging[offset..offset + 16].copy_from_slice(bytemuck::cast_slice(&viewport));
            let parameters = &step.operation.parameters;
            if parameters.len() > 64 {
                return Err("A GPU pass supports up to 64 scalar uniforms".into());
            }
            resources.staging[offset + 16..offset + 16 + parameters.len() * 4]
                .copy_from_slice(bytemuck::cast_slice(parameters));
        }
        // One contiguous upload and one command submission for the graph.
        self.queue
            .write_buffer(&resources.uniforms, 0, &resources.staging);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("vjay graph"),
            });
        self.streams
            .update(&self.device, &self.queue, &mut encoder)?;
        for (i, step) in plan.steps.iter().enumerate() {
            if !resources.forwarded[i] {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("node pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &resources.views[step.target],
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    ..Default::default()
                });
                pass.set_pipeline(&self.pipelines[i]);
                pass.set_bind_group(0, &resources.groups[i], &[(i * resources.stride) as u32]);
                pass.draw(0..3, 0..1);
                drop(pass);
            }
            // Consume the preview before pooled output textures may be reused.
            if let Some(p) = &self.presentation {
                for index in step.preview.iter().chain(&step.extra_previews) {
                    p.borrow_mut()
                        .encode_node(*index, &self.device, &mut encoder)?;
                }
            }
        }
        if !plan.program_active {
            let _clear = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("empty program"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &resources.views[plan.output_slot],
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
        }
        if let Some(presentation) = &self.presentation {
            presentation
                .borrow_mut()
                .encode(&self.device, &mut encoder)?
        }
        self.streams.encode(&mut encoder, &resources.textures)?;
        self.queue.submit([encoder.finish()]);
        self.submitted_frames += 1;
        self.streams.submitted(&self.device, &self.queue)?;
        if let Some(presentation) = &self.presentation {
            presentation.borrow_mut().present(&self.queue);
        }
        Ok(())
    }
    fn preview(&self) -> Option<PreviewToken> {
        self.presentation.as_ref().map(|_| PreviewToken(0))
    }
    fn capture(&mut self) -> PortResult<CapturedFrame> {
        self.readback()
    }
    fn description(&self) -> &str {
        &self.description
    }
}
