use std::sync::Arc;
use std::time::Instant;
use std::{cell::RefCell, rc::Rc};
use vjay_application::media::VideoDecoderFactory;
use vjay_application::ports::{Clock, PortResult, RenderBackend, RuntimeFactory};

pub(crate) mod blit;
mod capture;
pub mod glsl;
pub mod presentation;
pub mod presentation_group;
pub mod programs;
mod renderer;
mod resources;

#[derive(Clone)]
pub struct GpuContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}
mod prepared;
mod stills;
#[derive(Clone, Default)]
pub(super) struct RendererAdapters {
    files: Option<Arc<dyn vjay_application::media::ImageLoader>>,
    generated: Option<Arc<dyn vjay_application::media::PreparedImageFactory>>,
    video_io: Arc<crate::video_io::VideoIoRegistry>,
}
mod streams;
mod video;

pub struct WgpuRuntimeFactory {
    device: wgpu::Device,
    queue: wgpu::Queue,
    presentation: Option<Rc<RefCell<dyn presentation::GpuPresentation>>>,
    description: String,
    media: Arc<dyn VideoDecoderFactory>,
    images: RendererAdapters,
    context: GpuContext,
    controls: Option<Rc<dyn Fn() -> Box<dyn vjay_application::control_io::ControlIo>>>,
    audio: Option<Rc<dyn vjay_application::audio::AudioFactory>>,
    programs: Arc<programs::ShaderPrograms>,
    assets: Option<Arc<dyn vjay_application::asset_sources::AssetSourceFactory>>,
}

impl WgpuRuntimeFactory {
    pub fn for_desktop(
        presentation: Rc<RefCell<dyn presentation::GpuPresentation>>,
        media: Arc<dyn VideoDecoderFactory>,
        programs: Arc<programs::ShaderPrograms>,
    ) -> PortResult<Self> {
        let adapter = {
            let p = presentation.borrow();
            pollster::block_on(p.instance().request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(p.surface()),
                force_fallback_adapter: false,
                ..Default::default()
            }))
            .map_err(|e| e.to_string())?
        };
        let info = adapter.get_info();
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("vjay desktop device"),
            ..Default::default()
        }))
        .map_err(|e| e.to_string())?;
        presentation.borrow_mut().initialize(&device, &adapter)?;
        let description = format!("{} / {:?}", info.name, info.backend);
        let context = GpuContext {
            instance: presentation.borrow().instance().clone(),
            adapter,
            device: device.clone(),
            queue: queue.clone(),
        };
        Ok(Self {
            device,
            queue,
            description,
            presentation: Some(presentation),
            media,
            images: RendererAdapters::default(),
            context,
            programs,
            assets: None,
            controls: None,
            audio: None,
        })
    }
    pub fn headless(
        media: Arc<dyn VideoDecoderFactory>,
        programs: Arc<programs::ShaderPrograms>,
    ) -> PortResult<Self> {
        Self::headless_with_backends(media, programs, wgpu::Backends::all())
    }
    pub fn headless_with_backends(
        media: Arc<dyn VideoDecoderFactory>,
        programs: Arc<programs::ShaderPrograms>,
        backends: wgpu::Backends,
    ) -> PortResult<Self> {
        pollster::block_on(async {
            let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
            descriptor.backends = backends;
            let instance = wgpu::Instance::new(descriptor);
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    compatible_surface: None,
                    force_fallback_adapter: false,
                    ..Default::default()
                })
                .await
                .map_err(|e| format!("No WebGPU adapter: {e}"))?;
            let info = adapter.get_info();
            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("vjay device"),
                    ..Default::default()
                })
                .await
                .map_err(|e| e.to_string())?;
            let context = GpuContext {
                instance,
                adapter,
                device: device.clone(),
                queue: queue.clone(),
            };
            Ok(Self {
                device,
                queue,
                presentation: None,
                description: format!("{} / {:?}", info.name, info.backend),
                media,
                images: RendererAdapters::default(),
                context,
                programs,
                assets: None,
                controls: None,
                audio: None,
            })
        })
    }
    pub fn with_video_io(mut self, factory: Arc<crate::video_io::VideoIoRegistry>) -> Self {
        self.images.video_io = factory;
        self
    }
    pub fn with_images(mut self, loader: Arc<dyn vjay_application::media::ImageLoader>) -> Self {
        self.images.files = Some(loader);
        self
    }
    pub fn context(&self) -> GpuContext {
        self.context.clone()
    }
    pub fn with_prepared_images(
        mut self,
        factory: Arc<dyn vjay_application::media::PreparedImageFactory>,
    ) -> Self {
        self.images.generated = Some(factory);
        self
    }
    pub fn with_asset_sources(
        mut self,
        factory: Arc<dyn vjay_application::asset_sources::AssetSourceFactory>,
    ) -> Self {
        self.assets = Some(factory);
        self
    }
    pub fn with_controls(
        mut self,
        create: impl Fn() -> Box<dyn vjay_application::control_io::ControlIo> + 'static,
    ) -> Self {
        self.controls = Some(Rc::new(create));
        self
    }
    pub fn with_audio(mut self, create: Rc<dyn vjay_application::audio::AudioFactory>) -> Self {
        self.audio = Some(create);
        self
    }
}

impl RuntimeFactory for WgpuRuntimeFactory {
    fn create_asset_sources(&self) -> Arc<dyn vjay_application::asset_sources::AssetSourceFactory> {
        self.assets
            .clone()
            .unwrap_or_else(|| Arc::new(vjay_application::asset_sources::DisabledAssetSources))
    }
    fn create_control_io(&self) -> Box<dyn vjay_application::control_io::ControlIo> {
        self.controls.as_ref().map_or_else(
            || {
                Box::new(vjay_application::control_io::DisabledControlIo)
                    as Box<dyn vjay_application::control_io::ControlIo>
            },
            |create| create(),
        )
    }
    fn create_audio(&self) -> Box<dyn vjay_application::audio::AudioBackend> {
        self.audio.as_ref().map_or_else(
            || {
                Box::new(vjay_application::audio::DisabledAudio)
                    as Box<dyn vjay_application::audio::AudioBackend>
            },
            |create| create.create(),
        )
    }
    fn create_renderer(&self) -> PortResult<Box<dyn RenderBackend>> {
        Ok(Box::new(renderer::WgpuRenderer::new(
            self.device.clone(),
            self.queue.clone(),
            self.presentation.clone(),
            self.description.clone(),
            self.media.clone(),
            self.programs.clone(),
            self.images.clone(),
        )?))
    }
    fn create_clock(&self) -> Box<dyn Clock> {
        Box::new(MonotonicClock(Instant::now()))
    }
}

struct MonotonicClock(Instant);
impl Clock for MonotonicClock {
    fn seconds(&self) -> f64 {
        self.0.elapsed().as_secs_f64()
    }
}
