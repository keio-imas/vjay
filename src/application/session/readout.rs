use super::*;
impl Session {
    pub fn control_readout(&self, control: &ControlBinding, precision: usize) -> String {
        let value = control.display.output.map_or_else(
            || Some(self.control_value(control)),
            |output| self.signal_value(control.node, output),
        );
        let format = control.display.format;
        let text = format.format(value.map_or(f64::NAN, f64::from), precision);
        control.display.total_output.map_or_else(
            || text.clone(),
            |output| {
                format!(
                    "{text} / {}",
                    format.format(
                        self.signal_value(control.node, output)
                            .map_or(f64::NAN, f64::from),
                        precision
                    )
                )
            },
        )
    }
}
