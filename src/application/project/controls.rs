use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ControlBinding {
    pub id: u64,
    pub node: NodeId,
    pub parameter: usize,
    pub position: Point,
    pub size: Point,
    #[serde(default)]
    pub widget: WidgetKind,
    #[serde(default)]
    pub target: ControlTarget,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub range: Option<[f32; 2]>,
    #[serde(
        default,
        skip_serializing_if = "crate::value_display::ControlDisplay::is_default"
    )]
    pub display: crate::value_display::ControlDisplay,
}
pub use vjay_domain::capabilities::WidgetKind;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControlTarget {
    #[default]
    Parameter,
    Output,
}
impl Default for ControlBinding {
    fn default() -> Self {
        Self {
            id: 0,
            node: NodeId(0),
            parameter: 0,
            position: Point { x: 12.0, y: 12.0 },
            size: Point { x: 220.0, y: 110.0 },
            widget: WidgetKind::Slider,
            target: ControlTarget::Parameter,
            label: String::new(),
            range: None,
            display: Default::default(),
        }
    }
}

pub fn valid_control(
    control: &ControlBinding,
    node: &NodeRecord,
    factory: &dyn NodeFactory,
) -> bool {
    let Ok(definition) = factory.get(&node.type_id) else {
        return false;
    };
    let descriptor = definition.schema(&node.settings);
    let valid = match control.target {
        ControlTarget::Parameter => control.parameter < descriptor.parameters.len(),
        ControlTarget::Output => descriptor.outputs.get(control.parameter).is_some_and(|p| {
            p.signal == vjay_domain::port::SignalType::Scalar
                || (p.signal == vjay_domain::port::SignalType::Text
                    && control.widget == WidgetKind::Label)
        }),
    };
    let range_valid = control.range.is_none_or(|[a, b]| {
        a.is_finite()
            && b.is_finite()
            && a < b
            && a >= -1e6
            && b <= 1e6
            && (control.target == ControlTarget::Output
                || descriptor
                    .parameters
                    .get(control.parameter)
                    .is_some_and(|p| a >= p.min && b <= p.max))
    });
    valid
        && [control.display.output, control.display.total_output]
            .into_iter()
            .all(|index| {
                index.is_none_or(|i| {
                    descriptor
                        .outputs
                        .get(i)
                        .is_some_and(|p| p.signal == vjay_domain::port::SignalType::Scalar)
                })
            })
        && (control.widget != WidgetKind::Files
            || (control.target == ControlTarget::Parameter
                && descriptor.capabilities.asset_selection_parameter == Some(control.parameter)))
        && range_valid
        && control.label.len() <= 256
        && control
            .range
            .is_none_or(|[a, b]| a.is_finite() && b.is_finite() && a < b && a >= -1e6 && b <= 1e6)
        && (control.widget != WidgetKind::XYPad
            || control.parameter + 1
                < if control.target == ControlTarget::Parameter {
                    descriptor.parameters.len()
                } else {
                    descriptor.outputs.len()
                })
}
