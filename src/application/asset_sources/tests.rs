use super::*;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use vjay_domain::{
    capabilities::Capabilities,
    node::{Node, NodeDescriptor},
    port::Port,
    registry::NodeRegistry,
    settings::NodeSettings,
};
#[derive(Default)]
struct Counts {
    starts: AtomicUsize,
    jobs_dropped: AtomicUsize,
    assets_dropped: AtomicUsize,
    ready: AtomicBool,
}
struct Factory(Arc<Counts>);
struct Job(Arc<Counts>, PathBuf);
struct Asset(Arc<Counts>, PathBuf);
impl AssetSourceFactory for Factory {
    fn start(&self, request: &AssetRequest) -> PortResult<Box<dyn AssetJob>> {
        self.0.starts.fetch_add(1, Ordering::Relaxed);
        Ok(Box::new(Job(
            self.0.clone(),
            format!("/clips/{}", request.location).into(),
        )))
    }
}
impl AssetJob for Job {
    fn poll(
        &mut self,
        _: crate::media::FrameDelivery,
    ) -> Option<PortResult<Box<dyn ResolvedAsset>>> {
        self.0
            .ready
            .load(Ordering::Acquire)
            .then(|| Ok(Box::new(Asset(self.0.clone(), self.1.clone())) as Box<dyn ResolvedAsset>))
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        self.0.jobs_dropped.fetch_add(1, Ordering::Relaxed);
    }
}
impl ResolvedAsset for Asset {
    fn path(&self) -> &Path {
        &self.1
    }
}
impl Drop for Asset {
    fn drop(&mut self) {
        self.0.assets_dropped.fetch_add(1, Ordering::Relaxed);
    }
}
struct CustomSource;
static D: NodeDescriptor = NodeDescriptor {
    type_id: "custom.source",
    name: "Custom source",
    category: "Extension",
    inputs: &[],
    outputs: &[Port::texture("Image")],
    parameters: &[],
    capabilities: Capabilities::NONE,
};
impl Node for CustomSource {
    fn descriptor(&self) -> &'static NodeDescriptor {
        &D
    }
    fn default_settings(&self) -> NodeSettings {
        let mut s = NodeSettings::default();
        s.properties.insert("source".into(), "a".into());
        s
    }
    fn asset_source(&self, s: &NodeSettings) -> Option<AssetRequest> {
        Some(AssetRequest {
            provider: "custom-provider",
            selection_parameter: None,
            file_output: false,
            location: s.properties["source"].clone(),
            options: Default::default(),
        })
    }
}
fn fixture() -> (NodeRegistry, Graph, NodeId, Arc<Counts>, AssetSources) {
    let mut nodes = NodeRegistry::default();
    nodes.register(CustomSource).unwrap();
    let mut graph = Graph::default();
    let node = graph.add("custom.source", &nodes).unwrap();
    let counts = Arc::new(Counts::default());
    let sources = AssetSources::new(Arc::new(Factory(counts.clone())));
    (nodes, graph, node, counts, sources)
}
#[test]
fn open_source_contract_reuses_jobs_and_cancels_replaced_or_deleted_requests() {
    let (nodes, mut graph, node, counts, mut sources) = fixture();
    sources.rebuild(&graph, &nodes, &[]).unwrap();
    sources.rebuild(&graph, &nodes, &[]).unwrap();
    assert_eq!(counts.starts.load(Ordering::Relaxed), 1);
    assert!(sources.poll(crate::media::FrameDelivery::Latest).is_none());
    let mut s = graph.node(node).unwrap().settings.clone();
    s.properties.insert("source".into(), "b".into());
    graph.set_settings(node, s, &nodes).unwrap();
    sources.rebuild(&graph, &nodes, &[]).unwrap();
    assert_eq!(counts.starts.load(Ordering::Relaxed), 2);
    assert_eq!(counts.jobs_dropped.load(Ordering::Relaxed), 1);
    graph.remove(node).unwrap();
    sources.rebuild(&graph, &nodes, &[]).unwrap();
    assert_eq!(counts.jobs_dropped.load(Ordering::Relaxed), 2);
}
#[test]
fn resolved_assets_outlive_a_source_node_while_an_asset_reference_remains() {
    let (nodes, mut graph, node, counts, mut sources) = fixture();
    sources.rebuild(&graph, &nodes, &[]).unwrap();
    counts.ready.store(true, Ordering::Release);
    let (_, path) = sources.poll(crate::media::FrameDelivery::Exact).unwrap();
    assert_eq!(path, PathBuf::from("/clips/a"));
    graph.remove(node).unwrap();
    let assets = [crate::media::MediaAsset {
        id: vjay_domain::node::AssetId(0),
        path,
    }];
    sources.rebuild(&graph, &nodes, &assets).unwrap();
    assert_eq!(counts.assets_dropped.load(Ordering::Relaxed), 0);
    sources.retain_assets(&[]);
    assert_eq!(counts.assets_dropped.load(Ordering::Relaxed), 1);
}
