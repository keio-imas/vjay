//! macOS native viewport implementation; selected only by the composition root.
mod clipping;
mod pass_through;
use crate::gpu::{
    blit::Blit,
    presentation::{GpuPresentation, PreviewBounds},
};
use gpui::Window;
use objc2::{MainThreadMarker, rc::Retained};
use objc2_app_kit::NSView;
use objc2_foundation::{NSPoint, NSRect, NSSize};
use objc2_quartz_core::CAMetalLayer;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use vjay_application::ports::PortResult;

/// Owns a child NSView and its own CAMetalLayer, leaving GPUI's UI renderer
/// independent. All AppKit access occurs on GPUI's main thread.
pub struct MacGpuPreview {
    pending: Option<wgpu::SurfaceTexture>,
    // Surface must be released before the backing layer and view.
    surface: wgpu::Surface<'static>,
    instance: wgpu::Instance,
    layer: Retained<CAMetalLayer>,
    child: Retained<NSView>,
    host: Retained<NSView>,
    config: Option<wgpu::SurfaceConfiguration>,
    blit: Option<Blit>,
    dimensions: [u32; 2],
    resize: bool,
    vsync: bool,
    last_bounds: Option<PreviewBounds>,
    hidden: bool,
    regions: Vec<crate::gpu::presentation::PreviewRegion>,
    mask_bounds: Option<[f32; 3]>,
}

impl MacGpuPreview {
    pub fn attach(window: &Window) -> PortResult<Self> {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::METAL;
        Self::attach_with_instance(window, wgpu::Instance::new(descriptor))
    }
    pub fn attach_with_instance(window: &Window, instance: wgpu::Instance) -> PortResult<Self> {
        let handle = HasWindowHandle::window_handle(window).map_err(|e| e.to_string())?;
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
            return Err("Expected an AppKit window".into());
        };
        // SAFETY: GPUI provides a live AppKit NSView. Retaining it gives this
        // adapter ownership through surface teardown, including window closure.
        let host = unsafe { Retained::retain(handle.ns_view.as_ptr().cast::<NSView>()) }
            .ok_or("Missing native view")?;
        Self::attach_host(host, instance)
    }
    fn attach_host(host: Retained<NSView>, instance: wgpu::Instance) -> PortResult<Self> {
        let mtm = MainThreadMarker::new().ok_or("Native preview requires the main thread")?;
        let child = pass_through::create(
            mtm,
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1.0, 1.0)),
        );
        let layer = CAMetalLayer::new();
        child.setWantsLayer(true);
        child.setLayer(Some(&layer));
        host.addSubview(&child);
        // SAFETY: `layer` is a real CAMetalLayer, owned by this adapter. The
        // surface field drops first; no raw window lifetime escapes the adapter.
        let surface = unsafe {
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(
                Retained::as_ptr(&layer).cast_mut().cast(),
            ))
        }
        .map_err(|e| e.to_string())?;
        Ok(Self {
            surface,
            instance,
            layer,
            child,
            host,
            config: None,
            blit: None,
            dimensions: [1, 1],
            resize: true,
            vsync: true,
            last_bounds: None,
            hidden: false,
            regions: Vec::new(),
            mask_bounds: None,
            pending: None,
        })
    }
}

impl GpuPresentation for MacGpuPreview {
    fn set_vsync(&mut self, enabled: bool) {
        self.vsync = enabled;
        if let Some(c) = &mut self.config {
            c.present_mode = if enabled {
                wgpu::PresentMode::AutoVsync
            } else {
                wgpu::PresentMode::AutoNoVsync
            };
            self.resize = true;
        }
    }
    fn set_preview_regions(&mut self, regions: &[crate::gpu::presentation::PreviewRegion]) {
        self.apply_regions(regions);
    }
    fn set_hidden(&mut self, hidden: bool) {
        if self.hidden != hidden {
            self.hidden = hidden;
            self.child.setHidden(hidden);
        }
    }
    fn instance(&self) -> &wgpu::Instance {
        &self.instance
    }
    fn request_adapter(&self) -> PortResult<wgpu::Adapter> {
        pollster::block_on(self.instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&self.surface),
            force_fallback_adapter: false,
            ..Default::default()
        }))
        .map_err(|e| e.to_string())
    }
    fn initialize(&mut self, device: &wgpu::Device, adapter: &wgpu::Adapter) -> PortResult<()> {
        let mut config = self
            .surface
            .get_default_config(adapter, 1, 1)
            .ok_or("No compatible native surface")?;
        let capabilities = self.surface.get_capabilities(adapter);
        config.format = capabilities
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(config.format);
        config.present_mode = if self.vsync {
            wgpu::PresentMode::AutoVsync
        } else {
            wgpu::PresentMode::AutoNoVsync
        };
        config.desired_maximum_frame_latency = 2;
        self.blit = Some(Blit::new(device, config.format));
        self.config = Some(config);
        Ok(())
    }
    fn prepare(&mut self, device: &wgpu::Device, image: &wgpu::TextureView) -> PortResult<()> {
        self.blit
            .as_mut()
            .ok_or("Preview is not initialized")?
            .prepare(device, image);
        Ok(())
    }
    fn set_bounds(&mut self, bounds: PreviewBounds) {
        if self.last_bounds == Some(bounds) {
            return;
        }
        self.last_bounds = Some(bounds);
        let y = if self.host.isFlipped() {
            bounds.y
        } else {
            bounds.window_height - bounds.y - bounds.height
        };
        self.child.setFrame(NSRect::new(
            NSPoint::new(bounds.x as f64, y as f64),
            NSSize::new(bounds.width as f64, bounds.height as f64),
        ));
        self.layer.setContentsScale(bounds.scale as f64);
        let dimensions = [
            (bounds.width * bounds.scale).round().max(1.0) as u32,
            (bounds.height * bounds.scale).round().max(1.0) as u32,
        ];
        if dimensions != self.dimensions {
            self.dimensions = dimensions;
            self.resize = true;
        }
    }
    fn encode(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
    ) -> PortResult<()> {
        if self.hidden {
            return Ok(());
        }
        if self
            .host
            .window()
            .is_none_or(|window| window.isMiniaturized() || !window.isVisible())
        {
            return Ok(());
        }
        let config = self.config.as_mut().ok_or("Preview is not initialized")?;
        if self.resize {
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
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.resize = true;
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                return Err("Native preview surface was lost; restart VJAY".into());
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err("Native preview surface validation failed".into());
            }
        };
        let view = frame.texture.create_view(&Default::default());
        self.blit
            .as_ref()
            .ok_or("Preview pipeline unavailable")?
            .encode(encoder, &view);
        self.pending = Some(frame);
        Ok(())
    }
    fn present(&mut self, queue: &wgpu::Queue) {
        if let Some(frame) = self.pending.take() {
            queue.present(frame);
        }
    }
}

pub struct MacOutputFactory {
    context: crate::gpu::GpuContext,
    group: std::rc::Rc<std::cell::RefCell<crate::gpu::presentation_group::PresentationGroup>>,
}
impl MacOutputFactory {
    pub fn new(
        context: crate::gpu::GpuContext,
        group: std::rc::Rc<std::cell::RefCell<crate::gpu::presentation_group::PresentationGroup>>,
    ) -> Self {
        Self { context, group }
    }
}
impl crate::platform::OutputFactory for MacOutputFactory {
    fn create(
        &self,
        window: &Window,
        source: vjay_domain::graph::NodeId,
    ) -> PortResult<crate::platform::SharedPreview> {
        let mut output =
            MacGpuPreview::attach_with_instance(window, self.context.instance.clone())?;
        output.initialize(&self.context.device, &self.context.adapter)?;
        let output: crate::platform::SharedPreview =
            std::rc::Rc::new(std::cell::RefCell::new(output));
        self.group.borrow_mut().register(source, &output)?;
        Ok(output)
    }
}

impl Drop for MacGpuPreview {
    fn drop(&mut self) {
        self.child.removeFromSuperview();
    }
}

pub struct MacNodePreviewFactory {
    host: Retained<NSView>,
    context: crate::gpu::GpuContext,
}
impl MacNodePreviewFactory {
    pub fn new(window: &Window, context: crate::gpu::GpuContext) -> PortResult<Self> {
        let handle = HasWindowHandle::window_handle(window).map_err(|e| e.to_string())?;
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
            return Err("Expected AppKit window".into());
        };
        let host = unsafe { Retained::retain(handle.ns_view.as_ptr().cast::<NSView>()) }
            .ok_or("Missing native view")?;
        Ok(Self { host, context })
    }
}
impl crate::gpu::presentation::NodePreviewFactory for MacNodePreviewFactory {
    fn create(&self) -> PortResult<Box<dyn GpuPresentation>> {
        let mut preview =
            MacGpuPreview::attach_host(self.host.clone(), self.context.instance.clone())?;
        preview.initialize(&self.context.device, &self.context.adapter)?;
        Ok(Box::new(preview))
    }
}
