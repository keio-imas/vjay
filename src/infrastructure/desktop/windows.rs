//! Win32 child viewports with independent DX12 swapchains on the shared VJ device.
mod clipping;
mod factories;
mod surface;
#[cfg(test)]
mod tests;
mod window;

use crate::gpu::presentation::{GpuPresentation, PreviewBounds, PreviewRegion};
pub use factories::{WindowsNodePreviewFactory, WindowsOutputFactory};
use raw_window_handle::{DisplayHandle, RawWindowHandle, Win32WindowHandle};
use std::num::NonZeroIsize;
use vjay_application::ports::PortResult;
use window::ChildWindow;

pub struct WindowsGpuPreview {
    instance: wgpu::Instance,
    window: ChildWindow,
    error: Option<String>,
}

impl WindowsGpuPreview {
    pub fn attach(window: &gpui::Window) -> PortResult<Self> {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::DX12;
        Self::attach_with_instance(window, wgpu::Instance::new(descriptor))
    }

    pub fn attach_with_instance(
        window: &gpui::Window,
        instance: wgpu::Instance,
    ) -> PortResult<Self> {
        Self::attach_host(window::host_handle(window)?, instance)
    }

    fn attach_host(
        host: ::windows::Win32::Foundation::HWND,
        instance: wgpu::Instance,
    ) -> PortResult<Self> {
        let window = ChildWindow::new(host)?;
        let handle = NonZeroIsize::new(window.hwnd().0 as isize).ok_or("Missing preview HWND")?;
        // SAFETY: The child window's destruction callback drops the surface before
        // Win32 destroys its HWND, including when its GPUI parent closes first.
        let surface = unsafe {
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                // Windows display handles borrow no external resources. wgpu 30
                // requires one here when the instance has no display of its own.
                raw_display_handle: Some(DisplayHandle::windows().as_raw()),
                raw_window_handle: RawWindowHandle::Win32(Win32WindowHandle::new(handle)),
            })
        }
        .map_err(|e| format!("Cannot create Windows DX12 preview: {e}"))?;
        *window.state.gpu.borrow_mut() = Some(surface::SurfacePresenter::new(surface));
        Ok(Self {
            instance,
            window,
            error: None,
        })
    }
}

impl GpuPresentation for WindowsGpuPreview {
    fn instance(&self) -> &wgpu::Instance {
        &self.instance
    }

    fn request_adapter(&self) -> PortResult<wgpu::Adapter> {
        let gpu = self.window.state.gpu.borrow();
        let gpu = gpu.as_ref().ok_or("Windows preview is closed")?;
        let adapter =
            pollster::block_on(self.instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&gpu.surface),
                force_fallback_adapter: false,
                ..Default::default()
            }))
            .map_err(|e| format!("No DX12 adapter for Windows preview: {e}"))?;
        if adapter.get_info().device_type == wgpu::DeviceType::Cpu {
            return Err(
                "Windows preview requires a hardware DX12 GPU; software rendering is disabled"
                    .into(),
            );
        }
        Ok(adapter)
    }

    fn initialize(&mut self, device: &wgpu::Device, adapter: &wgpu::Adapter) -> PortResult<()> {
        self.window
            .state
            .gpu
            .borrow_mut()
            .as_mut()
            .ok_or("Windows preview is closed")?
            .initialize(device, adapter)
    }

    fn prepare(&mut self, device: &wgpu::Device, image: &wgpu::TextureView) -> PortResult<()> {
        if let Some(gpu) = self.window.state.gpu.borrow_mut().as_mut() {
            gpu.prepare(device, image)?;
        }
        Ok(())
    }

    fn set_vsync(&mut self, enabled: bool) {
        if let Some(gpu) = self.window.state.gpu.borrow_mut().as_mut() {
            gpu.set_vsync(enabled);
        }
    }

    fn set_hidden(&mut self, hidden: bool) {
        self.window.set_hidden(hidden);
    }

    fn set_bounds(&mut self, bounds: PreviewBounds) {
        // Never hold a GPU-state borrow across Win32 calls: they can deliver
        // destruction messages synchronously and must be able to release it.
        match self.window.set_bounds(bounds) {
            Ok(dimensions) => {
                self.error = None;
                if let Some(gpu) = self.window.state.gpu.borrow_mut().as_mut() {
                    gpu.set_dimensions(dimensions);
                }
            }
            Err(error) => self.error = Some(error),
        }
    }

    fn set_preview_regions(&mut self, regions: &[PreviewRegion]) {
        if let Err(error) = self.window.set_regions(regions) {
            self.error = Some(error);
        }
    }

    fn encode(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
    ) -> PortResult<()> {
        if !self.window.drawable() {
            return Ok(());
        }
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        if let Some(gpu) = self.window.state.gpu.borrow_mut().as_mut() {
            gpu.encode(device, encoder)?;
        }
        Ok(())
    }

    fn present(&mut self, queue: &wgpu::Queue) {
        if let Some(gpu) = self.window.state.gpu.borrow_mut().as_mut() {
            gpu.present(queue);
        }
    }
}
