use super::*;

#[derive(Clone)]
pub(super) enum Target {
    OscEndpoint,
    Shader(NodeId),
    Uniform(NodeId, usize),
    Comment(NodeId),
    Key(NodeId),
    Osc(NodeId),
    NodeLabel(NodeId),
    ControlLabel(u64),
    ControlRange(u64),
    Number(NodeId, usize),
    Property(NodeId, String),
}
pub(super) struct Configuration {
    target: Target,
    input: Entity<text_field::TextField>,
    error: String,
}
impl Editor {
    pub(super) fn text_editing(&self, window: &Window, cx: &Context<Self>) -> bool {
        self.palette
            .as_ref()
            .is_some_and(|p| p.search.read(cx).focus.is_focused(window))
            || self
                .search
                .as_ref()
                .is_some_and(|input| input.read(cx).focus.is_focused(window))
            || self
                .configuration
                .as_ref()
                .is_some_and(|config| config.input.read(cx).focus.is_focused(window))
    }

    pub(super) fn open_configuration(
        &mut self,
        target: Target,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let text = match target {
            Target::Uniform(id, index) => self
                .session()
                .shader_uniforms(id)
                .ok()
                .and_then(|v| v.get(index).cloned())
                .map_or_else(String::new, |u| {
                    format!("{}, {}, {}, {}", u.name, u.min, u.max, u.default)
                }),
            Target::Comment(id) => self
                .session()
                .project()
                .graph
                .node(id)
                .map_or_else(|_| String::new(), |n| n.settings.comment.clone()),
            Target::Property(id, ref key) => self
                .session()
                .project()
                .graph
                .node(id)
                .ok()
                .and_then(|node| node.settings.properties.get(key))
                .cloned()
                .unwrap_or_default(),
            Target::OscEndpoint => format!(
                "{}:{}",
                self.session().input_settings.osc_host,
                self.session().input_settings.osc_port.unwrap_or(9000)
            ),
            Target::Shader(id) => self
                .session()
                .project()
                .graph
                .node(id)
                .map(|n| n.settings.shader.clone())
                .unwrap_or_default(),
            Target::Key(id) => self
                .session()
                .project()
                .graph
                .node(id)
                .map(|n| n.settings.key.clone())
                .unwrap_or_default(),
            Target::Osc(id) => self
                .session()
                .project()
                .graph
                .node(id)
                .map(|n| n.settings.osc_address.clone())
                .unwrap_or_default(),
            Target::NodeLabel(id) => self
                .session()
                .project()
                .graph
                .node(id)
                .map(|n| n.settings.label.clone())
                .unwrap_or_default(),
            Target::Number(id, p) => self
                .session()
                .project()
                .graph
                .node(id)
                .ok()
                .and_then(|n| n.parameters.get(p))
                .map_or_else(String::new, ToString::to_string),
            Target::ControlLabel(id) => self
                .session()
                .project()
                .controls
                .iter()
                .find(|c| c.id == id)
                .map(|c| c.label.clone())
                .unwrap_or_default(),
            Target::ControlRange(id) => self
                .session()
                .project()
                .controls
                .iter()
                .find(|c| c.id == id)
                .map(|c| {
                    let [a, b] = self.session().control_range(c);
                    format!("{a}, {b}")
                })
                .unwrap_or_default(),
        };
        let input = cx.new(|cx| {
            text_field::TextField::new(
                text,
                matches!(target, Target::Shader(_) | Target::Comment(_))
                    || matches!(&target, Target::Property(id,key) if self.session().schema(*id).properties.iter().any(|p|p.key==*key && p.multiline)),
                cx,
            )
        });
        window.focus(&input.read(cx).focus);
        self.configuration = Some(Configuration {
            target,
            input,
            error: String::new(),
        });
        cx.notify();
    }
    fn apply_configuration(&mut self, text: String, target: Target) -> PortResult<()> {
        match target {
            Target::Uniform(id, index) => {
                let parts: Vec<_> = text.split(',').map(str::trim).collect();
                if parts.len() != 4 {
                    return Err("Enter name, minimum, maximum, default".into());
                }
                let mut uniforms = self.session().shader_uniforms(id)?;
                let value = uniforms.get_mut(index).ok_or("Uniform no longer exists")?;
                value.name = parts[0].into();
                value.min = parts[1].parse().map_err(|_| "Invalid minimum")?;
                value.max = parts[2].parse().map_err(|_| "Invalid maximum")?;
                value.default = parts[3].parse().map_err(|_| "Invalid default")?;
                self.session_mut().set_shader_uniforms(id, uniforms)
            }
            Target::Comment(id) => {
                let mut settings = self
                    .session()
                    .project()
                    .graph
                    .node(id)
                    .map_err(|e| e.to_string())?
                    .settings
                    .clone();
                settings.comment = text;
                self.session_mut().set_settings(id, settings)
            }
            Target::Property(id, key) => {
                let mut settings = self
                    .session()
                    .project()
                    .graph
                    .node(id)
                    .map_err(|e| e.to_string())?
                    .settings
                    .clone();
                let value = settings
                    .properties
                    .get_mut(&key)
                    .ok_or("Setting no longer exists")?;
                *value = text;
                settings.property_labels.remove(&key);
                self.session_mut().set_settings(id, settings)
            }
            Target::OscEndpoint => {
                let endpoint = text
                    .trim()
                    .parse::<std::net::SocketAddr>()
                    .map_err(|_| "Enter an IP address and port, e.g. 0.0.0.0:9000")?;
                if endpoint.port() == 0 {
                    return Err("Use a port from 1 to 65535".into());
                }
                let mut settings = self.session().input_settings.clone();
                settings.osc_host = endpoint.ip().to_string();
                settings.osc_port = Some(endpoint.port());
                self.session_mut().configure_inputs(settings)
            }
            Target::Number(id, p) => self.session_mut().set_parameter(
                id,
                p,
                text.trim().parse().map_err(|_| "Enter a finite number")?,
            ),
            Target::Shader(id) | Target::Key(id) | Target::Osc(id) | Target::NodeLabel(id) => {
                let mut settings = self
                    .session()
                    .project()
                    .graph
                    .node(id)
                    .map_err(|e| e.to_string())?
                    .settings
                    .clone();
                match target {
                    Target::Shader(_) => settings.shader = text,
                    Target::Key(_) => settings.key = text.trim().to_lowercase(),
                    Target::Osc(_) => {
                        if !text.starts_with('/') {
                            return Err("OSC addresses start with /".into());
                        }
                        settings.osc_address = text;
                    }
                    _ => settings.label = text,
                }
                self.session_mut().set_settings(id, settings)
            }
            Target::ControlLabel(id) | Target::ControlRange(id) => {
                let mut binding = self
                    .session()
                    .project()
                    .controls
                    .iter()
                    .find(|c| c.id == id)
                    .cloned()
                    .ok_or("Control no longer exists")?;
                if matches!(target, Target::ControlLabel(_)) {
                    binding.label = text;
                } else {
                    let (a, b) = text.split_once(',').ok_or("Enter minimum, maximum")?;
                    binding.range = Some([
                        a.trim().parse().map_err(|_| "Invalid minimum")?,
                        b.trim().parse().map_err(|_| "Invalid maximum")?,
                    ]);
                }
                self.session_mut().configure_control(binding)
            }
        }
    }
    pub(super) fn configuration_popup(
        &self,
        cx: &Context<Self>,
        width: f32,
        height: f32,
    ) -> Option<gpui::AnyElement> {
        let config = self.configuration.as_ref()?;
        let shader = matches!(config.target, Target::Shader(_));
        let title = match config.target {
            Target::OscEndpoint => "OSC listen address · IP:port",
            Target::Shader(_) => "GLSL fragment shader",
            Target::Uniform(_, _) => "Uniform · name, minimum, maximum, default",
            Target::Comment(_) => "Comment / Label",
            Target::Key(_) => "Keyboard key",
            Target::Osc(_) => "OSC address",
            Target::NodeLabel(_) | Target::ControlLabel(_) => "Label",
            Target::ControlRange(_) => "Control range · minimum, maximum",
            Target::Number(_, _) => "Value",
            Target::Property(_, _) => "Node setting",
        };
        let input = config.input.clone();
        let target = config.target.clone();
        Some(div().id("configuration-popup").absolute().inset_0().bg(gpui::rgba(0x00000088)).flex().items_center().justify_center()
            .on_mouse_down(MouseButton::Left,|_,_,cx|cx.stop_propagation())
            .child(div().w(px(if shader{(width-120.0).min(900.0)}else{420.0})).max_h(px(height-80.0)).p_4().flex().flex_col().gap_3().bg(rgb(SURFACE)).border_1().border_color(rgb(BORDER)).shadow_lg()
                .child(div().font_weight(FontWeight::MEDIUM).child(title))
                .when(shader,|element|element.child(div().text_xs().text_color(rgb(MUTED)).child("mainImage(out vec4 color, in vec2 fragCoord) · iTime · iResolution · iChannel0/1 · iParameters")))
                .child(div().id("configuration-text").max_h(px(if shader{height-240.0}else{100.0})).overflow_y_scroll().child(input.clone()))
                .when(!config.error.is_empty(),|element|element.child(div().max_h(px(100.0)).overflow_hidden().text_xs().text_color(rgb(0xe79791)).child(config.error.clone())))
                .child(div().flex().gap_2().justify_end()
                    .child(button("cancel-configuration","Cancel").on_click(cx.listener(|this,_,window,cx|{this.configuration=None;window.focus(&this.focus);cx.notify();})))
                    .child(button("apply-configuration",if shader{"Compile & apply"}else{"Apply"}).debug_selector(||"apply-configuration".into()).bg(rgb(ACCENT)).text_color(rgb(BACKGROUND)).on_click(cx.listener(move |this,_,window,cx| {
                        match this.apply_configuration(input.read(cx).text.clone(),target.clone()){Ok(())=>{this.configuration=None;this.message.clear();window.focus(&this.focus);},Err(error)=>if let Some(config)=this.configuration.as_mut(){config.error=error;}}cx.notify();
                    })))))
            .into_any_element())
    }
}
