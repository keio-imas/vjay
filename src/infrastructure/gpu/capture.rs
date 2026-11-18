use super::renderer::WgpuRenderer;
use std::{sync::mpsc, time::Duration};
use vjay_application::ports::{CapturedFrame, PortResult};

impl WgpuRenderer {
    /// Explicit export path only. Preview never copies pixels back to the CPU.
    pub(super) fn readback(&self) -> PortResult<CapturedFrame> {
        let slot = self
            .resources
            .as_ref()
            .ok_or("No frame to capture")?
            .output_slot;
        self.readback_slot(slot)
    }
    pub(super) fn readback_slot(&self, slot: usize) -> PortResult<CapturedFrame> {
        let r = self.resources.as_ref().ok_or("No frame to capture")?;
        let row = r.extent.width * 4;
        let padded =
            row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("capture readback"),
            size: padded as u64 * r.extent.height as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            r.textures[slot].as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(r.extent.height),
                },
            },
            wgpu::Extent3d {
                width: r.extent.width,
                height: r.extent.height,
                depth_or_array_layers: 1,
            },
        );
        let submission = self.queue.submit([encoder.finish()]);
        let (send, receive) = mpsc::sync_channel(1);
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = send.send(result);
            });
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(Duration::from_secs(10)),
            })
            .map_err(|e| e.to_string())?;
        receive
            .recv_timeout(Duration::from_secs(10))
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
        let mapped = buffer
            .slice(..)
            .get_mapped_range()
            .map_err(|e| e.to_string())?;
        let mut rgba = Vec::with_capacity((row * r.extent.height) as usize);
        for line in mapped.chunks_exact(padded as usize) {
            rgba.extend_from_slice(&line[..row as usize]);
        }
        drop(mapped);
        buffer.unmap();
        Ok(CapturedFrame {
            extent: r.extent,
            rgba,
        })
    }
}
