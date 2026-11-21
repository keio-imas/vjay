use crate::{
    builtins::ImageNode,
    capabilities::Capabilities,
    execution::{ImageKernel, ShaderSource},
    node::{Node, NodeDescriptor as D, Parameter as P},
    port::Port,
    registry::NodeRegistry,
    settings::NodeSettings,
};
struct Spatial3D {
    descriptor: &'static D,
    mode: f32,
}
impl Node for Spatial3D {
    fn descriptor(&self) -> &'static D {
        self.descriptor
    }
    fn image(&self, values: &[f32], settings: &NodeSettings, output: usize) -> Option<ImageKernel> {
        let mut kernel = ImageNode {
            descriptor: self.descriptor,
            selector: self.mode,
        }
        .image(values, settings, output)?;
        kernel.shader = ShaderSource::Registered("vjay.spatial3d.v1");
        Some(kernel)
    }
}
const OUT: &[Port] = &[Port::texture("Image")];
const fn angle(name: &'static str) -> P {
    P::new(name, -180.0, 180.0, 0.0).wrapping(360.0)
}
pub(crate) fn register(r: &mut NodeRegistry) {
    static TRANSFORM: D = D {
        type_id: "Transform3D",
        name: "3D Transform",
        category: "3D",
        inputs: &[Port::texture("Image")],
        outputs: OUT,
        capabilities: Capabilities::NONE,
        parameters: &[
            P::new("X", -2.0, 2.0, 0.0),
            P::new("Y", -2.0, 2.0, 0.0),
            P::new("Z", -8.0, 2.0, 0.0),
            angle("Rotation X"),
            angle("Rotation Y"),
            angle("Rotation Z"),
            P::new("Scale", 0.001, 4.0, 1.0),
            P::new("FOV", 10.0, 140.0, 60.0),
        ],
    };
    static SHAPE: D = D {
        type_id: "Primitive3D",
        name: "3D Shape",
        category: "3D",
        inputs: &[Port::texture("Texture").optional()],
        outputs: OUT,
        capabilities: Capabilities::NONE,
        parameters: &[
            P::new("Shape", 0.0, 3.0, 0.0).choices(&["Sphere", "Cube", "Torus", "Cylinder"]),
            angle("Rotation X"),
            angle("Rotation Y"),
            angle("Rotation Z"),
            P::new("Scale", 0.1, 2.0, 1.0),
            P::new("Camera distance", 2.0, 10.0, 3.0),
            P::new("Texture amount", 0.0, 1.0, 0.0),
            P::new("Light", 0.0, 2.0, 1.0),
            P::new("Red", 0.0, 1.0, 0.25),
            P::new("Green", 0.0, 1.0, 0.65),
            P::new("Blue", 0.0, 1.0, 1.0),
            P::new("Opacity", 0.0, 1.0, 1.0),
        ],
    };
    static TUNNEL: D = D {
        type_id: "Tunnel3D",
        name: "3D Tunnel",
        category: "3D",
        inputs: &[Port::texture("Image")],
        outputs: OUT,
        capabilities: Capabilities::NONE,
        parameters: &[
            P::new("Radius", 0.05, 2.0, 0.5),
            P::new("Depth repeat", 0.1, 8.0, 1.0),
            angle("Rotation"),
            P::new("Speed", -4.0, 4.0, 0.2),
            P::new("Amount", 0.0, 1.0, 1.0),
        ],
    };
    for (descriptor, mode) in [(&TRANSFORM, 0.0), (&SHAPE, 1.0), (&TUNNEL, 2.0)] {
        r.register(Spatial3D { descriptor, mode })
            .expect("valid 3D node");
    }
}
