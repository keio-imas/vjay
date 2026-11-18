use crate::{
    builtins::ImageNode,
    capabilities::Capabilities as C,
    node::{Node, NodeDescriptor as D, Parameter as P},
    port::Port,
    registry::NodeRegistry,
    settings::NodeSettings,
};
struct Visual {
    descriptor: &'static D,
    mode: f32,
}
impl Node for Visual {
    fn descriptor(&self) -> &'static D {
        self.descriptor
    }
    fn image(
        &self,
        values: &[f32],
        settings: &NodeSettings,
        output: usize,
    ) -> Option<crate::execution::ImageKernel> {
        let mut kernel = ImageNode {
            descriptor: self.descriptor,
            selector: self.mode,
        }
        .image(values, settings, output)?;
        kernel.shader = crate::execution::ShaderSource::Registered("vjay.composite.v1");
        Some(kernel)
    }
}
const ONE: &[Port] = &[Port::texture("Image")];
const OUT: &[Port] = &[Port::texture("Image")];
const PAIR: &[Port] = &[
    Port::texture("Background").optional(),
    Port::texture("Foreground"),
];
const EIGHT: &[Port] = &[
    Port::texture("1").optional(),
    Port::texture("2").optional(),
    Port::texture("3").optional(),
    Port::texture("4").optional(),
    Port::texture("5").optional(),
    Port::texture("6").optional(),
    Port::texture("7").optional(),
    Port::texture("8").optional(),
];
macro_rules! visual {
    ($r:ident,$id:literal,$name:literal,$category:literal,$inputs:expr,$mode:expr,$params:expr) => {{
        static DEFINITION: D = D {
            type_id: $id,
            name: $name,
            category: $category,
            inputs: $inputs,
            outputs: OUT,
            parameters: $params,
            capabilities: C::NONE,
        };
        $r.register(Visual {
            descriptor: &DEFINITION,
            mode: $mode,
        })
        .expect("valid visual node");
    }};
}
pub(crate) fn register(r: &mut NodeRegistry) {
    static IMAGE: D = D {
        type_id: "Image",
        name: "Image",
        category: "Sources",
        inputs: &[],
        outputs: OUT,
        parameters: &[],
        capabilities: C {
            still_image: true,
            ..C::NONE
        },
    };
    r.register(ImageNode {
        descriptor: &IMAGE,
        selector: 3.0,
    })
    .expect("valid image node");
    visual!(
        r,
        "Composite",
        "Composite / Over",
        "Compositing",
        PAIR,
        0.0,
        &[
            P::new("X", -2.0, 2.0, 0.0),
            P::new("Y", -2.0, 2.0, 0.0),
            P::new("Scale X", 0.01, 4.0, 1.0),
            P::new("Scale Y", 0.01, 4.0, 1.0),
            P::new("Rotation", -180.0, 180.0, 0.0).wrapping(360.0),
            P::new("Opacity", 0.0, 1.0, 1.0)
        ]
    );
    visual!(
        r,
        "ImageSelector",
        "Image selector",
        "Compositing",
        EIGHT,
        1.0,
        &[P::new("Index", 0.0, 7.0, 0.0).choices(crate::choices::INPUTS)]
    );
    visual!(
        r,
        "Grid",
        "Grid",
        "Compositing",
        EIGHT,
        2.0,
        &[
            P::new("Columns", 1.0, 8.0, 2.0),
            P::new("Rows", 1.0, 8.0, 2.0),
            P::new("Gap", 0.0, 0.4, 0.02),
            P::new("Keep aspect", 0.0, 1.0, 1.0).choices(crate::choices::SWITCH)
        ]
    );
    visual!(
        r,
        "ChromaKey",
        "Chroma key",
        "Effects",
        ONE,
        3.0,
        &[
            P::new("Key R", 0.0, 1.0, 0.0),
            P::new("Key G", 0.0, 1.0, 1.0),
            P::new("Key B", 0.0, 1.0, 0.0),
            P::new("Tolerance", 0.0, 1.0, 0.15),
            P::new("Softness", 0.001, 1.0, 0.08),
            P::new("Spill", 0.0, 1.0, 0.4)
        ]
    );
    visual!(
        r,
        "DuplicateGrid",
        "Duplicate grid",
        "Compositing",
        ONE,
        12.0,
        &[
            P::new("Columns", 1.0, 16.0, 3.0),
            P::new("Rows", 1.0, 16.0, 3.0),
            P::new("Gap", 0.0, 0.4, 0.02),
            P::new("Keep aspect", 0.0, 1.0, 1.0).choices(crate::choices::SWITCH),
        ]
    );
    visual!(
        r,
        "Mask",
        "Alpha / Luma mask",
        "Compositing",
        &[Port::texture("Image"), Port::texture("Mask")],
        4.0,
        &[
            P::new("Use alpha", 0.0, 1.0, 0.0).choices(&["Luminance", "Alpha"]),
            P::new("Invert", 0.0, 1.0, 0.0).choices(crate::choices::SWITCH)
        ]
    );
    visual!(
        r,
        "Wipe",
        "Wipe",
        "Compositing",
        PAIR,
        5.0,
        &[
            P::new("Progress", 0.0, 1.0, 0.5),
            P::new("Angle", -180.0, 180.0, 0.0).wrapping(360.0),
            P::new("Softness", 0.001, 0.5, 0.01)
        ]
    );
    visual!(
        r,
        "Crop",
        "Crop",
        "Geometry",
        ONE,
        6.0,
        &[
            P::new("Left", 0.0, 1.0, 0.0),
            P::new("Right", 0.0, 1.0, 1.0),
            P::new("Top", 0.0, 1.0, 0.0),
            P::new("Bottom", 0.0, 1.0, 1.0)
        ]
    );
    visual!(
        r,
        "Mirror",
        "Mirror / Flip",
        "Geometry",
        ONE,
        7.0,
        &[
            P::new("Horizontal", 0.0, 1.0, 1.0).choices(crate::choices::SWITCH),
            P::new("Vertical", 0.0, 1.0, 0.0).choices(crate::choices::SWITCH)
        ]
    );
    visual!(
        r,
        "Displace",
        "Displace",
        "Geometry",
        &[Port::texture("Image"), Port::texture("Displacement")],
        8.0,
        &[
            P::new("X amount", 0.0, 1.0, 0.1),
            P::new("Y amount", 0.0, 1.0, 0.1)
        ]
    );
    visual!(
        r,
        "Levels",
        "Levels / Gamma",
        "Color",
        ONE,
        9.0,
        &[
            P::new("Black", 0.0, 0.99, 0.0),
            P::new("White", 0.01, 1.0, 1.0),
            P::new("Gamma", 0.1, 4.0, 1.0)
        ]
    );
    visual!(
        r,
        "Threshold",
        "Threshold",
        "Effects",
        ONE,
        10.0,
        &[
            P::new("Threshold", 0.0, 1.0, 0.5),
            P::new("Softness", 0.001, 0.5, 0.01)
        ]
    );
    visual!(
        r,
        "Vignette",
        "Vignette",
        "Effects",
        ONE,
        11.0,
        &[
            P::new("Radius", 0.0, 1.0, 0.4),
            P::new("Softness", 0.001, 1.0, 0.3),
            P::new("Strength", 0.0, 1.0, 0.7)
        ]
    );
}
