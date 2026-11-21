use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_metal::{MTLDevice, MTLSharedEvent};
use vjay_application::ports::PortResult;
pub(crate) struct Fence {
    pub event: Retained<ProtocolObject<dyn MTLSharedEvent>>,
    pub value: u64,
}
impl Fence {
    pub fn new(device: &wgpu::Device) -> PortResult<Self> {
        let metal = unsafe { device.as_hal::<wgpu::hal::api::Metal>() }.ok_or("Expected Metal")?;
        let event = metal
            .raw_device()
            .newSharedEvent()
            .ok_or("Cannot create Metal shared event")?;
        Ok(Self { event, value: 0 })
    }
    pub fn signal_from_wgpu(&mut self, queue: &wgpu::Queue) -> PortResult<()> {
        self.value += 1;
        let metal = unsafe { queue.as_hal::<wgpu::hal::api::Metal>() }.ok_or("Expected Metal")?;
        metal.add_signal_event(self.event.clone(), self.value);
        Ok(())
    }
    pub fn wait_in_wgpu(&self, queue: &wgpu::Queue) -> PortResult<()> {
        let metal = unsafe { queue.as_hal::<wgpu::hal::api::Metal>() }.ok_or("Expected Metal")?;
        metal.add_wait_event(self.event.clone(), self.value);
        Ok(())
    }
}

/// Mark the persistent input initialized in wgpu before any external Metal writes.
/// Otherwise wgpu's first sampled use clears the externally produced frame.
pub(crate) fn initialize_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
) {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("external input initialization"),
    });
    {
        let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &texture.create_view(&Default::default()),
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
    queue.submit([encoder.finish()]);
}
