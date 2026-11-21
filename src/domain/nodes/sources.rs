use crate::builtins::{ImageNode, ids};
use crate::capabilities::*;
use crate::{execution::ImageKernel, node::Node, settings::NodeSettings};
use crate::{
    node::{NodeDescriptor, Parameter as P},
    port::Port,
    registry::NodeRegistry,
};

static VIDEO: NodeDescriptor = NodeDescriptor {
    type_id: ids::VIDEO,
    category: "Sources",
    capabilities: Capabilities {
        transport: Some(crate::transport::TransportBinding::at(0)),
        media: Some(MediaBinding {
            speed: 0,
            position: 1,
            looping: Some(2),
            start: Some(3),
            end: Some(4),
            seek: Some(5),
        }),
        momentary_parameter: Some(5),
        audio: Some(AudioRecipe::Source),
        ..Capabilities::NONE
    },
    name: "Video",
    inputs: &[],
    outputs: &[Port::texture("Video"), Port::audio("Audio")],
    parameters: &[
        P::new("Speed", 0.0, 8.0, 1.0),
        P::new("Seek position", 0.0, 1.0, 0.0),
        P::new("Loop", 0.0, 1.0, 1.0).choices(crate::choices::SWITCH),
        P::new("Range start", 0.0, 1.0, 0.0),
        P::new("Range end", 0.0, 1.0, 1.0),
        P::new("Seek", 0.0, 1.0, 0.0).choices(crate::choices::SWITCH),
    ],
};

static SOLID: NodeDescriptor = NodeDescriptor {
    type_id: ids::SOLID,
    category: "Sources",
    capabilities: Capabilities::NONE,
    name: "Solid color",
    inputs: &[],
    outputs: &[Port::texture("Image")],
    parameters: &[
        P::new("Red", 0.0, 1.0, 1.0),
        P::new("Green", 0.0, 1.0, 0.3),
        P::new("Blue", 0.0, 1.0, 0.1),
    ],
};

pub(crate) fn register(registry: &mut NodeRegistry) {
    registry
        .register(VideoNode)
        .expect("valid built-in definition");
    registry
        .register(ImageNode {
            descriptor: &SOLID,
            selector: 4.0,
        })
        .expect("valid built-in definition");
}

struct VideoNode;
impl Node for VideoNode {
    fn feedback(&self) -> Option<crate::execution::ScalarKernel> {
        Some(crate::transport::feedback())
    }
    fn port_version(&self) -> u32 {
        1
    }
    fn migrate_input_port(&self, input: usize, from: u32) -> usize {
        input + if from == 0 { 8 } else { 0 }
    }

    fn descriptor(&self) -> &'static NodeDescriptor {
        &VIDEO
    }
    fn schema(&self, _: &NodeSettings) -> crate::schema::NodeSchema {
        crate::transport::schema(&VIDEO)
    }
    fn image(&self, values: &[f32], settings: &NodeSettings, output: usize) -> Option<ImageKernel> {
        ImageNode {
            descriptor: &VIDEO,
            selector: 3.0,
        }
        .image(values, settings, output)
    }
    fn migrate_parameters(&self, values: &mut Vec<f32>) {
        if values.len() == 2 {
            values.extend_from_slice(&[1.0, 0.0, 1.0]);
        }
        if values.len() == 5 {
            values.push(0.0);
        }
    }
    fn validate_parameters(&self, values: &[f32], _: &NodeSettings) -> bool {
        crate::node::validate_parameters(&VIDEO, values) && values[3] < values[4]
    }
}
