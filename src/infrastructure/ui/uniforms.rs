use super::*;
use configuration::Target;
impl Editor {
    pub(super) fn uniform_editor(&self, id: NodeId, cx: &Context<Self>) -> gpui::AnyElement {
        let uniforms = self.session().shader_uniforms(id).unwrap_or_default();
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(MUTED))
                    .child(format!("Uniforms · {} / 64", uniforms.len())),
            )
            .children(uniforms.iter().enumerate().map(|(index, u)| {
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        button(
                            SharedString::from(format!("uniform-name-{index}")),
                            u.name.clone(),
                        )
                        .debug_selector(move || format!("uniform-name-{index}"))
                        .flex_1()
                        .text_xs()
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.open_configuration(Target::Uniform(id, index), window, cx)
                            },
                        )),
                    )
                    .child(
                        button(SharedString::from(format!("remove-uniform-{index}")), "×")
                            .px_1()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let result =
                                    this.session().shader_uniforms(id).and_then(|mut uniforms| {
                                        uniforms.remove(index);
                                        this.session_mut().set_shader_uniforms(id, uniforms)
                                    });
                                this.apply(result);
                                cx.notify();
                            })),
                    )
            }))
            .child(
                button("add-uniform", "+ Uniform")
                    .debug_selector(|| "add-uniform".into())
                    .text_xs()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let result = this.session().shader_uniforms(id).and_then(|mut uniforms| {
                            if uniforms.len() >= 64 {
                                return Err("Maximum 64 scalar uniforms".into());
                            }
                            let mut index = 1;
                            while uniforms
                                .iter()
                                .any(|u| u.name == format!("uniform_{index}"))
                            {
                                index += 1;
                            }
                            uniforms.push(vjay_domain::uniform::ShaderUniform {
                                name: format!("uniform_{index}"),
                                min: -1000.0,
                                max: 1000.0,
                                default: 0.0,
                            });
                            this.session_mut().set_shader_uniforms(id, uniforms)
                        });
                        this.apply(result);
                        cx.notify();
                    })),
            )
            .into_any_element()
    }
}
