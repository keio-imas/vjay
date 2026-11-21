//! Optional built-in definitions. Consumers depend on NodeFactory, not these IDs.
use crate::{execution::*, node::*, registry::NodeRegistry, settings::NodeSettings};
pub mod ids;
mod images;
pub(crate) use crate::fragment::FragmentNode;
pub(crate) mod scalar;

pub fn registry() -> NodeRegistry {
    let mut registry = NodeRegistry::default();
    crate::composition::register(&mut registry);
    crate::seek_control::register(&mut registry);
    crate::deck_rate::register(&mut registry);
    crate::media_device::register(&mut registry);
    images::register(&mut registry);
    crate::sources::register(&mut registry);
    crate::spatial::register(&mut registry);
    crate::effects::register(&mut registry);
    crate::extra_nodes::register(&mut registry);
    crate::control_nodes::register(&mut registry);
    crate::visual_nodes::register(&mut registry);
    crate::math_nodes::register(&mut registry);
    crate::annotations::register(&mut registry);
    crate::noise::register(&mut registry);
    crate::spatial3d::register(&mut registry);
    crate::lip_sync::register(&mut registry);
    crate::text_value::register(&mut registry);
    crate::text_node::register(&mut registry);
    crate::web_video::register(&mut registry);
    crate::video_io_nodes::register(&mut registry);
    crate::file_nodes::register(&mut registry);
    crate::loadable_video::register(&mut registry);
    registry
}
pub(crate) struct ImageNode {
    pub(crate) descriptor: &'static NodeDescriptor,
    pub(crate) selector: f32,
}
impl Node for ImageNode {
    fn descriptor(&self) -> &'static NodeDescriptor {
        self.descriptor
    }
    fn image(&self, values: &[f32], _: &NodeSettings, output: usize) -> Option<ImageKernel> {
        let mut parameters = vec![0.0; values.len().max(4)];
        let mut parameter_slots = vec![None; parameters.len()];
        for (index, &value) in values.iter().enumerate() {
            parameters[index] = value;
            parameter_slots[index] = Some(index);
        }
        // This particular shared shader emits an RGBA component per output.
        if self.descriptor.type_id == ids::RGB_SPLIT {
            parameters[0] = output as f32;
        }
        Some(ImageKernel {
            shader: ShaderSource::Registered("vjay.images.v1"),
            selector: self.selector,
            parameters,
            parameter_slots,
            prepared: None,
            stream: None,
            media: self.descriptor.capabilities.media,
        })
    }
}
pub(crate) struct RoutingNode(pub(crate) &'static NodeDescriptor);
impl Node for RoutingNode {
    fn descriptor(&self) -> &'static NodeDescriptor {
        self.0
    }
}
pub(crate) struct SignalNode {
    pub(crate) descriptor: &'static NodeDescriptor,
    pub(crate) evaluate: ScalarEvaluate,
    pub(crate) cells: usize,
    pub(crate) tap: Option<ScalarTap>,
}
impl Node for SignalNode {
    fn descriptor(&self) -> &'static NodeDescriptor {
        self.descriptor
    }
    fn scalar(&self) -> Option<ScalarKernel> {
        Some(ScalarKernel {
            evaluate: self.evaluate,
            state: vec![0.0; self.cells],
            tap: self.tap,
        })
    }
}
