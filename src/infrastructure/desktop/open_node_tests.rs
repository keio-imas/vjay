//! An extension defined outside domain/application, using only public contracts.
use vjay_domain::{
    capabilities::Capabilities, execution::*, node::*, port::Port, settings::NodeSettings,
};
pub(crate) const TYPE_ID: &str = "test.open-extension.v1";
pub(crate) const PROGRAM: &str = "test.open-extension.color.v1";
pub(crate) const ALTERNATE: &str = "test.open-extension.color.v2";
pub(crate) struct Extension;
static DESCRIPTOR: NodeDescriptor = NodeDescriptor {
    type_id: TYPE_ID,
    name: "Open extension",
    category: "Extensions",
    inputs: &[Port::texture("Image").optional()],
    outputs: &[
        Port::texture("Image"),
        Port::scalar("Time"),
        Port::scalar("Level"),
        Port::scalar("A"),
        Port::scalar("B"),
        Port::scalar("C"),
        Port::scalar("D"),
    ],
    parameters: &[Parameter::new("Level", 0.0, 1.0, 0.25)],
    capabilities: Capabilities::NONE,
};
impl Node for Extension {
    fn descriptor(&self) -> &'static NodeDescriptor {
        &DESCRIPTOR
    }
    fn default_settings(&self) -> NodeSettings {
        let mut settings = NodeSettings::default();
        settings
            .properties
            .insert("channel".into(), "test.clock".into());
        settings
    }
    fn validate_settings(&self, settings: &NodeSettings) -> bool {
        settings.is_valid()
            && settings
                .properties
                .get("channel")
                .is_some_and(|c| c.starts_with("test."))
    }
    fn image(&self, p: &[f32], _: &NodeSettings, _: usize) -> Option<ImageKernel> {
        Some(ImageKernel {
            shader: ShaderSource::Registered(if p[0] > 0.5 { ALTERNATE } else { PROGRAM }),
            selector: 0.0,
            parameters: vec![p[0], 0.5, 0.75, 0.0],
            parameter_slots: vec![Some(0), None, None, None],
            prepared: None,
            stream: None,
            media: None,
        })
    }
    fn scalar(&self) -> Option<ScalarKernel> {
        Some(ScalarKernel {
            evaluate,
            state: vec![0.0],
            tap: None,
        })
    }
}
fn evaluate(c: &ScalarContext<'_>, state: &mut [f64], out: &mut [f32]) {
    state[0] += c.delta;
    let channel = &c.settings.properties["channel"];
    for (index, value) in out.iter_mut().enumerate() {
        *value = c.host.external(channel, index);
    }
    out[0] = state[0] as f32;
    out[1] = c.parameters[0];
}
mod behavior;
