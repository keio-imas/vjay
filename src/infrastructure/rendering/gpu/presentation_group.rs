use super::presentation::{GpuPresentation, PreviewBounds};
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};
use vjay_application::ports::PortResult;

/// One renderer and one queue feed the editor and independent output windows.
/// Window views own extra surfaces; weak registrations remove closed windows.
pub struct PresentationGroup {
    primary: Box<dyn GpuPresentation>,
    vsync: bool,
    outputs: Vec<(
        vjay_domain::graph::NodeId,
        Weak<RefCell<dyn GpuPresentation>>,
    )>,
    output_sources: Vec<(vjay_domain::graph::NodeId, wgpu::TextureView)>,
    source: Option<wgpu::TextureView>,
    device: Option<wgpu::Device>,
    nodes: Vec<NodeSurface>,
    node_factory: Option<Box<dyn super::presentation::NodePreviewFactory>>,
    nodes_hidden: bool,
}
impl PresentationGroup {
    pub fn new(primary: Box<dyn GpuPresentation>) -> Self {
        Self {
            primary,
            vsync: true,
            outputs: Vec::new(),
            output_sources: Vec::new(),
            source: None,
            device: None,
            nodes: Vec::new(),
            node_factory: None,
            nodes_hidden: false,
        }
    }
    pub fn set_node_factory(&mut self, factory: Box<dyn super::presentation::NodePreviewFactory>) {
        self.node_factory = Some(factory);
    }
    pub fn register(
        &mut self,
        node: vjay_domain::graph::NodeId,
        output: &Rc<RefCell<dyn GpuPresentation>>,
    ) -> PortResult<()> {
        if let (Some(device), Some((_, source))) = (
            &self.device,
            self.output_sources.iter().find(|(id, _)| *id == node),
        ) {
            output.borrow_mut().prepare(device, source)?;
        } else {
            output.borrow_mut().set_hidden(true);
        }
        output.borrow_mut().set_vsync(self.vsync);
        self.outputs.push((node, Rc::downgrade(output)));
        Ok(())
    }
}
struct NodeSurface {
    id: vjay_domain::graph::NodeId,
    preview: Box<dyn GpuPresentation>,
    visible: bool,
}
impl GpuPresentation for PresentationGroup {
    fn set_vsync(&mut self, enabled: bool) {
        self.vsync = enabled;
        self.primary.set_vsync(enabled);
        for node in &mut self.nodes {
            node.preview.set_vsync(enabled);
        }
        for (_, output) in &self.outputs {
            if let Some(output) = output.upgrade() {
                output.borrow_mut().set_vsync(enabled);
            }
        }
    }
    fn prepare_outputs(
        &mut self,
        device: &wgpu::Device,
        outputs: &[(vjay_domain::graph::NodeId, wgpu::TextureView)],
    ) -> PortResult<()> {
        self.output_sources = outputs.to_vec();
        for (id, weak) in &self.outputs {
            if let Some(output) = weak.upgrade() {
                if let Some((_, image)) = outputs.iter().find(|(node, _)| node == id) {
                    output.borrow_mut().prepare(device, image)?;
                    output.borrow_mut().set_hidden(false);
                } else {
                    output.borrow_mut().set_hidden(true);
                }
            }
        }
        Ok(())
    }
    fn set_node_regions(
        &mut self,
        node: vjay_domain::graph::NodeId,
        bounds: PreviewBounds,
        regions: &[super::presentation::PreviewRegion],
    ) {
        if let Some(surface) = self.nodes.iter_mut().find(|s| s.id == node) {
            surface.visible = !regions.is_empty();
            surface.preview.set_bounds(bounds);
            surface.preview.set_preview_regions(regions);
            surface
                .preview
                .set_hidden(!surface.visible || self.nodes_hidden);
        }
    }
    fn begin_nodes_frame(&mut self) {
        for node in &mut self.nodes {
            node.visible = false;
        }
    }
    fn finish_nodes_frame(&mut self) {
        for node in &mut self.nodes {
            if !node.visible {
                node.preview.set_hidden(true);
            }
        }
    }
    fn prepare_nodes(
        &mut self,
        device: &wgpu::Device,
        nodes: &[(vjay_domain::graph::NodeId, wgpu::TextureView)],
    ) -> PortResult<()> {
        let Some(factory) = &self.node_factory else {
            return Ok(());
        };
        let mut old = std::mem::take(&mut self.nodes);
        for (id, image) in nodes {
            let mut surface = if let Some(i) = old.iter().position(|n| n.id == *id) {
                old.swap_remove(i)
            } else {
                let mut preview = factory.create()?;
                preview.set_vsync(self.vsync);
                preview.set_hidden(true);
                NodeSurface {
                    id: *id,
                    preview,
                    visible: false,
                }
            };
            surface.preview.prepare(device, image)?;
            self.nodes.push(surface);
        }
        Ok(())
    }
    fn set_node_bounds(&mut self, node: vjay_domain::graph::NodeId, bounds: Option<PreviewBounds>) {
        if let Some(surface) = self.nodes.iter_mut().find(|s| s.id == node) {
            surface.visible = bounds.is_some();
            if let Some(bounds) = bounds {
                surface.preview.set_bounds(bounds);
            }
            surface
                .preview
                .set_hidden(!surface.visible || self.nodes_hidden);
        }
    }
    fn set_nodes_hidden(&mut self, hidden: bool) {
        self.nodes_hidden = hidden;
        for node in &mut self.nodes {
            node.preview.set_hidden(hidden || !node.visible);
        }
    }
    fn encode_node(
        &mut self,
        index: usize,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
    ) -> PortResult<()> {
        if let Some(node) = self.nodes.get_mut(index) {
            node.preview.encode(device, encoder)?;
        }
        Ok(())
    }
    fn set_hidden(&mut self, hidden: bool) {
        self.primary.set_hidden(hidden);
    }
    fn instance(&self) -> &wgpu::Instance {
        self.primary.instance()
    }
    fn surface(&self) -> &wgpu::Surface<'static> {
        self.primary.surface()
    }
    fn initialize(&mut self, device: &wgpu::Device, adapter: &wgpu::Adapter) -> PortResult<()> {
        self.device = Some(device.clone());
        self.primary.initialize(device, adapter)
    }
    fn prepare(&mut self, device: &wgpu::Device, image: &wgpu::TextureView) -> PortResult<()> {
        self.primary.prepare(device, image)?;
        self.source = Some(image.clone());
        Ok(())
    }
    fn set_bounds(&mut self, bounds: PreviewBounds) {
        self.primary.set_bounds(bounds);
    }
    fn encode(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
    ) -> PortResult<()> {
        self.outputs.retain(|(_, output)| output.strong_count() > 0);
        self.primary.encode(device, encoder)?;
        for output in self.outputs.iter().filter_map(|(_, p)| p.upgrade()) {
            output.borrow_mut().encode(device, encoder)?;
        }
        Ok(())
    }
    fn present(&mut self, queue: &wgpu::Queue) {
        self.primary.present(queue);
        for node in &mut self.nodes {
            node.preview.present(queue);
        }
        for output in self.outputs.iter().filter_map(|(_, p)| p.upgrade()) {
            output.borrow_mut().present(queue);
        }
    }
}
