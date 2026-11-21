use super::*;
use objc2::rc::Retained;
use std::ffi::{CStr, CString, c_char, c_void};
use vjay_application::preferences::VideoSource;

type Handle = *mut c_void;
unsafe extern "C" {
    fn vjay_capture_device_count() -> i32;
    fn vjay_capture_device_info(
        index: i32,
        uid: *mut c_char,
        name: *mut c_char,
        capacity: usize,
    ) -> bool;
    fn vjay_capture_create(
        device: Handle,
        queue: Handle,
        target: Handle,
        uid: *const c_char,
        width: i32,
        height: i32,
        fps: i32,
        synthetic: bool,
    ) -> Handle;
    fn vjay_capture_encode(handle: Handle, event: Handle, value: u64) -> bool;
    fn vjay_capture_status(handle: Handle) -> i32;
    fn vjay_capture_destroy(handle: Handle);
}
#[derive(Default)]
pub struct CameraFactory;
impl VideoIoFactory for CameraFactory {
    fn sources(&self) -> Vec<String> {
        self.source_info().into_iter().map(|s| s.id).collect()
    }
    fn source_info(&self) -> Vec<VideoSource> {
        let count = unsafe { vjay_capture_device_count() }.clamp(0, 64);
        (0..count)
            .filter_map(|i| {
                let (mut uid, mut name) = ([0 as c_char; 1024], [0 as c_char; 1024]);
                if !unsafe {
                    vjay_capture_device_info(i, uid.as_mut_ptr(), name.as_mut_ptr(), 1024)
                } {
                    return None;
                }
                Some(VideoSource {
                    id: unsafe { CStr::from_ptr(uid.as_ptr()) }
                        .to_string_lossy()
                        .into_owned(),
                    name: unsafe { CStr::from_ptr(name.as_ptr()) }
                        .to_string_lossy()
                        .into_owned(),
                })
            })
            .collect()
    }
    fn receiver(
        &self,
        r: &StreamRequest,
        d: &wgpu::Device,
        q: &wgpu::Queue,
        e: Extent,
    ) -> PortResult<Box<dyn VideoReceiver>> {
        Receiver::new(r, d, q, e, false).map(|r| Box::new(r) as Box<dyn VideoReceiver>)
    }
    fn sender(
        &self,
        _: &StreamRequest,
        _: &wgpu::Device,
        _: &wgpu::Queue,
        _: Extent,
    ) -> PortResult<Box<dyn VideoSender>> {
        Err("Media Device Input is a capture source".into())
    }
}
struct Receiver {
    handle: Handle,
    texture: wgpu::Texture,
    fence: super::metal::Fence,
}
impl Receiver {
    fn new(
        r: &StreamRequest,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        extent: Extent,
        synthetic: bool,
    ) -> PortResult<Self> {
        if r.name.is_empty() && !synthetic {
            return Err("Choose a camera or capture device before enabling capture".into());
        }
        let (w, h) = r
            .options
            .get("capture.size")
            .map_or("1280x720", String::as_str)
            .split_once('x')
            .ok_or("Invalid capture size")?;
        let size = Extent::new(
            w.parse().map_err(|_| "Invalid width")?,
            h.parse().map_err(|_| "Invalid height")?,
        )?;
        let fps = r
            .options
            .get("capture.fps")
            .map_or("30", String::as_str)
            .parse::<i32>()
            .map_err(|_| "Invalid capture frame rate")?;
        if !(1..=120).contains(&fps) {
            return Err("Unsupported capture frame rate".into());
        }
        let name = CString::new(r.name.as_str()).map_err(|_| "Invalid capture identifier")?;
        let fence = super::metal::Fence::new(device)?;
        let texture = super::texture(device, extent);
        super::metal::initialize_texture(device, queue, &texture);
        // Native references are retained by the ARC capture object. Rust retains the
        // persistent wgpu texture; frame buffers live until their GPU copy completes.
        let native_device =
            unsafe { device.as_hal::<wgpu::hal::api::Metal>() }.ok_or("Capture requires Metal")?;
        let native_queue = unsafe { queue.as_hal::<wgpu::hal::api::Metal>() }
            .ok_or("Capture requires a Metal queue")?;
        let native_texture = unsafe { texture.as_hal::<wgpu::hal::api::Metal>() }
            .ok_or("Capture requires a Metal texture")?;
        let handle = unsafe {
            vjay_capture_create(
                Retained::as_ptr(native_device.raw_device())
                    .cast_mut()
                    .cast(),
                native_queue.as_raw() as *const _ as Handle,
                native_texture.raw_handle() as *const _ as Handle,
                name.as_ptr(),
                size.width as i32,
                size.height as i32,
                fps,
                synthetic,
            )
        };
        drop(native_texture);
        drop(native_queue);
        drop(native_device);
        if handle.is_null() {
            return Err("Cannot initialize the Metal camera capture runtime".into());
        }
        Ok(Self {
            handle,
            texture,
            fence,
        })
    }
}
impl VideoReceiver for Receiver {
    fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }
    fn status(&self) -> Option<&'static str> {
        match unsafe { vjay_capture_status(self.handle) } {
            7 => None,
            1 => Some("Allow camera access in the system permission dialog"),
            2 => Some("Camera is unavailable or disconnected"),
            3 => Some(
                "Camera access denied. Allow VJAY in System Settings → Privacy & Security → Camera",
            ),
            4 => Some("Camera does not support this capture size / frame rate"),
            5 => Some("Cannot start camera. It may be in use by another application"),
            9 => Some("Camera capture was interrupted. Toggle Capture off/on to retry"),
            _ => Some("Waiting for camera frames"),
        }
    }
    fn update(
        &mut self,
        _: &wgpu::Device,
        queue: &wgpu::Queue,
        _: &mut wgpu::CommandEncoder,
    ) -> PortResult<()> {
        self.fence.value += 1;
        if unsafe {
            vjay_capture_encode(
                self.handle,
                Retained::as_ptr(&self.fence.event).cast_mut().cast(),
                self.fence.value,
            )
        } {
            self.fence.wait_in_wgpu(queue)?;
        }
        Ok(())
    }
}
impl Drop for Receiver {
    fn drop(&mut self) {
        unsafe { vjay_capture_destroy(self.handle) }
    }
}

#[cfg(test)]
mod tests;
