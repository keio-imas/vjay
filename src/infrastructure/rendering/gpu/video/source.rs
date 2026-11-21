use std::{path::Path, sync::Arc};
use vjay_application::{
    media::{FrameDelivery, MediaMetrics, VideoDecoder},
    ports::*,
};

/// Backend-local PAL: native implementations keep decoded images on the GPU.
/// The application only depends on RenderBackend and the portable media ports.
pub trait GpuVideoFactory: Send + Sync {
    fn create(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> PortResult<Box<dyn GpuVideoRuntime>>;
}
pub trait GpuVideoRuntime {
    fn open(&mut self, path: &Path, extent: Extent) -> PortResult<Box<dyn GpuVideoDecoder>>;
}
pub trait GpuVideoDecoder {
    fn texture(&self) -> &wgpu::Texture;
    fn duration(&self) -> f64;
    fn ready(&self) -> bool;
    fn failed(&self) -> bool;
    fn status(&self) -> Option<&str>;
    fn frame_at(
        &mut self,
        seconds: f64,
        delivery: FrameDelivery,
        queue: &wgpu::Queue,
    ) -> PortResult<bool>;
    fn metrics(&self) -> MediaMetrics;
}
pub(super) struct UploadedVideo {
    decoder: Box<dyn VideoDecoder>,
    texture: wgpu::Texture,
    extent: Extent,
}
impl UploadedVideo {
    pub fn new(device: &wgpu::Device, decoder: Box<dyn VideoDecoder>, extent: Extent) -> Self {
        Self {
            decoder,
            extent,
            texture: device.create_texture(&wgpu::TextureDescriptor {
                label: Some("CPU video input"),
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
            }),
        }
    }
}
impl GpuVideoDecoder for UploadedVideo {
    fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }
    fn duration(&self) -> f64 {
        self.decoder.duration()
    }
    fn ready(&self) -> bool {
        self.decoder.ready()
    }
    fn failed(&self) -> bool {
        self.decoder.failed()
    }
    fn status(&self) -> Option<&str> {
        self.decoder.status()
    }
    fn metrics(&self) -> MediaMetrics {
        self.decoder.metrics()
    }
    fn frame_at(
        &mut self,
        seconds: f64,
        delivery: FrameDelivery,
        queue: &wgpu::Queue,
    ) -> PortResult<bool> {
        let Some(frame) = self.decoder.frame_at(seconds, delivery)? else {
            return Ok(false);
        };
        queue.write_texture(
            self.texture.as_image_copy(),
            &frame.rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.extent.width * 4),
                rows_per_image: Some(self.extent.height),
            },
            wgpu::Extent3d {
                width: self.extent.width,
                height: self.extent.height,
                depth_or_array_layers: 1,
            },
        );
        self.decoder.recycle(frame);
        Ok(true)
    }
}

pub(crate) type NativeFactory = Option<Arc<dyn GpuVideoFactory>>;
