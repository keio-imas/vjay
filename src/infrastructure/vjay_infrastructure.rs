//! Native platform, persistence, and WebGPU adapters.
pub mod audio;
pub mod control_io;
pub mod gpu;
#[cfg(target_os = "macos")]
pub mod macos;
pub mod media;
#[cfg(test)]
mod open_node_tests;
pub mod platform;
pub mod storage;
mod ui;

#[doc(hidden)]
pub mod demo;

pub mod services;

pub mod images;
pub mod text_images;
pub mod web_media;

pub mod clipboard;

pub mod file_collection;

pub mod video_io;
