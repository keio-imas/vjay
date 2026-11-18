use super::*;
#[derive(Clone)]
enum ChoiceTarget {
    Parameter(usize),
    Property(String),
}
pub(super) struct ParameterMenu {
    node: NodeId,
    target: ChoiceTarget,
    screen: Position,
}
impl Editor {
    pub(in crate::ui) fn trigger_parameter(
        &self,
        node: NodeId,
        parameter: usize,
        driven: bool,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        button(
            SharedString::from(format!("parameter-trigger-{}-{parameter}", node.0)),
            if driven {
                "Signal controlled"
            } else {
                "Trigger"
            },
        )
        .debug_selector(move || format!("parameter-trigger-{}-{parameter}", node.0))
        .text_xs()
        .bg(rgb(BACKGROUND))
        .border_1()
        .border_color(rgb(BORDER))
        .when(!driven, |e| {
            e.on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, window, cx| {
                    window.focus(&this.focus);
                    let r = this.session_mut().set_parameter(node, parameter, 1.0);
                    this.apply(r);
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    let r = this.session_mut().set_parameter(node, parameter, 0.0);
                    this.apply(r);
                    cx.notify();
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    let r = this.session_mut().set_parameter(node, parameter, 0.0);
                    this.apply(r);
                    cx.notify();
                }),
            )
        })
        .into_any_element()
    }
    pub(in crate::ui) fn choice_parameter(
        &self,
        node: NodeId,
        parameter: usize,
        value: f32,
        driven: bool,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        let info = &self.session().schema(node).parameters[parameter];
        button(
            SharedString::from(format!("choice-{}-{parameter}", node.0)),
            info.choice_label(value).unwrap_or("Unknown"),
        )
        .debug_selector(move || format!("choice-{}-{parameter}", node.0))
        .w_full()
        .flex()
        .items_center()
        .text_xs()
        .bg(rgb(BACKGROUND))
        .border_1()
        .border_color(rgb(BORDER))
        .child(div().flex_1())
        .child(
            div()
                .text_color(rgb(MUTED))
                .child(if driven { "Signal" } else { "▾" }),
        )
        .when(!driven, |e| {
            e.on_click(cx.listener(move |this, _, window, cx| {
                this.parameter_menu = Some(ParameterMenu {
                    node,
                    target: ChoiceTarget::Parameter(parameter),
                    screen: position(window.mouse_position()),
                });
                cx.notify();
            }))
        })
        .into_any_element()
    }
    pub(in crate::ui) fn choice_property(
        &self,
        node: NodeId,
        key: &str,
        label: &str,
        value: &str,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        let owned = key.to_owned();
        let selector = owned.clone();
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(MUTED))
                    .child(label.to_owned()),
            )
            .child(
                button(
                    SharedString::from(format!("property-choice-{key}")),
                    value.to_owned(),
                )
                .debug_selector(move || format!("property-choice-{selector}"))
                .flex()
                .items_center()
                .text_xs()
                .bg(rgb(BACKGROUND))
                .border_1()
                .border_color(rgb(BORDER))
                .child(div().flex_1())
                .child("▾")
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.parameter_menu = Some(ParameterMenu {
                        node,
                        target: ChoiceTarget::Property(owned.clone()),
                        screen: position(window.mouse_position()),
                    });
                    cx.notify();
                })),
            )
            .into_any_element()
    }
    fn apply_choice(
        &mut self,
        node: NodeId,
        target: &ChoiceTarget,
        label: &str,
        value: f32,
    ) -> PortResult<()> {
        match target {
            ChoiceTarget::Parameter(parameter) => {
                self.session_mut().set_parameter(node, *parameter, value)
            }
            ChoiceTarget::Property(key) => {
                let mut settings = self
                    .session()
                    .project()
                    .graph
                    .node(node)
                    .map_err(|e| e.to_string())?
                    .settings
                    .clone();
                settings.properties.insert(key.clone(), label.into());
                self.session_mut().set_settings(node, settings)
            }
        }
    }
    pub(super) fn parameter_popup(
        &self,
        cx: &Context<Self>,
        width: f32,
        height: f32,
    ) -> Option<gpui::AnyElement> {
        let menu = self.parameter_menu.as_ref()?;
        let node = menu.node;
        let target = menu.target.clone();
        let schema = self.session().project().graph.schema(node).ok()?;
        let (choices, minimum) = match &target {
            ChoiceTarget::Parameter(index) => {
                let info = schema.parameters.get(*index)?;
                (info.choices, info.min)
            }
            ChoiceTarget::Property(key) => {
                let info = schema.properties.iter().find(|p| p.key == *key)?;
                (info.choices, 0.0)
            }
        };
        Some(
            div()
                .id("parameter-menu")
                .debug_selector(|| "parameter-menu".into())
                .absolute()
                .left(px(
                    (menu.screen.x - 190.0).clamp(0.0, (width - 250.0).max(0.0))
                ))
                .top(px(menu.screen.y.min(height - 270.0).max(0.0)))
                .w(px(240.0))
                .max_h(px(250.0))
                .flex()
                .flex_col()
                .p_1()
                .bg(rgb(ELEVATED))
                .border_1()
                .border_color(rgb(BORDER))
                .shadow_lg()
                .overflow_y_scroll()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                    this.parameter_menu = None;
                    cx.notify();
                }))
                .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
                .children(choices.iter().enumerate().map(|(index, &label)| {
                    let value = minimum + index as f32;
                    let target = target.clone();
                    button(SharedString::from(format!("option-{index}")), label)
                        .debug_selector(move || format!("option-{index}"))
                        .text_xs()
                        .flex_shrink_0()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let result = this.apply_choice(node, &target, label, value);
                            this.apply(result);
                            this.parameter_menu = None;
                            cx.notify();
                        }))
                }))
                .into_any_element(),
        )
    }
}
