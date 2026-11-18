use crate::ports::PortResult;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use vjay_domain::{
    execution::AssetRequest,
    graph::{Graph, NodeId},
    node::NodeFactory,
};
#[derive(Clone, Debug)]
pub struct AssetItem {
    pub label: String,
    pub path: PathBuf,
}
pub trait ResolvedAsset: Send {
    fn items(&self) -> Arc<[AssetItem]> {
        Arc::from([])
    }
    fn path(&self) -> &Path;
}
pub trait AssetJob: Send {
    fn poll(
        &mut self,
        delivery: crate::media::FrameDelivery,
    ) -> Option<PortResult<Box<dyn ResolvedAsset>>>;
}
pub trait AssetSourceFactory: Send + Sync {
    fn start(&self, request: &AssetRequest) -> PortResult<Box<dyn AssetJob>>;
}
pub struct DisabledAssetSources;
impl AssetSourceFactory for DisabledAssetSources {
    fn start(&self, _: &AssetRequest) -> PortResult<Box<dyn AssetJob>> {
        Err("URL media adapter is unavailable".into())
    }
}
struct Source {
    node: NodeId,
    request: AssetRequest,
    job: Option<Box<dyn AssetJob>>,
    status: String,
    failed: bool,
    selection: Option<crate::signals::ValueRef>,
    items: Arc<[AssetItem]>,
    selected: usize,
    still_image: bool,
}
pub struct AssetSources {
    factory: Arc<dyn AssetSourceFactory>,
    sources: Vec<Source>,
    assets: Vec<Box<dyn ResolvedAsset>>,
}
impl AssetSources {
    pub fn new(factory: Arc<dyn AssetSourceFactory>) -> Self {
        Self {
            factory,
            sources: Vec::new(),
            assets: Vec::new(),
        }
    }
    pub fn reset(&mut self) {
        self.sources.clear();
        self.assets.clear();
    }
    pub fn retain_assets(&mut self, assets: &[crate::media::MediaAsset]) {
        self.assets
            .retain(|a| assets.iter().any(|b| b.path == a.path()));
    }
    pub fn reload(&mut self, node: NodeId) {
        self.sources.retain(|s| s.node != node);
    }
    pub fn status(&self, node: NodeId) -> Option<&str> {
        self.sources
            .iter()
            .find(|s| s.node == node)
            .map(|s| s.status.as_str())
    }
    pub fn error(&self, node: NodeId) -> Option<&str> {
        self.sources
            .iter()
            .find(|s| s.node == node && s.failed)
            .map(|s| s.status.as_str())
    }
    pub fn activity(&self) -> Option<&str> {
        self.sources
            .iter()
            .find(|s| s.job.is_some() || s.failed)
            .map(|s| s.status.as_str())
    }
    pub fn rebuild(
        &mut self,
        graph: &Graph,
        nodes: &dyn NodeFactory,
        assets: &[crate::media::MediaAsset],
    ) -> PortResult<()> {
        self.assets
            .retain(|a| assets.iter().any(|b| b.path == a.path()));
        let mut previous = std::mem::take(&mut self.sources);
        let bindings = crate::bindings::PortBindings::new(graph)?;
        for (node_index, node) in graph.nodes().iter().enumerate() {
            let schema = graph.schema(node.id).map_err(|e| e.to_string())?;
            if schema
                .capabilities
                .file_input
                .is_some_and(|p| bindings.source(node_index, p).is_some())
            {
                continue;
            }
            let Some(mut request) = nodes.get(&node.type_id)?.asset_source(&node.settings) else {
                continue;
            };
            if let Some(p) = request.selection_parameter {
                if p >= node.parameters.len() {
                    return Err("Asset selection parameter is outside the node schema".into());
                }
                let input = graph
                    .schema(node.id)
                    .map_err(|e| e.to_string())?
                    .inputs
                    .len()
                    + p;
                let selected = bindings.source(node_index, input).and_then(|_| {
                    previous
                        .iter()
                        .find(|s| s.node == node.id && s.request.location == request.location)
                        .map(|s| s.selected)
                });
                request.options.insert(
                    "index".into(),
                    selected.map_or_else(
                        || {
                            node.parameters
                                .get(p)
                                .copied()
                                .unwrap_or(0.0)
                                .round()
                                .to_string()
                        },
                        |i| i.to_string(),
                    ),
                );
            }
            if let Some(index) = previous
                .iter()
                .position(|s| s.node == node.id && s.request == request)
            {
                self.sources.push(previous.swap_remove(index));
                continue;
            }
            let items = previous
                .iter()
                .find(|s| s.node == node.id && s.request.location == request.location)
                .map(|s| s.items.clone())
                .unwrap_or_else(|| Arc::from([]));
            let result = self.factory.start(&request);
            let (job, status, failed) = match result {
                Ok(job) => (Some(job), "Loading video…".into(), false),
                Err(error) => (None, error, true),
            };
            let selected = request
                .options
                .get("index")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            self.sources.push(Source {
                node: node.id,
                request,
                job,
                status,
                failed,
                selected,
                selection: None,
                items,
                still_image: schema.capabilities.still_image,
            });
        }
        Ok(())
    }
    pub fn bind(&mut self, signals: &crate::signals::SignalEngine) {
        for source in &mut self.sources {
            source.selection = source
                .request
                .selection_parameter
                .and_then(|p| signals.parameter(source.node, p));
        }
    }
    pub fn tick(&mut self, signals: &crate::signals::SignalEngine) {
        for source in &mut self.sources {
            let Some(selection) = source.selection else {
                continue;
            };
            let index = selection.read(&signals.values).round() as usize;
            if source.job.is_some() || source.selected == index {
                continue;
            }
            source.selected = index;
            source
                .request
                .options
                .insert("index".into(), index.to_string());
            match self.factory.start(&source.request) {
                Ok(job) => {
                    source.job = Some(job);
                    source.failed = false;
                    source.status = "Loading video…".into();
                }
                Err(error) => {
                    source.status = error;
                    source.failed = true;
                }
            }
        }
    }
    pub fn items(&self, node: NodeId) -> &[AssetItem] {
        self.sources
            .iter()
            .find(|s| s.node == node)
            .map_or(&[], |s| s.items.as_ref())
    }
    pub fn file_output(&self, node: NodeId) -> bool {
        self.sources
            .iter()
            .find(|s| s.node == node)
            .is_some_and(|s| s.request.file_output)
    }
    pub fn still_image(&self, node: NodeId) -> bool {
        self.sources
            .iter()
            .find(|s| s.node == node)
            .is_some_and(|s| s.still_image)
    }
    pub fn poll(&mut self, delivery: crate::media::FrameDelivery) -> Option<(NodeId, PathBuf)> {
        for source in &mut self.sources {
            let Some(result) = source.job.as_mut().and_then(|j| j.poll(delivery)) else {
                continue;
            };
            source.job = None;
            match result {
                Ok(asset) => {
                    source.items = asset.items();
                    let path = asset.path().to_owned();
                    if !self.assets.iter().any(|old| old.path() == asset.path()) {
                        self.assets.push(asset);
                    }
                    source.status = "Ready".into();
                    return Some((source.node, path));
                }
                Err(error) => {
                    source.status = error;
                    source.failed = true;
                }
            }
        }
        None
    }
}
#[cfg(test)]
mod tests;
