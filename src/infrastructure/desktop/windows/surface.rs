use crate::gpu::blit::Blit;
use vjay_application::ports::PortResult;

/// This state is owned jointly with the child HWND's destruction callback.
pub(super) struct SurfacePresenter {
    pending: Option<wgpu::SurfaceTexture>,
    pub surface: wgpu::Surface<'static>,
    config: Option<wgpu::SurfaceConfiguration>,
    blit: Option<Blit>,
    dimensions: [u32; 2],
    resize: bool,
    vsync: bool,
}

impl SurfacePresenter {
    #[cfg(test)]
    pub fn has_pending_frame(&self) -> bool {
        self.pending.is_some()
    }

    pub fn new(surface: wgpu::Surface<'static>) -> Self {
        Self {
            surface,
            pending: None,
            config: None,
            blit: None,
            dimensions: [1, 1],
            resize: true,
            vsync: true,
        }
    }

    pub fn initialize(&mut self, device: &wgpu::Device, adapter: &wgpu::Adapter) -> PortResult<()> {
        let mut config = self
            .surface
            .get_default_config(adapter, 1, 1)
            .ok_or("No compatible Windows swapchain configuration")?;
        config.format = self
            .surface
            .get_capabilities(adapter)
            .formats
            .into_iter()
            .find(|f| !f.is_srgb())
            .unwrap_or(config.format);
        config.present_mode = self.present_mode();
        config.desired_maximum_frame_latency = 2;
        self.blit = Some(Blit::new(device, config.format));
        self.config = Some(config);
        Ok(())
    }

    pub fn prepare(&mut self, device: &wgpu::Device, image: &wgpu::TextureView) -> PortResult<()> {
        self.blit
            .as_mut()
            .ok_or("Windows preview is not initialized")?
            .prepare(device, image);
        Ok(())
    }

    fn present_mode(&self) -> wgpu::PresentMode {
        if self.vsync {
            wgpu::PresentMode::AutoVsync
        } else {
            wgpu::PresentMode::AutoNoVsync
        }
    }

    pub fn set_vsync(&mut self, enabled: bool) {
        if self.vsync == enabled {
            return;
        }
        self.vsync = enabled;
        let mode = self.present_mode();
        if let Some(config) = &mut self.config {
            config.present_mode = mode;
        }
        self.resize = true;
    }

    pub fn set_dimensions(&mut self, dimensions: [u32; 2]) {
        if self.dimensions != dimensions {
            self.dimensions = dimensions;
            self.resize = true;
        }
    }

    pub fn encode(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
    ) -> PortResult<()> {
        if self.dimensions.contains(&0) {
            return Ok(());
        }
        let config = self
            .config
            .as_mut()
            .ok_or("Windows preview is not initialized")?;
        if self.resize {
            let limit = device.limits().max_texture_dimension_2d;
            if self.dimensions.iter().any(|&size| size > limit) {
                return Err(format!(
                    "Windows viewport exceeds GPU texture limit {limit}"
                ));
            }
            config.width = self.dimensions[0];
            config.height = self.dimensions[1];
            self.surface.configure(device, config);
            self.resize = false;
        }
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => frame,
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                self.resize = true;
                frame
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.resize = true;
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err("Windows preview surface validation failed".into());
            }
        };
        let view = frame.texture.create_view(&Default::default());
        self.blit
            .as_ref()
            .ok_or("Windows preview pipeline unavailable")?
            .encode(encoder, &view);
        self.pending = Some(frame);
        Ok(())
    }

    pub fn present(&mut self, queue: &wgpu::Queue) {
        if let Some(frame) = self.pending.take() {
            queue.present(frame);
        }
    }
}
