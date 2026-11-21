use super::*;
use vjay_application::project::{ControlBinding, ControlTarget, WidgetKind, valid_control};
use vjay_domain::port::SignalType;
struct Candidate {
    label: String,
    search: String,
    binding: ControlBinding,
}
pub(super) struct BindingPicker {
    search: Entity<text_field::TextField>,
    entries: Vec<Candidate>,
    matches: Vec<usize>,
    screen: Position,
}
impl Editor {
    pub(super) fn open_binding_picker(
        &mut self,
        id: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(control) = self
            .session()
            .project()
            .controls
            .iter()
            .find(|c| c.id == id)
        else {
            return;
        };
        let mut entries = Vec::new();
        for node in self.session().project().graph.nodes() {
            let schema = self.session().schema(node.id);
            let targets = schema
                .parameters
                .iter()
                .enumerate()
                .map(|(p, v)| (p, ControlTarget::Parameter, v.name.as_str()))
                .chain(
                    schema
                        .outputs
                        .iter()
                        .enumerate()
                        .filter(|(_, p)| matches!(p.signal, SignalType::Scalar | SignalType::Text))
                        .map(|(p, v)| (p, ControlTarget::Output, v.name.as_str())),
                );
            for (parameter, target, name) in targets {
                let mut binding = control.clone();
                binding.node = node.id;
                binding.parameter = parameter;
                binding.target = target;
                binding.range = None;
                binding.display = Default::default();
                if target == ControlTarget::Output {
                    binding.widget = if schema.outputs[parameter].signal == SignalType::Text {
                        WidgetKind::Label
                    } else {
                        WidgetKind::Meter
                    };
                } else if control.target == ControlTarget::Output {
                    binding.widget = WidgetKind::Slider;
                }
                if !valid_control(&binding, node, self.session().node_factory()) {
                    continue;
                }
                let label = format!(
                    "{} #{} · {}{}",
                    schema.name,
                    node.id.0,
                    if target == ControlTarget::Output {
                        "↗ "
                    } else {
                        ""
                    },
                    name
                );
                entries.push(Candidate {
                    search: label.to_lowercase(),
                    label,
                    binding,
                });
            }
        }
        let search = cx.new(|cx| text_field::TextField::new(String::new(), false, cx));
        cx.observe(&search, |this, _, cx| {
            if let Some(p) = &mut this.binding_picker {
                let q = p.search.read(cx).text.to_lowercase();
                p.matches = p
                    .entries
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| e.search.contains(&q))
                    .map(|(i, _)| i)
                    .collect();
            }
            cx.notify();
        })
        .detach();
        window.focus(&search.read(cx).focus);
        let matches = (0..entries.len()).collect();
        self.binding_picker = Some(BindingPicker {
            search,
            entries,
            matches,
            screen: position(window.mouse_position()),
        });
        cx.notify();
    }
    pub(super) fn binding_search_focused(&self, window: &Window, cx: &Context<Self>) -> bool {
        self.binding_picker
            .as_ref()
            .is_some_and(|p| p.search.read(cx).focus.is_focused(window))
    }
    pub(super) fn binding_popup(
        &self,
        cx: &Context<Self>,
        width: f32,
        height: f32,
    ) -> Option<gpui::AnyElement> {
        let p = self.binding_picker.as_ref()?;
        Some(
            div()
                .id("binding-picker")
                .debug_selector(|| "binding-picker".into())
                .absolute()
                .left(px((p.screen.x - 260.0).clamp(0.0, (width - 380.0).max(0.0))))
                .top(px(p.screen.y.clamp(0.0, (height - 400.0).max(0.0))))
                .w(px(360.0))
                .h(px(370.0))
                .flex()
                .flex_col()
                .p_2()
                .gap_2()
                .bg(rgb(SURFACE))
                .border_1()
                .border_color(rgb(BORDER))
                .shadow_lg()
                .on_mouse_down_out(cx.listener(|this, _, window, cx| {
                    this.binding_picker = None;
                    window.focus(&this.focus);
                    cx.notify();
                }))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
                .child("Choose binding target")
                .child(p.search.clone())
                .child(
                    gpui::uniform_list(
                        "binding-candidates",
                        p.matches.len(),
                        cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                            let Some(p) = &this.binding_picker else {
                                return Vec::new();
                            };
                            range
                                .map(|i| {
                                    let entry = &p.entries[p.matches[i]];
                                    let binding = entry.binding.clone();
                                    div()
                                        .id(i)
                                        .debug_selector(move || format!("binding-candidate-{i}"))
                                        .h(px(30.0))
                                        .px_2()
                                        .flex()
                                        .items_center()
                                        .text_xs()
                                        .truncate()
                                        .cursor_pointer()
                                        .hover(|s| s.bg(rgb(ELEVATED)))
                                        .child(entry.label.clone())
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            let result = this
                                                .session_mut()
                                                .configure_control(binding.clone());
                                            this.apply(result);
                                            this.binding_picker = None;
                                            window.focus(&this.focus);
                                            cx.notify();
                                        }))
                                })
                                .collect::<Vec<_>>()
                        }),
                    )
                    .flex_1()
                    .min_h_0(),
                )
                .into_any_element(),
        )
    }
}
