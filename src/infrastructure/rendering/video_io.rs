use std::sync::Arc;
use vjay_application::ports::{Extent, PortResult};
use vjay_domain::execution::StreamRequest;
#[cfg(target_os = "macos")]
pub mod camera;
#[cfg(target_os = "macos")]
pub(crate) mod metal;
pub mod ndi;
#[cfg(target_os = "windows")]
pub mod spout;
#[cfg(target_os = "macos")]
pub mod syphon;
pub trait VideoReceiver {
    fn status(&self) -> Option<&'static str> {
        None
    }
    fn texture(&self) -> &wgpu::Texture;
    fn update(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
    ) -> PortResult<()>;
}
pub trait VideoSender {
    fn source_name(&self) -> Option<&str> {
        None
    }
    fn encode(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        image: &wgpu::Texture,
    ) -> PortResult<()>;
    fn submitted(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> PortResult<()>;
}
/// GPU adapter factories stay behind RenderBackend; the application exchanges StreamRequest only.
pub trait VideoIoFactory: Send + Sync {
    fn source_info(&self) -> Vec<vjay_application::preferences::VideoSource> {
        self.sources()
            .into_iter()
            .map(|name| vjay_application::preferences::VideoSource {
                id: name.clone(),
                name,
            })
            .collect()
    }
    fn sources(&self) -> Vec<String>;
    fn receiver(
        &self,
        request: &StreamRequest,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        extent: Extent,
    ) -> PortResult<Box<dyn VideoReceiver>>;
    fn sender(
        &self,
        request: &StreamRequest,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        extent: Extent,
    ) -> PortResult<Box<dyn VideoSender>>;
}
#[derive(Default)]
pub struct VideoIoRegistry(pub Vec<(&'static str, Arc<dyn VideoIoFactory>)>);
impl VideoIoRegistry {
    pub fn factory(&self, provider: &str) -> PortResult<&dyn VideoIoFactory> {
        self.0
            .iter()
            .find(|(id, _)| *id == provider)
            .map(|(_, f)| f.as_ref())
            .ok_or_else(|| format!("Video transport {provider} is unavailable on this platform"))
    }
}
pub struct Unavailable(pub String);
impl VideoIoFactory for Unavailable {
    fn sources(&self) -> Vec<String> {
        vec![]
    }
    fn receiver(
        &self,
        _: &StreamRequest,
        _: &wgpu::Device,
        _: &wgpu::Queue,
        _: Extent,
    ) -> PortResult<Box<dyn VideoReceiver>> {
        Err(self.0.clone())
    }
    fn sender(
        &self,
        _: &StreamRequest,
        _: &wgpu::Device,
        _: &wgpu::Queue,
        _: Extent,
    ) -> PortResult<Box<dyn VideoSender>> {
        Err(self.0.clone())
    }
}
pub(crate) fn texture(device: &wgpu::Device, extent: Extent) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("external video input"),
        size: wgpu::Extent3d {
            width: extent.width,
            height: extent.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::STORAGE_BINDING
            | wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    })
}
