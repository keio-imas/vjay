//! Use cases, execution plans and adapter ports. Concrete adapters are injected by main.
pub mod engine;
pub mod ports;
pub mod preferences;
pub mod project;
pub mod session;
// Retain the public API while keeping implementation files grouped by responsibility.
pub use engine::{asset_sources, audio, compiler, composition, media_timeline, services, signals};
pub(crate) use engine::{bindings, file_media, port_migrations, transport_migrations};
pub use ports::{control_io, media};
pub use project::{clipboard, value_display, workspace};
