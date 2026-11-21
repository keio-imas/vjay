//! Backend-independent graph and node contracts. No window or GPU types.
pub mod builtins;
pub mod capabilities;
pub mod choices;
mod control_nodes;
mod effects;
pub mod execution;
mod extra_nodes;
pub mod graph;
pub mod node;
pub mod port;
pub mod registry;
pub mod settings;
mod sources;
mod spatial;
pub mod tempo;

pub mod schema;
pub mod uniform;

mod fragment;

mod visual_nodes;

mod annotations;
mod lip_sync;
mod math_nodes;
mod noise;
mod spatial3d;
mod text_node;
mod web_video;

mod loadable_video;

mod text_value;

mod file_nodes;

mod video_io_nodes;

mod media_device;

pub mod transport;

pub mod composition;

mod seek_control;

mod deck_rate;
