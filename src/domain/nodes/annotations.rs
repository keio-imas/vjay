use crate::{
    capabilities::Capabilities as C,
    node::{Node, NodeDescriptor as D},
    registry::NodeRegistry,
    settings::NodeSettings,
};
struct Annotation(&'static D);
impl Node for Annotation {
    fn descriptor(&self) -> &'static D {
        self.0
    }
    fn default_settings(&self) -> NodeSettings {
        NodeSettings {
            comment: self.0.name.into(),
            ..Default::default()
        }
    }
}
pub(crate) fn register(registry: &mut NodeRegistry) {
    static COMMENT: D = D {
        type_id: "Comment",
        name: "Comment",
        category: "Organization",
        inputs: &[],
        outputs: &[],
        parameters: &[],
        capabilities: C {
            annotation: true,
            ..C::NONE
        },
    };
    static LABEL: D = D {
        type_id: "Label",
        name: "Label",
        category: "Organization",
        inputs: &[],
        outputs: &[],
        parameters: &[],
        capabilities: C {
            annotation: true,
            ..C::NONE
        },
    };
    registry
        .register(Annotation(&COMMENT))
        .expect("valid comment");
    registry.register(Annotation(&LABEL)).expect("valid label");
}
