use serde::{Deserialize, Serialize};

/// Presentation only: control ranges and signal values retain their original units.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValueFormat {
    #[default]
    Number,
    Timecode,
}
impl ValueFormat {
    pub const ALL: [Self; 2] = [Self::Number, Self::Timecode];
    pub fn name(self) -> &'static str {
        match self {
            Self::Number => "Number",
            Self::Timecode => "hh:mm:ss.fff",
        }
    }
    pub fn format(self, value: f64, precision: usize) -> String {
        if !value.is_finite() {
            return "—".into();
        }
        match self {
            Self::Number => format!("{value:.precision$}", precision = precision.min(9)),
            Self::Timecode => {
                let ms = (value.abs() * 1000.0).round() as u64;
                let sign = if value.is_sign_negative() && ms > 0 {
                    "-"
                } else {
                    ""
                };
                format!(
                    "{sign}{:02}:{:02}:{:02}.{:03}",
                    ms / 3_600_000,
                    ms / 60_000 % 60,
                    ms / 1000 % 60,
                    ms % 1000
                )
            }
        }
    }
}

/// Readout sources are scalar outputs of the bound node. Values arrive through
/// ordinary graph wires, so copied and nested controls never reference a peer's internals.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ControlDisplay {
    pub format: ValueFormat,
    pub output: Option<usize>,
    pub total_output: Option<usize>,
}
impl ControlDisplay {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timecode_rounds_before_splitting_and_keeps_hours_and_sign() {
        let f = |v| ValueFormat::Timecode.format(v, 3);
        assert_eq!(f(0.0), "00:00:00.000");
        assert_eq!(f(3661.234), "01:01:01.234");
        assert_eq!(f(59.9996), "00:01:00.000");
        assert_eq!(f(3599.9996), "01:00:00.000");
        assert_eq!(f(-0.01), "-00:00:00.010");
        assert_eq!(f(-0.0001), "00:00:00.000");
        assert_eq!(f(360_000.0), "100:00:00.000");
        assert_eq!(f(f64::NAN), "—");
        assert_eq!(f(f64::INFINITY), "—");
        assert_eq!(ValueFormat::Number.format(0.5, 3), "0.500");
    }
}
