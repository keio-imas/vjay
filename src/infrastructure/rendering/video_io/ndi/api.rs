use libloading::Library;
use std::{
    ffi::{CStr, CString, c_char, c_void},
    path::Path,
    sync::Arc,
};
use vjay_application::ports::PortResult;
pub type Handle = *mut c_void;
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Source {
    pub name: *const c_char,
    pub address: *const c_char,
}
#[repr(C)]
pub struct Finder {
    pub local: bool,
    pub groups: *const c_char,
    pub ips: *const c_char,
}
#[repr(C)]
pub struct Receive {
    pub source: Source,
    pub color: i32,
    pub bandwidth: i32,
    pub fields: bool,
    pub name: *const c_char,
}
#[repr(C)]
pub struct SendSettings {
    pub name: *const c_char,
    pub groups: *const c_char,
    pub video_clock: bool,
    pub audio_clock: bool,
}
#[repr(C)]
pub struct Video {
    pub width: i32,
    pub height: i32,
    pub fourcc: u32,
    pub rate_n: i32,
    pub rate_d: i32,
    pub aspect: f32,
    pub format: i32,
    pub timecode: i64,
    pub data: *mut u8,
    pub stride: i32,
    pub metadata: *const c_char,
    pub timestamp: i64,
}
impl Default for Video {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            fourcc: u32::from_le_bytes(*b"RGBA"),
            rate_n: 60,
            rate_d: 1,
            aspect: 1.0,
            format: 1,
            timecode: i64::MAX,
            data: std::ptr::null_mut(),
            stride: 0,
            metadata: std::ptr::null(),
            timestamp: 0,
        }
    }
}
#[repr(C)]
pub struct Metadata {
    pub length: i32,
    pub timecode: i64,
    pub data: *const c_char,
}
pub struct Api {
    _library: Library,
    initialized: bool,
    destroy: unsafe extern "C" fn(),
    pub find_create: unsafe extern "C" fn(*const Finder) -> Handle,
    pub find_destroy: unsafe extern "C" fn(Handle),
    pub find_wait: unsafe extern "C" fn(Handle, u32) -> bool,
    pub find_sources: unsafe extern "C" fn(Handle, *mut u32) -> *const Source,
    pub recv_create: unsafe extern "C" fn(*const Receive) -> Handle,
    pub recv_destroy: unsafe extern "C" fn(Handle),
    pub recv_capture:
        unsafe extern "C" fn(Handle, *mut Video, *mut c_void, *mut c_void, u32) -> i32,
    pub recv_metadata: unsafe extern "C" fn(Handle, *const Metadata) -> bool,
    pub recv_free: unsafe extern "C" fn(Handle, *const Video),
    pub send_create: unsafe extern "C" fn(*const SendSettings) -> Handle,
    pub send_destroy: unsafe extern "C" fn(Handle),
    pub send_video: unsafe extern "C" fn(Handle, *const Video),
    pub send_name: unsafe extern "C" fn(Handle) -> *const Source,
}
impl Api {
    pub fn load(path: &Path) -> PortResult<Arc<Self>> {
        static APIS: std::sync::OnceLock<
            std::sync::Mutex<std::collections::HashMap<std::path::PathBuf, std::sync::Weak<Api>>>,
        > = std::sync::OnceLock::new();
        let mut apis = APIS
            .get_or_init(Default::default)
            .lock()
            .map_err(|_| "NDI library cache unavailable")?;
        let key = path.canonicalize().unwrap_or_else(|_| path.to_owned());
        if let Some(api) = apis.get(&key).and_then(std::sync::Weak::upgrade) {
            return Ok(api);
        }
        let api = Self::open(path)?;
        apis.insert(key, Arc::downgrade(&api));
        Ok(api)
    }
    fn open(path: &Path) -> PortResult<Arc<Self>> {
        unsafe {
            let lib = Library::new(path).map_err(|e| format!("NDI Runtime unavailable: {e}"))?;
            let init: unsafe extern "C" fn() -> bool = *lib
                .get::<unsafe extern "C" fn() -> bool>(b"NDIlib_initialize\0")
                .map_err(|e| e.to_string())?;
            macro_rules! sym {
                ($s:literal) => {
                    *lib.get(concat!($s, "\0").as_bytes())
                        .map_err(|e| e.to_string())?
                };
            }
            let mut api = Self {
                initialized: false,
                destroy: sym!("NDIlib_destroy"),
                find_create: sym!("NDIlib_find_create_v2"),
                find_destroy: sym!("NDIlib_find_destroy"),
                find_wait: sym!("NDIlib_find_wait_for_sources"),
                find_sources: sym!("NDIlib_find_get_current_sources"),
                recv_create: sym!("NDIlib_recv_create_v3"),
                recv_destroy: sym!("NDIlib_recv_destroy"),
                recv_capture: sym!("NDIlib_recv_capture_v3"),
                recv_metadata: sym!("NDIlib_recv_send_metadata"),
                recv_free: sym!("NDIlib_recv_free_video_v2"),
                send_create: sym!("NDIlib_send_create"),
                send_destroy: sym!("NDIlib_send_destroy"),
                send_video: sym!("NDIlib_send_send_video_async_v2"),
                send_name: sym!("NDIlib_send_get_source_name"),
                _library: lib,
            };
            if !init() {
                return Err("NDI initialization failed".into());
            }
            api.initialized = true;
            Ok(Arc::new(api))
        }
    }
}
pub fn string(s: &str) -> PortResult<CString> {
    CString::new(s).map_err(|_| "NDI names cannot contain NUL".into())
}
pub unsafe fn name(source: *const Source) -> Option<String> {
    if source.is_null() {
        None
    } else {
        let name = unsafe { (*source).name };
        if name.is_null() {
            None
        } else {
            Some(
                unsafe { CStr::from_ptr(name) }
                    .to_string_lossy()
                    .into_owned(),
            )
        }
    }
}

impl Drop for Api {
    fn drop(&mut self) {
        if self.initialized {
            unsafe {
                (self.destroy)();
            }
        }
    }
}
