#[cfg(test)]
mod tests;
use crate::{
    compiler::{RenderPlan, compile_with_previews},
    ports::*,
    project::*,
};
use vjay_domain::{
    graph::{Connection, NodeId},
    node::{NodeDescriptor, NodeFactory},
};
mod clipboard;
mod deck;
mod encapsulate;
mod subpatch;
mod subpatch_files;
pub use deck::ClipPosition;
mod controls;
mod editing;
mod media;
mod midi;
mod parameters;
mod preferences;
mod project_io;
mod projects;
pub use project_io::{LoadedProject, OpenRequest, SaveRequest, SavedProject};
mod readout;
mod rendering;
mod settings;
mod sync;
mod uniforms;
pub use midi::MidiActivity;

pub struct Session {
    ancestors: Vec<subpatch::Ancestor>,
    execution: crate::composition::Expanded,
    next_node_id: u64,
    next_asset_id: u64,
    preferences: crate::preferences::Preferences,
    settings_store: std::sync::Arc<dyn crate::preferences::SettingsStore>,
    modified: std::cell::Cell<bool>,
    revision: std::cell::Cell<u64>,
    project_epoch: u64,
    sync_taps: std::collections::HashMap<NodeId, vjay_domain::tempo::TapTempo>,
    untitled: bool,
    file_media: crate::file_media::FileMedia,
    asset_sources: crate::asset_sources::AssetSources,
    services: crate::services::ServicePlan,
    project: Project,
    nodes: Box<dyn NodeFactory>,
    renderer: Box<dyn RenderBackend>,
    clock: Box<dyn Clock>,
    frame_trace: Option<std::sync::Arc<dyn crate::ports::telemetry::FrameTrace>>,
    last_trace_clock: Option<f64>,
    last_render_seconds: f64,
    store: Box<dyn ProjectStore>,
    plan: Option<RenderPlan>,
    extent: Extent,
    dirty: bool,
    prepare: bool,
    valid: bool,
    seconds: f64,
    last_clock: f64,
    pub paused: bool,
    pub error: Option<String>,
    signals: crate::signals::SignalEngine,
    control_io: Box<dyn crate::control_io::ControlIo>,
    pub input_settings: crate::control_io::InputSettings,
    pub input_status: String,
    midi: MidiActivity,
    timeline: crate::media_timeline::MediaTimeline,
    audio_plan: crate::audio::AudioPlan,
    audio: Box<dyn crate::audio::AudioBackend>,
    pub audio_error: Option<String>,
}

impl Session {
    pub fn node_factory(&self) -> &dyn vjay_domain::node::NodeFactory {
        self.nodes.as_ref()
    }

    pub fn new(
        nodes: Box<dyn NodeFactory>,
        runtime: &dyn RuntimeFactory,
        store: Box<dyn ProjectStore>,
        extent: Extent,
    ) -> PortResult<Self> {
        let project = Project::empty(nodes.as_ref())?;
        let clock = runtime.create_clock();
        let last_clock = clock.seconds();
        let next_node_id = project
            .graph
            .nodes()
            .iter()
            .map(|n| n.id.0)
            .max()
            .unwrap_or(0)
            + 1;
        Ok(Self {
            ancestors: Vec::new(),
            execution: Default::default(),
            next_node_id,
            next_asset_id: 0,
            preferences: Default::default(),
            settings_store: runtime.settings_store(),
            modified: std::cell::Cell::new(false),
            revision: std::cell::Cell::new(0),
            project_epoch: 0,
            sync_taps: Default::default(),
            untitled: true,
            file_media: Default::default(),
            asset_sources: crate::asset_sources::AssetSources::new(runtime.create_asset_sources()),
            services: Default::default(),
            project,
            nodes,
            renderer: runtime.create_renderer()?,
            clock,
            frame_trace: runtime.frame_trace(),
            last_trace_clock: None,
            last_render_seconds: 0.0,
            store,
            plan: None,
            extent,
            dirty: true,
            prepare: true,
            valid: false,
            seconds: 0.0,
            last_clock,
            paused: false,
            error: None,
            signals: Default::default(),
            control_io: runtime.create_control_io(),
            input_settings: Default::default(),
            input_status: "Inputs off".into(),
            midi: Default::default(),
            timeline: Default::default(),
            audio_plan: Default::default(),
            audio: runtime.create_audio(),
            audio_error: None,
        })
    }
    pub fn project(&self) -> &Project {
        &self.project
    }
    pub fn extent(&self) -> Extent {
        self.extent
    }
    pub fn seek_start(&mut self) {
        self.seconds = 0.0;
        self.last_clock = self.clock.seconds();
        self.timeline.seek(0.0);
    }
    pub fn seek_end(&mut self) {
        self.paused = true;
        self.timeline.seek(1.0);
    }
    pub fn seconds(&self) -> f64 {
        self.seconds
    }
    pub fn preview(&self) -> Option<PreviewToken> {
        self.renderer.preview()
    }
    pub fn node_error(&self, id: NodeId) -> Option<&str> {
        self.asset_sources
            .error(id)
            .or_else(|| self.renderer.node_error(id))
    }
    pub fn asset_source_items(&self, node: NodeId) -> &[crate::asset_sources::AssetItem] {
        self.asset_sources.items(node)
    }
    pub fn asset_source_status(&self, node: NodeId) -> Option<&str> {
        self.asset_sources.status(node)
    }
    pub fn reload_asset_source(&mut self, node: NodeId) {
        self.asset_sources.reload(node);
        self.invalidate(false);
    }
    pub fn media_status(&self) -> Option<&str> {
        self.asset_sources
            .activity()
            .or_else(|| self.renderer.media_status())
    }
    pub fn backend_description(&self) -> &str {
        self.renderer.description()
    }
    pub fn plan(&self) -> Option<&RenderPlan> {
        self.plan.as_ref()
    }
    pub fn descriptor(&self, type_id: &str) -> &'static NodeDescriptor {
        self.nodes
            .get(type_id)
            .expect("graph types are registered")
            .descriptor()
    }

    pub fn schema(&self, id: NodeId) -> &vjay_domain::schema::NodeSchema {
        self.owner(id)
            .expect("node is part of the project")
            .graph
            .schema(id)
            .expect("registered graph schema")
    }
    pub fn catalog(&self) -> &[&'static NodeDescriptor] {
        self.nodes.catalog()
    }

    pub fn save(&self) -> PortResult<()> {
        self.store.save(&self.root_snapshot()?.document())?;
        self.modified.set(false);
        Ok(())
    }
    pub fn load(&mut self) -> PortResult<()> {
        // Validate the entire candidate before replacing the live project.
        self.restore(self.store.load()?)?;
        self.untitled = false;
        Ok(())
    }
    pub fn restore(&mut self, document: crate::project::ProjectDocument) -> PortResult<()> {
        let candidate = Project::restore(document, self.nodes.as_ref())?;
        self.install_project(candidate);
        Ok(())
    }
    fn install_project(&mut self, candidate: Project) {
        self.project_epoch = self.project_epoch.wrapping_add(1);
        self.project = candidate;
        self.ancestors.clear();
        self.execution = Default::default();
        self.reserve_identities();
        self.asset_sources.reset();
        self.signals = Default::default();
        self.file_media = Default::default();
        self.timeline = Default::default();
        self.audio_plan = Default::default();
        self.audio_error = self.audio.prepare(&self.audio_plan, &[]).err();
        self.seconds = 0.0;
        self.last_clock = self.clock.seconds();
        self.midi.learning = None;
        if self.project.any_node(&|n| {
            self.descriptor(&n.type_id)
                .capabilities
                .settings
                .contains(&vjay_domain::capabilities::SettingEditor::Midi)
        }) {
            self.ensure_midi();
        }
        self.invalidate(true);
        self.modified.set(false);
        self.sync_taps.clear();
    }
    pub fn store_location(&self) -> &std::path::Path {
        self.store.location()
    }
    pub fn window_output(&self, node: NodeId) -> bool {
        self.owner(node)
            .and_then(|p| p.graph.node(node).ok())
            .is_some_and(|n| {
                self.schema(node).capabilities.program_output.is_some()
                    && self
                        .nodes
                        .get(&n.type_id)
                        .is_ok_and(|d| d.video_sink(&n.settings).is_none())
            })
    }
    pub fn video_sources(&self, provider: &str) -> Vec<String> {
        self.renderer.video_sources(provider)
    }
    pub fn capture_output(&mut self, node: NodeId) -> PortResult<CapturedFrame> {
        self.renderer.capture_output(node)
    }
    pub fn capture(&mut self) -> PortResult<CapturedFrame> {
        if !self.valid {
            return Err("No valid rendered frame to capture".into());
        }
        self.renderer.capture()
    }
    pub fn wait_idle(&self) -> PortResult<()> {
        self.renderer.wait_idle()
    }
    pub fn render_metrics(&self) -> crate::ports::telemetry::RenderMetrics {
        self.renderer.render_metrics()
    }
    pub fn media_metrics(&self) -> crate::media::MediaMetrics {
        self.renderer.media_metrics()
    }
}
