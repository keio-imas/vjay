//! GPUI frontend and platform window/presentation adapters.
pub mod clipboard;
#[doc(hidden)]
pub mod demo;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(test)]
mod open_node_tests;
pub mod platform;
pub(crate) mod ui;
#[cfg(target_os = "windows")]
pub mod windows;
