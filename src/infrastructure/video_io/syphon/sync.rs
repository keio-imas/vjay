use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_metal::{MTLDevice, MTLSharedEvent};
use vjay_application::ports::PortResult;
pub(super) struct Fence {
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
