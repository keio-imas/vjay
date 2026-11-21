use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct WorkspaceLayout {
    #[serde(default = "default_zoom")]
    pub patch_zoom: f32,
    pub library_width: f32,
    pub inspector_width: f32,
    pub top_height: f32,
    pub preview_ratio: f32,
}
impl Default for WorkspaceLayout {
    fn default() -> Self {
        Self {
            patch_zoom: 1.0,
            library_width: 230.0,
            inspector_width: 280.0,
            top_height: 284.0,
            preview_ratio: 0.5,
        }
    }
}
impl WorkspaceLayout {
    pub fn is_valid(self) -> bool {
        [
            self.patch_zoom,
            self.library_width,
            self.inspector_width,
            self.top_height,
            self.preview_ratio,
        ]
        .iter()
        .all(|v| v.is_finite())
            && (0.25..=4.0).contains(&self.patch_zoom)
            && (120.0..=600.0).contains(&self.library_width)
            && (220.0..=650.0).contains(&self.inspector_width)
            && (140.0..=1400.0).contains(&self.top_height)
            && (0.2..=0.8).contains(&self.preview_ratio)
    }
}

fn default_zoom() -> f32 {
    1.0
}
