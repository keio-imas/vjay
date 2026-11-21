mod ffi;
mod stream;
use crate::gpu::{GpuVideoDecoder, GpuVideoFactory, GpuVideoRuntime};
use objc2::rc::Retained;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use vjay_application::ports::*;

pub struct MetalMovieFactory {
    ffmpeg: PathBuf,
    ffprobe: PathBuf,
}
impl MetalMovieFactory {
    pub fn new(ffmpeg: PathBuf, ffprobe: PathBuf) -> Self {
        Self { ffmpeg, ffprobe }
    }
}
struct Runtime {
    context: Arc<ffi::Context>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    ffmpeg: PathBuf,
    ffprobe: PathBuf,
}
impl GpuVideoFactory for MetalMovieFactory {
    fn create(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> PortResult<Box<dyn GpuVideoRuntime>> {
        let native_device = unsafe { device.as_hal::<wgpu::hal::api::Metal>() }
            .ok_or("Native movies require Metal")?;
        let native_queue = unsafe { queue.as_hal::<wgpu::hal::api::Metal>() }
            .ok_or("Native movies require a Metal queue")?;
        let mut error = [0; 1024];
        let context = unsafe {
            ffi::vjay_movie_context_create(
                Retained::as_ptr(native_device.raw_device())
                    .cast_mut()
                    .cast(),
                native_queue.as_raw() as *const _ as ffi::Raw,
                error.as_mut_ptr(),
                error.len(),
            )
        };
        if context.is_null() {
            return Err(ffi::message(&error));
        }
        Ok(Box::new(Runtime {
            context: Arc::new(ffi::Context(context)),
            device: device.clone(),
            queue: queue.clone(),
            ffmpeg: self.ffmpeg.clone(),
            ffprobe: self.ffprobe.clone(),
        }))
    }
}
impl GpuVideoRuntime for Runtime {
    fn open(&mut self, path: &Path, extent: Extent) -> PortResult<Box<dyn GpuVideoDecoder>> {
        stream::NativeMovie::open(
            &self.device,
            &self.queue,
            self.context.clone(),
            path.to_owned(),
            extent,
            self.ffmpeg.clone(),
            self.ffprobe.clone(),
        )
        .map(|v| Box::new(v) as Box<dyn GpuVideoDecoder>)
    }
}
