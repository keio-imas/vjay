use std::ffi::{c_char, c_void};
pub(super) type Raw = *mut c_void;
unsafe extern "C" {
    pub fn vjay_movie_context_create(
        device: Raw,
        queue: Raw,
        error: *mut c_char,
        capacity: usize,
    ) -> Raw;
    pub fn vjay_movie_context_destroy(context: Raw);
    pub fn vjay_movie_create(context: Raw, texture: Raw) -> Raw;
    pub fn vjay_movie_open(
        handle: Raw,
        path: *const c_char,
        error: *mut c_char,
        capacity: usize,
    ) -> bool;
    pub fn vjay_movie_request(handle: Raw, key: u64);
    pub fn vjay_movie_decode(handle: Raw, key: u64, error: *mut c_char, capacity: usize) -> i32;
    pub fn vjay_movie_encode(handle: Raw, key: u64, event: Raw, value: u64) -> bool;
    pub fn vjay_movie_duration(handle: Raw) -> f64;
    pub fn vjay_movie_fps(handle: Raw) -> f64;
    pub fn vjay_movie_stat(handle: Raw, field: u32) -> u64;
    pub fn vjay_movie_destroy(handle: Raw);
}
pub(super) struct Handle(pub Raw);
// The native player owns its decoder on one worker. Its cache is mutex-protected;
// request/metadata fields are atomic and GPU commands retain their frame/player.
unsafe impl Send for Handle {}
unsafe impl Sync for Handle {}
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe { vjay_movie_destroy(self.0) }
    }
}
pub(super) struct Context(pub Raw);
// CVMetalTextureCache and immutable Metal pipelines support concurrent clients.
unsafe impl Send for Context {}
unsafe impl Sync for Context {}
impl Drop for Context {
    fn drop(&mut self) {
        unsafe { vjay_movie_context_destroy(self.0) }
    }
}
pub(super) fn message(error: &[c_char]) -> String {
    unsafe { std::ffi::CStr::from_ptr(error.as_ptr()) }
        .to_string_lossy()
        .into_owned()
}
