use super::*;
use std::{
    ffi::{CStr, CString, c_char, c_void},
    path::Path,
};
use windows::core::Interface;
type Handle = *mut c_void;
struct Api {
    create: unsafe extern "C" fn(Handle, Handle, *const c_char, bool) -> Handle,
    destroy: unsafe extern "C" fn(Handle),
    send: unsafe extern "C" fn(Handle, Handle) -> bool,
    probe: unsafe extern "C" fn(Handle, *mut u32, *mut u32, *mut u32) -> bool,
    receive: unsafe extern "C" fn(Handle, Handle) -> bool,
    source: unsafe extern "C" fn(i32, *mut c_char, i32) -> i32,
    _library: libloading::Library,
}
pub struct SpoutFactory(Arc<Api>);
impl SpoutFactory {
    pub fn new(path: &Path) -> PortResult<Self> {
        unsafe {
            let library = libloading::Library::new(path)
                .map_err(|e| format!("Spout bridge unavailable: {e}"))?;
            macro_rules! load {
                ($name:literal) => {
                    *library
                        .get(concat!($name, "\0").as_bytes())
                        .map_err(|e| e.to_string())?
                };
            }
            Ok(Self(Arc::new(Api {
                create: load!("vjay_spout_create"),
                destroy: load!("vjay_spout_destroy"),
                send: load!("vjay_spout_send"),
                probe: load!("vjay_spout_probe"),
                receive: load!("vjay_spout_receive"),
                source: load!("vjay_spout_source"),
                _library: library,
            })))
        }
    }
}
struct Endpoint {
    handle: Handle,
    api: Arc<Api>,
}
impl Endpoint {
    fn new(
        api: Arc<Api>,
        name: &str,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        sender: bool,
    ) -> PortResult<Self> {
        let device = unsafe { device.as_hal::<wgpu::hal::api::Dx12>() }
            .ok_or("Spout requires the DX12 backend")?;
        let queue = unsafe { queue.as_hal::<wgpu::hal::api::Dx12>() }
            .ok_or("Spout requires a DX12 queue")?;
        let name = CString::new(name).map_err(|_| "Invalid Spout name")?;
        let handle = unsafe {
            (api.create)(
                device.raw_device().as_raw(),
                queue.as_raw().as_raw(),
                name.as_ptr(),
                sender,
            )
        };
        if handle.is_null() {
            return Err("Cannot create Spout Direct3D12 endpoint".into());
        }
        Ok(Self { handle, api })
    }
}
impl Drop for Endpoint {
    fn drop(&mut self) {
        unsafe {
            (self.api.destroy)(self.handle);
        }
    }
}
fn raw(texture: &wgpu::Texture) -> PortResult<Handle> {
    let texture =
        unsafe { texture.as_hal::<wgpu::hal::api::Dx12>() }.ok_or("Expected DX12 texture")?;
    Ok(unsafe { texture.raw_resource() }.as_raw())
}
fn transition(
    encoder: &mut wgpu::CommandEncoder,
    texture: &wgpu::Texture,
    state: wgpu::TextureUses,
) {
    encoder.transition_resources(
        std::iter::empty(),
        std::iter::once(wgpu::TextureTransition {
            texture,
            selector: None,
            state,
        }),
    );
}
impl VideoIoFactory for SpoutFactory {
    fn sources(&self) -> Vec<String> {
        let count = unsafe { (self.0.source)(-1, std::ptr::null_mut(), 0) }.clamp(0, 4096);
        (0..count)
            .filter_map(|i| {
                let mut bytes = [0i8; 256];
                unsafe {
                    (self.0.source)(i, bytes.as_mut_ptr(), 256);
                }
                bytes[255] = 0;
                let name = unsafe { CStr::from_ptr(bytes.as_ptr()) }
                    .to_string_lossy()
                    .into_owned();
                (!name.is_empty()).then_some(name)
            })
            .collect()
    }
    fn receiver(
        &self,
        r: &StreamRequest,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        extent: Extent,
    ) -> PortResult<Box<dyn VideoReceiver>> {
        if r.name.is_empty() {
            return Err("Choose a Spout source".into());
        }
        let output = super::texture(device, extent);
        let view = output.create_view(&Default::default());
        Ok(Box::new(Receiver {
            endpoint: Endpoint::new(self.0.clone(), &r.name, device, queue, false)?,
            output,
            view,
            input: None,
            blit: crate::gpu::blit::Blit::new(device, wgpu::TextureFormat::Rgba8Unorm),
        }))
    }
    fn sender(
        &self,
        r: &StreamRequest,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _: Extent,
    ) -> PortResult<Box<dyn VideoSender>> {
        Ok(Box::new(Sender {
            endpoint: Endpoint::new(self.0.clone(), &r.name, device, queue, true)?,
            image: None,
        }))
    }
}
struct Sender {
    endpoint: Endpoint,
    image: Option<wgpu::Texture>,
}
impl VideoSender for Sender {
    fn encode(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        image: &wgpu::Texture,
    ) -> PortResult<()> {
        transition(encoder, image, wgpu::TextureUses::COPY_SRC);
        self.image = Some(image.clone());
        Ok(())
    }
    fn submitted(&mut self, _: &wgpu::Device, _: &wgpu::Queue) -> PortResult<()> {
        if let Some(image) = &self.image {
            unsafe {
                (self.endpoint.api.send)(self.endpoint.handle, raw(image)?);
            }
        }
        Ok(())
    }
}
struct Receiver {
    endpoint: Endpoint,
    output: wgpu::Texture,
    view: wgpu::TextureView,
    input: Option<wgpu::Texture>,
    blit: crate::gpu::blit::Blit,
}
impl VideoReceiver for Receiver {
    fn texture(&self) -> &wgpu::Texture {
        &self.output
    }
    fn update(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
    ) -> PortResult<()> {
        let (mut width, mut height, mut format) = (0, 0, 0);
        if !unsafe {
            (self.endpoint.api.probe)(self.endpoint.handle, &mut width, &mut height, &mut format)
        } {
            return Ok(());
        }
        let extent = Extent::new(width, height)?;
        let format = match format {
            28 => wgpu::TextureFormat::Rgba8Unorm,
            87 => wgpu::TextureFormat::Bgra8Unorm,
            _ => return Err("Spout supports 8-bit RGBA/BGRA sources".into()),
        };
        if self
            .input
            .as_ref()
            .is_none_or(|t| t.width() != width || t.height() != height || t.format() != format)
        {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Spout receive texture"),
                size: wgpu::Extent3d {
                    width: extent.width,
                    height: extent.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            self.blit
                .prepare(device, &texture.create_view(&Default::default()));
            self.input = Some(texture);
        }
        let input = self.input.as_ref().expect("received format");
        let mut barrier = device.create_command_encoder(&Default::default());
        transition(&mut barrier, input, wgpu::TextureUses::COPY_DST);
        queue.submit([barrier.finish()]);
        unsafe {
            (self.endpoint.api.receive)(self.endpoint.handle, raw(input)?);
        }
        self.blit.encode(encoder, &self.view);
        Ok(())
    }
}
