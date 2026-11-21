use super::*;
use vjay_application::{project::ControlBinding, value_display::ValueFormat};
use vjay_domain::port::SignalType;

#[derive(Clone, Copy)]
pub(super) enum Field {
    Format,
    Value,
    Total,
}

impl Editor {
    pub(super) fn control_display_settings(
        &self,
        control: &ControlBinding,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        let schema = self.session().schema(control.node);
        let output_name = |output: Option<usize>, fallback: &str| {
            output
                .and_then(|i| schema.outputs.get(i))
                .map_or_else(|| fallback.to_owned(), |p| p.name.clone())
        };
        let id = control.id;
        let node = control.node;
        div().flex().flex_col().gap_2()
            .pt_3().pb_3().border_t_1().border_b_1().border_color(rgb(BORDER))
            .child(div().text_xs().font_weight(FontWeight::MEDIUM).child("Value display"))
            .children([
                (Field::Format, "format", "Format", control.display.format.name().to_owned()),
                (Field::Value, "value", "Readout", output_name(control.display.output, "Control value")),
                (Field::Total, "total", "Total", output_name(control.display.total_output, "None")),
            ].into_iter().map(|(field, key, label, value)| {
                div().flex().items_center().gap_2()
                    .child(div().w(px(64.0)).text_xs().text_color(rgb(MUTED)).child(label))
                    .child(button(SharedString::from(format!("display-{key}")), value)
                        .debug_selector(move || format!("display-{key}"))
                        .flex_1().min_w_0().flex().items_center().text_xs()
                        .bg(rgb(BACKGROUND)).border_1().border_color(rgb(BORDER))
                        .child(div().flex_1()).child("▾")
                        .on_click(cx.listener(move |this, _, window, cx| this.open_display_menu(id, node, field, window, cx))))
            }))
            .child(div().text_xs().text_color(rgb(MUTED)).child("Timecode reads seconds. Slider range and signal values stay unchanged. Readouts can use any scalar output of this node."))
            .into_any_element()
    }

    pub(super) fn display_choices(&self, id: u64, field: Field) -> Vec<(String, f32)> {
        if matches!(field, Field::Format) {
            return ValueFormat::ALL
                .into_iter()
                .enumerate()
                .map(|(i, f)| (f.name().into(), i as f32))
                .collect();
        }
        let mut choices = vec![(
            if matches!(field, Field::Value) {
                "Control value"
            } else {
                "None"
            }
            .into(),
            0.0,
        )];
        if let Some(control) = self
            .session()
            .project()
            .controls
            .iter()
            .find(|c| c.id == id)
        {
            choices.extend(
                self.session()
                    .schema(control.node)
                    .outputs
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| p.signal == SignalType::Scalar)
                    .map(|(i, p)| (p.name.clone(), (i + 1) as f32)),
            );
        }
        choices
    }

    pub(super) fn apply_display_choice(
        &mut self,
        id: u64,
        field: Field,
        value: f32,
    ) -> PortResult<()> {
        let mut control = self
            .session()
            .project()
            .controls
            .iter()
            .find(|c| c.id == id)
            .cloned()
            .ok_or("Control no longer exists")?;
        match field {
            Field::Format => {
                control.display.format = *ValueFormat::ALL
                    .get(value as usize)
                    .ok_or("Invalid value format")?
            }
            Field::Value => control.display.output = (value > 0.0).then(|| value as usize - 1),
            Field::Total => {
                control.display.total_output = (value > 0.0).then(|| value as usize - 1)
            }
        }
        self.session_mut().configure_control(control)
    }
}
