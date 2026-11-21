pub mod control_io;
pub mod media;
use crate::{compiler::RenderPlan, project::ProjectDocument, session::Session};
use std::path::Path;

pub type PortResult<T> = Result<T, String>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Extent {
    pub width: u32,
    pub height: u32,
}

impl Extent {
    pub fn new(width: u32, height: u32) -> PortResult<Self> {
        if width == 0 || height == 0 || width > 4096 || height > 4096 {
            return Err("Render dimensions must be between 1 and 4096".into());
        }
        Ok(Self { width, height })
    }
}

#[derive(Clone, Copy)]
pub struct FrameContext {
    pub seconds: f32,
    pub timeline_seconds: f64,
    pub extent: Extent,
    pub delivery: crate::media::FrameDelivery,
}

/// An opaque adapter token; application code never interprets it.
#[derive(Clone, Copy)]
pub struct PreviewToken(pub u64);

pub struct CapturedFrame {
    pub extent: Extent,
    pub rgba: Vec<u8>,
}

pub trait RenderBackend {
    /// Explicit profiling/export fence; ordinary playback never waits for the GPU here.
    fn wait_idle(&self) -> PortResult<()> {
        Ok(())
    }
    fn media_metrics(&self) -> crate::media::MediaMetrics {
        Default::default()
    }
    fn set_vsync(&mut self, _: bool) -> PortResult<()> {
        Ok(())
    }
    fn video_source_info(&self, provider: &str) -> Vec<crate::preferences::VideoSource> {
        self.video_sources(provider)
            .into_iter()
            .map(|name| crate::preferences::VideoSource {
                id: name.clone(),
                name,
            })
            .collect()
    }
    fn video_sources(&self, _provider: &str) -> Vec<String> {
        vec![]
    }
    fn capture_output(&mut self, _node: vjay_domain::graph::NodeId) -> PortResult<CapturedFrame> {
        Err("Output capture unavailable".into())
    }
    fn node_error(&self, _node: vjay_domain::graph::NodeId) -> Option<&str> {
        None
    }
    fn media_duration(&self, _node: vjay_domain::graph::NodeId) -> Option<f64> {
        None
    }
    fn media_ready(&self, _node: vjay_domain::graph::NodeId) -> bool {
        true
    }
    fn media_status(&self) -> Option<&str> {
        None
    }
    fn validate_shader(&self, _source: &str) -> PortResult<()> {
        Ok(())
    }
    fn set_assets(&mut self, _assets: &[crate::media::MediaAsset]) {}
    /// Expensive allocation/compilation path, called only on topology or size edits.
    fn prepare(&mut self, plan: &RenderPlan, extent: Extent) -> PortResult<()>;
    /// Steady-state path: no graph traversal, pipeline creation, or texture allocation.
    fn render(&mut self, plan: &RenderPlan, frame: FrameContext) -> PortResult<()>;
    fn preview(&self) -> Option<PreviewToken>;
    fn capture(&mut self) -> PortResult<CapturedFrame>;
    fn description(&self) -> &str;
}

pub trait Clock {
    fn seconds(&self) -> f64;
}

/// Abstract factory for a compatible rendering runtime and its time source.
pub trait RuntimeFactory {
    fn settings_store(&self) -> std::sync::Arc<dyn crate::preferences::SettingsStore> {
        std::sync::Arc::new(crate::preferences::MemorySettings::default())
    }
    fn create_asset_sources(&self) -> std::sync::Arc<dyn crate::asset_sources::AssetSourceFactory> {
        std::sync::Arc::new(crate::asset_sources::DisabledAssetSources)
    }
    fn create_renderer(&self) -> PortResult<Box<dyn RenderBackend>>;
    fn create_clock(&self) -> Box<dyn Clock>;
    fn create_control_io(&self) -> Box<dyn crate::control_io::ControlIo> {
        Box::new(crate::control_io::DisabledControlIo)
    }
    fn create_audio(&self) -> Box<dyn crate::audio::AudioBackend> {
        Box::new(crate::audio::DisabledAudio)
    }
}

pub trait ProjectStore {
    fn load_from(&self, _path: &Path) -> PortResult<ProjectDocument> {
        Err("Opening another project is unavailable".into())
    }
    fn save_to(&self, _path: &Path, _document: &ProjectDocument) -> PortResult<()> {
        Err("Save As is unavailable".into())
    }
    fn set_location(&mut self, _path: std::path::PathBuf) -> PortResult<()> {
        Err("Changing project location is unavailable".into())
    }
    fn load(&self) -> PortResult<ProjectDocument>;
    fn save(&self, document: &ProjectDocument) -> PortResult<()>;
    fn location(&self) -> &Path;
}

/// Platform abstraction: the use cases own no native window or event-loop API.
pub trait Platform {
    fn run(self: Box<Self>, create: SessionFactory) -> PortResult<()>;
}

pub type SessionFactory = Box<dyn FnOnce(&dyn RuntimeFactory) -> PortResult<Session>>;
