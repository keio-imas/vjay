//! Media Foundation / D3D11 video surfaces shared with the wgpu D3D12 device.
mod device;
mod format;
mod processor;
mod reader;
mod stream;
#[cfg(test)]
mod tests;
mod transfer;
mod worker;

use crate::gpu::{GpuVideoDecoder, GpuVideoFactory, GpuVideoRuntime};
use std::{cell::RefCell, path::Path, rc::Rc};
use transfer::TransferBatch;
use vjay_application::ports::{Extent, PortResult};

pub struct Dx12MovieFactory;

struct Runtime {
    device: wgpu::Device,
    queue: wgpu::Queue,
    transfers: Rc<RefCell<TransferBatch>>,
}

impl GpuVideoFactory for Dx12MovieFactory {
    fn create(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> PortResult<Box<dyn GpuVideoRuntime>> {
        if unsafe { device.as_hal::<wgpu::hal::api::Dx12>() }.is_none() {
            return Err("Windows GPU movies require the Direct3D 12 backend".into());
        }
        Ok(Box::new(Runtime {
            device: device.clone(),
            queue: queue.clone(),
            transfers: Rc::new(RefCell::new(TransferBatch::new(device, queue))),
        }))
    }
}

impl GpuVideoRuntime for Runtime {
    fn begin_frame(&mut self) -> PortResult<()> {
        self.transfers.borrow_mut().begin_frame();
        Ok(())
    }

    fn end_frame(&mut self) -> PortResult<()> {
        self.transfers.borrow_mut().end_frame()
    }

    fn open(&mut self, path: &Path, extent: Extent) -> PortResult<Box<dyn GpuVideoDecoder>> {
        stream::NativeMovie::open(
            &self.device,
            &self.queue,
            self.transfers.clone(),
            path.to_owned(),
            extent,
        )
        .map(|movie| Box::new(movie) as Box<dyn GpuVideoDecoder>)
    }
}
