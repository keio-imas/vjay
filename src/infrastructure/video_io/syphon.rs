use super::*;
use libloading::Library;
use objc2::{
    msg_send,
    rc::Retained,
    runtime::{AnyClass, AnyObject},
};
use objc2_foundation::{NSData, NSPoint, NSRect, NSSize, NSString};
use std::path::Path;
mod connection;
use super::metal as sync;
type LocalSources = Arc<std::sync::Mutex<Vec<(String, String, Vec<u8>)>>>;
pub struct SyphonFactory {
    library: Arc<Library>,
    local: LocalSources,
}
fn class(name: &'static std::ffi::CStr) -> PortResult<&'static AnyClass> {
    AnyClass::get(name)
        .ok_or_else(|| format!("Syphon class {} unavailable", name.to_string_lossy()))
}
unsafe fn text(object: &AnyObject, key: &str) -> String {
    let key = NSString::from_str(key);
    let value: Option<Retained<NSString>> = unsafe { msg_send![object,objectForKey:&*key] };
    value.map(|s| s.to_string()).unwrap_or_default()
}
fn servers() -> PortResult<Vec<(String, Retained<AnyObject>)>> {
    unsafe {
        let directory: Retained<AnyObject> =
            msg_send![class(c"SyphonServerDirectory")?, sharedDirectory];
        let servers: Retained<AnyObject> = msg_send![&*directory, servers];
        let count: usize = msg_send![&*servers, count];
        let mut list = Vec::new();
        for i in 0..count.min(4096) {
            let description: Retained<AnyObject> = msg_send![&*servers,objectAtIndex:i];
            let name = text(&description, "SyphonServerDescriptionNameKey");
            let app = text(&description, "SyphonServerDescriptionAppNameKey");
            list.push((format!("{app} / {name}"), description));
        }
        Ok(list)
    }
}
fn native_device(device: &wgpu::Device) -> PortResult<Retained<AnyObject>> {
    let hal = unsafe { device.as_hal::<wgpu::hal::api::Metal>() }.ok_or("Syphon requires Metal")?;
    unsafe {
        Retained::retain(
            Retained::as_ptr(hal.raw_device())
                .cast_mut()
                .cast::<AnyObject>(),
        )
    }
    .ok_or_else(|| "Metal device unavailable".into())
}
fn native_queue(queue: &wgpu::Queue) -> PortResult<Retained<AnyObject>> {
    let hal = unsafe { queue.as_hal::<wgpu::hal::api::Metal>() }
        .ok_or("Syphon requires a Metal queue")?;
    unsafe { Retained::retain((hal.as_raw() as *const _ as *mut AnyObject).cast()) }
        .ok_or_else(|| "Metal queue unavailable".into())
}
fn native_texture(texture: &wgpu::Texture) -> PortResult<Retained<AnyObject>> {
    let hal = unsafe { texture.as_hal::<wgpu::hal::api::Metal>() }
        .ok_or("Syphon requires a Metal texture")?;
    unsafe { Retained::retain(hal.raw_handle() as *const _ as *mut AnyObject) }
        .ok_or_else(|| "Metal texture unavailable".into())
}
impl SyphonFactory {
    pub fn new(path: &Path) -> PortResult<Self> {
        let path = if path.is_dir() {
            path.join("Syphon")
        } else {
            path.to_path_buf()
        };
        let library = unsafe { Library::new(path) }
            .map_err(|e| format!("Syphon framework unavailable: {e}"))?;
        class(c"SyphonMetalServer")?;
        Ok(Self {
            library: Arc::new(library),
            local: Arc::new(Default::default()),
        })
    }
}
impl VideoIoFactory for SyphonFactory {
    fn sources(&self) -> Vec<String> {
        let mut names = servers()
            .map(|v| v.into_iter().map(|(n, _)| n).collect::<Vec<_>>())
            .unwrap_or_default();
        if let Ok(local) = self.local.lock() {
            names.extend(local.iter().map(|(n, _, _)| n.clone()));
        }
        names.sort();
        names.dedup();
        names
    }
    fn receiver(
        &self,
        r: &StreamRequest,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        extent: Extent,
    ) -> PortResult<Box<dyn VideoReceiver>> {
        unsafe {
            let native = native_device(device)?;
            let client =
                connection::Client::new(r.name.clone(), self.local.clone(), native.clone())?;
            let _mps=Library::new("/System/Library/Frameworks/MetalPerformanceShaders.framework/MetalPerformanceShaders").map_err(|e|e.to_string())?;
            let allocated: objc2::rc::Allocated<AnyObject> =
                msg_send![class(c"MPSImageBilinearScale")?, alloc];
            let scale: Retained<AnyObject> = msg_send![allocated,initWithDevice:&*native];
            let texture = super::texture(device, extent);
            sync::initialize_texture(device, queue, &texture);
            let target = native_texture(&texture)?;
            Ok(Box::new(Receiver {
                client,
                scale,
                texture,
                target,
                queue: native_queue(queue)?,
                fence: sync::Fence::new(device)?,
                _library: self.library.clone(),
                _mps,
            }))
        }
    }
    fn sender(
        &self,
        r: &StreamRequest,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _: Extent,
    ) -> PortResult<Box<dyn VideoSender>> {
        unsafe {
            let native = native_device(device)?;
            let allocated: objc2::rc::Allocated<AnyObject> =
                msg_send![class(c"SyphonMetalServer")?, alloc];
            let name = NSString::from_str(&r.name);
            let server: Option<Retained<AnyObject>> = msg_send![allocated,initWithName:&*name,device:&*native,options:std::ptr::null::<AnyObject>()];
            let server = server.ok_or("Cannot create Syphon sender")?;
            let description: Retained<AnyObject> = msg_send![&*server, serverDescription];
            let app = text(&description, "SyphonServerDescriptionAppNameKey");
            let title = text(&description, "SyphonServerDescriptionNameKey");
            let uuid = text(&description, "SyphonServerDescriptionUUIDKey");
            let full_name = format!("{app} / {title}");
            let data: Option<Retained<NSData>> = msg_send![class(c"NSPropertyListSerialization")?,dataWithPropertyList:&*description,format:200usize,options:0usize,error:std::ptr::null_mut::<*mut AnyObject>()];
            if let Some(data) = data
                && let Ok(mut local) = self.local.lock()
            {
                local.push((full_name.clone(), uuid.clone(), data.to_vec()));
            }
            Ok(Box::new(Sender {
                full_name,
                uuid,
                local: self.local.clone(),
                server,
                queue: native_queue(queue)?,
                wgpu_queue: queue.clone(),
                fence: sync::Fence::new(device)?,
                image: None,
                _library: self.library.clone(),
            }))
        }
    }
}
struct Receiver {
    fence: sync::Fence,
    client: connection::Client,
    scale: Retained<AnyObject>,
    texture: wgpu::Texture,
    target: Retained<AnyObject>,
    queue: Retained<AnyObject>,
    _library: Arc<Library>,
    _mps: Library,
}
impl VideoReceiver for Receiver {
    fn status(&self) -> Option<&'static str> {
        self.client.status()
    }
    fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }
    fn update(
        &mut self,
        _: &wgpu::Device,
        queue: &wgpu::Queue,
        _: &mut wgpu::CommandEncoder,
    ) -> PortResult<()> {
        unsafe {
            let image = self.client.frame()?;
            if let Some(image) = image {
                let command: Retained<AnyObject> = msg_send![&*self.queue, commandBuffer];
                let _: () = msg_send![&*self.scale,encodeToCommandBuffer:&*command,sourceTexture:&*image,destinationTexture:&*self.target];
                self.fence.value += 1;
                let _: () = msg_send![&*command,encodeSignalEvent:&*self.fence.event,value:self.fence.value];
                let _: () = msg_send![&*command, commit];
                self.fence.wait_in_wgpu(queue)?;
            }
        }
        Ok(())
    }
}
struct Sender {
    full_name: String,
    uuid: String,
    local: LocalSources,
    wgpu_queue: wgpu::Queue,
    fence: sync::Fence,
    server: Retained<AnyObject>,
    queue: Retained<AnyObject>,
    image: Option<(Retained<AnyObject>, u32, u32)>,
    _library: Arc<Library>,
}
impl VideoSender for Sender {
    fn source_name(&self) -> Option<&str> {
        Some(&self.full_name)
    }
    fn encode(&mut self, _: &mut wgpu::CommandEncoder, image: &wgpu::Texture) -> PortResult<()> {
        self.image = Some((native_texture(image)?, image.width(), image.height()));
        self.fence.signal_from_wgpu(&self.wgpu_queue)?;
        Ok(())
    }
    fn submitted(&mut self, _: &wgpu::Device, _: &wgpu::Queue) -> PortResult<()> {
        if let Some((image, width, height)) = &self.image {
            unsafe {
                let command: Retained<AnyObject> = msg_send![&*self.queue, commandBuffer];
                let _: () = msg_send![&*command,encodeWaitForEvent:&*self.fence.event,value:self.fence.value];
                let region = NSRect::new(
                    NSPoint::new(0.0, 0.0),
                    NSSize::new(*width as f64, *height as f64),
                );
                let _: () = msg_send![&*self.server,publishFrameTexture:&**image,onCommandBuffer:&*command,imageRegion:region,flipped:false];
                let _: () = msg_send![&*command, commit];
            }
        }
        Ok(())
    }
}
impl Drop for Sender {
    fn drop(&mut self) {
        if let Ok(mut local) = self.local.lock() {
            local.retain(|(_, id, _)| id != &self.uuid);
        }
        unsafe {
            let _: () = msg_send![&*self.server, stop];
        }
    }
}
