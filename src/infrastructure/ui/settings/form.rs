use super::*;
fn heading(title: &'static str, detail: &'static str) -> Div {
    div()
        .mb_4()
        .child(
            div()
                .text_lg()
                .font_weight(FontWeight::SEMIBOLD)
                .child(title),
        )
        .child(div().mt_1().text_xs().text_color(rgb(MUTED)).child(detail))
}
impl Editor {
    pub(super) fn settings_form(&self, cx: &Context<Self>) -> gpui::AnyElement {
        let mut form = div().flex().flex_col();
        match self.settings.as_ref().expect("settings").tab {
            0 => {
                form = form.child(heading(
                    "Audio output",
                    "Choose the device used by Audio Out and headphone monitoring.",
                ));
                for (key, label, menu) in [
                    ("device", "Output device", true),
                    ("channels", "Channel pair", true),
                    ("sample_rate", "Sample rate", true),
                    ("buffer", "Buffer size", true),
                    ("volume", "Master volume (%)", false),
                ] {
                    form = form.child(self.settings_field(key, label, menu, cx));
                }
                form=form.child(div().mt_4().pt_3().border_t_1().border_color(rgb(BORDER)).text_xs().text_color(rgb(MUTED))
                    .child("Device changes restart the audio stream. Unsupported rates or channels are reported before saving."));
            }
            1 => {
                form = form.child(heading(
                    "Video output",
                    "Render size, frame rate, and the default destination for Output windows.",
                ));
                for (key, label, menu) in [
                    ("display", "Output display", true),
                    ("fullscreen", "Window mode", true),
                    ("width", "Render width", false),
                    ("height", "Render height", false),
                    ("frame_rate", "Frame rate", true),
                    ("vsync", "Vertical sync", true),
                ] {
                    form = form.child(self.settings_field(key, label, menu, cx));
                }
                form=form.child(div().mt_4().pt_3().border_t_1().border_color(rgb(BORDER)).text_xs().text_color(rgb(MUTED))
                    .child("Apply updates rendering and reopens existing Output windows on the chosen display. Each Output keeps its own feed."));
            }
            _ => {
                form=form.child(heading("Projects","Defaults for newly created projects."))
                    .child(self.settings_field("name","Default file name",false,cx))
                    .child(div().mt_4().pt_3().border_t_1().border_color(rgb(BORDER)).text_xs().text_color(rgb(MUTED))
                        .child("Settings are stored separately from project files. Save As still lets you choose any name and location."));
            }
        }
        form.into_any_element()
    }
}
