use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
struct Slot {
    buffer: wgpu::Buffer,
    busy: Arc<AtomicBool>,
    mapping: Arc<AtomicBool>,
    pending: bool,
}
pub struct Sender {
    slots: Vec<Slot>,
    worker: send::Worker,
    extent: Extent,
    padded: u32,
}
impl Sender {
    pub fn new(
        api: Arc<api::Api>,
        name: &str,
        device: &wgpu::Device,
        extent: Extent,
    ) -> PortResult<Self> {
        let padded = (extent.width * 4).div_ceil(256) * 256;
        let slots = (0..2)
            .map(|_| Slot {
                buffer: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("NDI async staging"),
                    size: padded as u64 * extent.height as u64,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                }),
                busy: Arc::new(AtomicBool::new(false)),
                mapping: Arc::new(AtomicBool::new(false)),
                pending: false,
            })
            .collect();
        Ok(Self {
            slots,
            worker: send::Worker::new(api, name, extent)?,
            extent,
            padded,
        })
    }
}
impl VideoSender for Sender {
    fn source_name(&self) -> Option<&str> {
        Some(&self.worker.source_name)
    }
    fn encode(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        image: &wgpu::Texture,
    ) -> PortResult<()> {
        if let Some(slot) = self
            .slots
            .iter_mut()
            .find(|s| !s.busy.swap(true, Ordering::AcqRel))
        {
            encoder.copy_texture_to_buffer(
                image.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: &slot.buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(self.padded),
                        rows_per_image: Some(self.extent.height),
                    },
                },
                image.size(),
            );
            slot.pending = true;
        }
        Ok(())
    }
    fn submitted(&mut self, device: &wgpu::Device, _: &wgpu::Queue) -> PortResult<()> {
        for slot in &mut self.slots {
            if !std::mem::take(&mut slot.pending) {
                continue;
            }
            let buffer = slot.buffer.clone();
            let busy = slot.busy.clone();
            let mapping = slot.mapping.clone();
            let mailbox = self.worker.mailbox.clone();
            let pool = self.worker.pool.clone();
            let row = self.extent.width as usize * 4;
            let height = self.extent.height as usize;
            let padded = self.padded as usize;
            mapping.store(true, Ordering::Release);
            slot.buffer
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |result| {
                    let owns_mapping = mapping.swap(false, Ordering::AcqRel) && result.is_ok();
                    if owns_mapping && !mailbox.stop.load(Ordering::Acquire) {
                        let data = pool.try_lock().ok().and_then(|mut p| p.pop());
                        if let Some(mut data) = data {
                            let copied = if let Ok(mapped) = buffer.slice(..).get_mapped_range() {
                                for y in 0..height {
                                    data[y * row..(y + 1) * row]
                                        .copy_from_slice(&mapped[y * padded..y * padded + row]);
                                }
                                true
                            } else {
                                false
                            };
                            buffer.unmap();
                            if !copied {
                                if let Ok(mut p) = pool.lock() {
                                    p.push(data);
                                }
                            } else if let Ok(mut ready) = mailbox.frame.try_lock() {
                                let old = ready.replace(data);
                                drop(ready);
                                if let Some(old) = old
                                    && let Ok(mut p) = pool.lock()
                                {
                                    p.push(old);
                                }
                                mailbox.ready.notify_one();
                            } else if let Ok(mut p) = pool.lock() {
                                p.push(data);
                            }
                        } else {
                            buffer.unmap();
                        }
                    } else if owns_mapping {
                        buffer.unmap();
                    }
                    busy.store(false, Ordering::Release);
                });
        }
        device
            .poll(wgpu::PollType::Poll)
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}
impl Drop for Sender {
    fn drop(&mut self) {
        self.worker.mailbox.stop.store(true, Ordering::Release);
        for slot in &self.slots {
            if slot.mapping.swap(false, Ordering::AcqRel) {
                slot.buffer.unmap();
            }
        }
    }
}
