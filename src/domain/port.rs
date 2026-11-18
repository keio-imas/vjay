use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SignalType {
    Texture,
    Scalar,
    Audio,
    Text,
    File,
}

#[derive(Clone, Copy, Debug)]
pub struct Port {
    pub name: &'static str,
    pub signal: SignalType,
    pub optional: bool,
}
impl Port {
    pub const fn texture(name: &'static str) -> Self {
        Self {
            name,
            signal: SignalType::Texture,
            optional: false,
        }
    }
    pub const fn scalar(name: &'static str) -> Self {
        Self {
            name,
            signal: SignalType::Scalar,
            optional: false,
        }
    }
    pub const fn audio(name: &'static str) -> Self {
        Self {
            name,
            signal: SignalType::Audio,
            optional: false,
        }
    }
    pub const fn text(name: &'static str) -> Self {
        Self {
            name,
            signal: SignalType::Text,
            optional: false,
        }
    }
    pub const fn file(name: &'static str) -> Self {
        Self {
            name,
            signal: SignalType::File,
            optional: false,
        }
    }
    pub const fn optional(mut self) -> Self {
        self.optional = true;
        self
    }
}
