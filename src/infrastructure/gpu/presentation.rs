use vjay_application::ports::PortResult;

#[derive(Clone, Copy, PartialEq)]
pub struct PreviewBounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub window_height: f32,
    pub scale: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreviewRegion {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
/// Native viewport boundary. No CPU pixel transfer is part of presentation.
pub trait NodePreviewFactory {
    fn create(&self) -> PortResult<Box<dyn GpuPresentation>>;
}

pub trait GpuPresentation {
    fn set_preview_regions(&mut self, _regions: &[PreviewRegion]) {}
    fn set_node_regions(
        &mut self,
        node: vjay_domain::graph::NodeId,
        bounds: PreviewBounds,
        regions: &[PreviewRegion],
    ) {
        self.set_node_bounds(node, (!regions.is_empty()).then_some(bounds));
    }
    fn begin_nodes_frame(&mut self) {}
    fn finish_nodes_frame(&mut self) {}
    fn prepare_outputs(
        &mut self,
        _device: &wgpu::Device,
        _outputs: &[(vjay_domain::graph::NodeId, wgpu::TextureView)],
    ) -> PortResult<()> {
        Ok(())
    }
    fn prepare_nodes(
        &mut self,
        _device: &wgpu::Device,
        _nodes: &[(vjay_domain::graph::NodeId, wgpu::TextureView)],
    ) -> PortResult<()> {
        Ok(())
    }
    fn set_node_bounds(
        &mut self,
        _node: vjay_domain::graph::NodeId,
        _bounds: Option<PreviewBounds>,
    ) {
    }
    fn set_nodes_hidden(&mut self, _hidden: bool) {}
    fn encode_node(
        &mut self,
        _index: usize,
        _device: &wgpu::Device,
        _encoder: &mut wgpu::CommandEncoder,
    ) -> PortResult<()> {
        Ok(())
    }
    fn set_hidden(&mut self, _hidden: bool) {}
    fn instance(&self) -> &wgpu::Instance;
    fn surface(&self) -> &wgpu::Surface<'static>;
    fn initialize(&mut self, device: &wgpu::Device, adapter: &wgpu::Adapter) -> PortResult<()>;
    fn prepare(&mut self, device: &wgpu::Device, image: &wgpu::TextureView) -> PortResult<()>;
    fn set_bounds(&mut self, bounds: PreviewBounds);
    fn encode(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
    ) -> PortResult<()>;
    fn present(&mut self, queue: &wgpu::Queue);
}
