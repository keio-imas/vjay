use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WidgetKind {
    #[default]
    Slider,
    Knob,
    Toggle,
    Button,
    XYPad,
    Number,
    Meter,
    Label,
    Files,
}
impl WidgetKind {
    pub const ALL: [Self; 9] = [
        Self::Slider,
        Self::Knob,
        Self::Toggle,
        Self::Button,
        Self::XYPad,
        Self::Number,
        Self::Meter,
        Self::Label,
        Self::Files,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Slider => "Slider",
            Self::Knob => "Knob",
            Self::Toggle => "Toggle",
            Self::Button => "Button",
            Self::XYPad => "XY pad",
            Self::Number => "Number",
            Self::Meter => "Meter",
            Self::Label => "Label",
            Self::Files => "Files",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MediaBinding {
    pub speed: usize,
    pub position: usize,
    pub looping: Option<usize>,
    pub start: Option<usize>,
    pub end: Option<usize>,
    pub seek: Option<usize>,
}
#[derive(Clone, Copy, Debug)]
pub struct AudioInput {
    pub input: usize,
    pub gain: Option<usize>,
}
/// Routing primitives. Nodes compose routes independently of their identity.
#[derive(Clone, Copy, Debug)]
pub enum AudioRecipe {
    Source,
    Mix(&'static [AudioInput]),
    Sink { input: usize, gain: usize },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingEditor {
    Fragment,
    Keyboard,
    Midi,
    Osc,
}
#[derive(Clone, Copy, Debug)]
pub struct ManualSyncBinding {
    pub mode: usize,
    pub rate: usize,
    pub range: usize,
    pub marker: usize,
}
#[derive(Clone, Copy, Debug)]
pub struct TempoSyncBinding {
    pub manual: Option<ManualSyncBinding>,
    pub clip_bpm: usize,
    pub dj_bpm: usize,
    pub trim: usize,
    pub nudge: usize,
    pub cue: usize,
    pub duration: usize,
    pub align: usize,
    pub nudge_step: usize,
    pub outputs: [usize; 3],
}
#[derive(Clone, Copy, Debug)]
pub struct RateBinding {
    pub mode: usize,
    pub rate: usize,
    pub range: usize,
    pub clip_bpm: usize,
    pub dj_bpm: usize,
    pub trim: usize,
    pub speed_output: usize,
}
#[derive(Clone, Copy, Debug)]
pub struct ValueControlBinding {
    pub parameter: usize,
    pub feedback_input: usize,
    pub request_output: usize,
    pub display_output: usize,
    pub request_trigger_output: Option<usize>,
    pub readout: Option<ValueReadoutBinding>,
}
#[derive(Clone, Copy, Debug)]
pub struct ValueReadoutBinding {
    pub inputs: [usize; 2],
    pub outputs: [usize; 2],
}
#[derive(Clone, Copy, Debug)]
pub struct Capabilities {
    pub rate: Option<RateBinding>,
    pub value_control: Option<ValueControlBinding>,
    pub transport: Option<crate::transport::TransportBinding>,
    pub asset_selection_parameter: Option<usize>,
    pub tempo_sync: Option<TempoSyncBinding>,
    pub audio_cue: bool,
    pub file_input: Option<usize>,
    pub still_image: bool,
    pub annotation: bool,
    pub program_output: Option<usize>,
    pub media: Option<MediaBinding>,
    pub audio: Option<AudioRecipe>,
    pub widget: Option<WidgetKind>,
    pub range_parameters: Option<[usize; 2]>,
    pub momentary_parameter: Option<usize>,
    pub settings: &'static [SettingEditor],
    pub tap: bool,
}
impl Capabilities {
    pub const NONE: Self = Self {
        rate: None,
        value_control: None,
        transport: None,
        asset_selection_parameter: None,
        tempo_sync: None,
        audio_cue: false,
        file_input: None,
        still_image: false,
        annotation: false,
        program_output: None,
        media: None,
        audio: None,
        widget: None,
        range_parameters: None,
        momentary_parameter: None,
        settings: &[],
        tap: false,
    };
}
