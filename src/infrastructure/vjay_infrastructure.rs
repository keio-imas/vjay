//! Desktop, media, integration, persistence and rendering adapters.
pub mod desktop;
pub mod integrations;
pub mod media;
pub mod persistence;
pub mod rendering;
// Public facades keep adapter construction and external integrations source compatible.
#[cfg(target_os = "macos")]
pub use desktop::macos;
pub(crate) use desktop::ui;
pub use desktop::{clipboard, demo, platform};
pub use integrations::{control_io, services};
pub use media::{audio, file_collection, images, text_images, web_media};
pub use persistence::{preferences, projects as storage};
pub use rendering::{gpu, video_io};
