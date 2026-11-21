use crate::{
    capabilities::{Capabilities, WidgetKind},
    execution::ScalarKernel,
    node::{Node, NodeDescriptor, Parameter},
    port::Port,
    registry::NodeRegistry,
};
static D: NodeDescriptor = NodeDescriptor {
    type_id: "SeekControl",
    name: "Seek Control",
    category: "Controls",
    inputs: &[
        Port::scalar("Playhead").optional(),
        Port::scalar("Readout value").optional(),
        Port::scalar("Readout total").optional(),
    ],
    outputs: &[
        Port::scalar("Seek"),
        Port::scalar("Display"),
        Port::scalar("Request"),
        Port::scalar("Readout value"),
        Port::scalar("Readout total"),
    ],
    parameters: &[Parameter::new("Seek position", 0.0, 1.0, 0.0)],
    capabilities: Capabilities {
        widget: Some(WidgetKind::Slider),
        value_control: Some(crate::capabilities::ValueControlBinding {
            parameter: 0,
            feedback_input: 0,
            request_output: 0,
            display_output: 1,
            request_trigger_output: Some(2),
            readout: Some(crate::capabilities::ValueReadoutBinding {
                inputs: [1, 2],
                outputs: [3, 4],
            }),
        }),
        ..Capabilities::NONE
    },
};
pub(crate) fn register(r: &mut NodeRegistry) {
    r.register(SeekControl).expect("valid seek control");
}
struct SeekControl;
impl Node for SeekControl {
    fn descriptor(&self) -> &'static NodeDescriptor {
        &D
    }
    fn port_version(&self) -> u32 {
        1
    }
    fn migrate_input_port(&self, input: usize, from: u32) -> usize {
        input.saturating_add(if from == 0 && input >= 1 { 2 } else { 0 })
    }
    fn scalar(&self) -> Option<ScalarKernel> {
        Some(ScalarKernel {
            state: Vec::new(),
            tap: None,
            evaluate: |c, _, o| {
                o[0] = c.parameters[0];
                o[1] = c.inputs[0].unwrap_or(c.parameters[0]).clamp(0.0, 1.0);
                o[2] = f32::from(c.host.trigger(c.node));
                o[3] = c.inputs[1].unwrap_or(o[1]);
                o[4] = c.inputs[2].unwrap_or(0.0);
            },
        })
    }
}
