use super::device::{Device, Fence, SharedHandle};
use std::sync::{Arc, mpsc::SyncSender};
use vjay_application::ports::{Extent, PortResult};
use windows::{
    Win32::Graphics::{
        Direct3D11::*,
        Direct3D12::ID3D12Resource,
        Dxgi::{Common::*, DXGI_SHARED_RESOURCE_READ, DXGI_SHARED_RESOURCE_WRITE, IDXGIResource1},
    },
    core::{Interface, PCWSTR},
};

pub(super) struct Slot {
    pub texture: ID3D11Texture2D,
    pub imported: wgpu::Texture,
    pub producer: Fence,
    pub consumer: Fence,
}

impl Slot {
    pub fn new(
        native: &Device,
        device: &wgpu::Device,
        extent: Extent,
    ) -> windows::core::Result<Arc<Self>> {
        unsafe {
            let mut texture = None;
            native.d11.CreateTexture2D(
                &D3D11_TEXTURE2D_DESC {
                    Width: extent.width,
                    Height: extent.height,
                    MipLevels: 1,
                    ArraySize: 1,
                    Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    Usage: D3D11_USAGE_DEFAULT,
                    BindFlags: (D3D11_BIND_RENDER_TARGET | D3D11_BIND_SHADER_RESOURCE).0 as u32,
                    MiscFlags: (D3D11_RESOURCE_MISC_SHARED | D3D11_RESOURCE_MISC_SHARED_NTHANDLE).0
                        as u32,
                    ..Default::default()
                },
                None,
                Some(&mut texture),
            )?;
            let texture = texture.expect("shared video texture created");
            let mut view = None;
            native
                .d11
                .CreateRenderTargetView(&texture, None, Some(&mut view))?;
            native
                .context
                .ClearRenderTargetView(view.as_ref().expect("clear target"), &[0.0, 0.0, 0.0, 1.0]);
            let dxgi: IDXGIResource1 = texture.cast()?;
            let handle = SharedHandle(dxgi.CreateSharedHandle(
                None,
                DXGI_SHARED_RESOURCE_READ.0 | DXGI_SHARED_RESOURCE_WRITE.0,
                PCWSTR::null(),
            )?);
            let mut resource: Option<ID3D12Resource> = None;
            native.d12.OpenSharedHandle(handle.0, &mut resource)?;
            let desc = descriptor(
                extent,
                "Shared GPU video transfer",
                wgpu::TextureUsages::COPY_SRC,
            );
            let hal = wgpu::hal::dx12::Device::texture_from_raw(
                resource.expect("DX12 import"),
                desc.format,
                desc.dimension,
                desc.size,
                1,
                1,
            );
            // D3D11 shared surfaces have COMMON state in D3D12. Initialization
            // and every subsequent write are ordered before use by producer.
            let imported = device.create_texture_from_hal::<wgpu::hal::api::Dx12>(
                hal,
                &desc,
                wgpu::TextureUses::PRESENT,
            );
            Ok(Arc::new(Self {
                texture,
                imported,
                producer: Fence::new(native)?,
                consumer: Fence::new(native)?,
            }))
        }
    }
}

pub(super) struct Published {
    pub key: u64,
    pub value: u64,
    pub slot: Arc<Slot>,
}
pub(super) struct Recycled {
    pub slot: Arc<Slot>,
    pub consumed: u64,
    pub produced: u64,
}

impl Published {
    fn encode_copy(&self, encoder: &mut wgpu::CommandEncoder, output: &wgpu::Texture) {
        encoder.copy_texture_to_texture(
            self.slot.imported.as_image_copy(),
            output.as_image_copy(),
            output.size(),
        );
        // Return the shared source to COMMON before D3D11 may write it again.
        encoder.transition_resources(
            std::iter::empty(),
            std::iter::once(wgpu::TextureTransition {
                texture: &self.slot.imported,
                selector: None,
                state: wgpu::TextureUses::PRESENT,
            }),
        );
    }

    pub fn recycle(self, copied: bool, sender: &SyncSender<Recycled>) {
        // There are only three slots and the recycle channel can hold all of
        // them. A disconnected receiver no longer needs its slot back.
        let _ = sender.try_send(Recycled {
            slot: self.slot,
            produced: self.value,
            consumed: if copied { self.value } else { 0 },
        });
    }
}

struct Copy {
    frame: Published,
    output: wgpu::Texture,
    recycled: SyncSender<Recycled>,
}

pub(super) struct TransferBatch {
    device: wgpu::Device,
    queue: wgpu::Queue,
    active: bool,
    copies: Vec<Copy>,
}

impl TransferBatch {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        Self {
            device: device.clone(),
            queue: queue.clone(),
            active: false,
            copies: Vec::new(),
        }
    }

    pub fn begin_frame(&mut self) {
        debug_assert!(!self.active);
        self.active = true;
    }

    pub fn end_frame(&mut self) -> PortResult<()> {
        self.active = false;
        self.submit()
    }

    pub fn copy(
        &mut self,
        frame: Published,
        output: &wgpu::Texture,
        recycled: &SyncSender<Recycled>,
    ) -> PortResult<()> {
        self.copies.push(Copy {
            frame,
            output: output.clone(),
            recycled: recycled.clone(),
        });
        if self.active { Ok(()) } else { self.submit() }
    }

    fn submit(&mut self) -> PortResult<()> {
        if self.copies.is_empty() {
            return Ok(());
        }
        let mut copies = self.copies.drain(..);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("GPU movie transfers"),
            });
        for copy in copies.as_slice() {
            copy.frame.encode_copy(&mut encoder, &copy.output);
        }
        // The render thread owns submissions to this queue. Stage all waits and
        // signals for this submit and release the HAL guard before submitting.
        {
            // SAFETY: Each producer fence is already queued by D3D11. The
            // consumer fences are signalled after the shared textures return to COMMON.
            let raw = unsafe { self.queue.as_hal::<wgpu::hal::api::Dx12>() }
                .ok_or("Expected DX12 queue")?;
            for copy in copies.as_slice() {
                let frame = &copy.frame;
                raw.add_wait_fence(frame.slot.producer.d12.clone(), frame.value);
                raw.add_signal_fence(frame.slot.consumer.d12.clone(), frame.value);
            }
        }
        self.queue.submit([encoder.finish()]);
        for copy in &mut copies {
            copy.frame.recycle(true, &copy.recycled);
        }
        Ok(())
    }
}

fn descriptor<'a>(
    extent: Extent,
    label: &'a str,
    usage: wgpu::TextureUsages,
) -> wgpu::TextureDescriptor<'a> {
    wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: extent.width,
            height: extent.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Bgra8Unorm,
        usage,
        view_formats: &[],
    }
}

pub(super) fn output(device: &wgpu::Device, queue: &wgpu::Queue, extent: Extent) -> wgpu::Texture {
    let output = device.create_texture(&descriptor(
        extent,
        "GPU movie output",
        wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::RENDER_ATTACHMENT,
    ));
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let _clear = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &output.create_view(&Default::default()),
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
    output
}
