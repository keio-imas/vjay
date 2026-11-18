use vjay_domain::{
    capabilities::Capabilities,
    execution::{ScalarContext, ScalarKernel, TextContext, TextKernel},
    node::{Node, NodeDescriptor, Parameter as P},
    port::Port,
    settings::NodeSettings,
};

pub struct AbletonLinkNode;
pub struct RekordboxHistoryNode;
static LINK: NodeDescriptor = NodeDescriptor {
    type_id: "AbletonLink",
    name: "Ableton Link",
    category: "External",
    inputs: &[],
    outputs: &[
        Port::scalar("BPM"),
        Port::scalar("Beat"),
        Port::scalar("Phase"),
        Port::scalar("Pulse"),
        Port::scalar("Peers"),
        Port::scalar("Playing"),
    ],
    parameters: &[
        P::new("Enabled", 0.0, 1.0, 1.0).choices(vjay_domain::choices::SWITCH),
        P::new("Quantum", 1.0, 32.0, 4.0),
        P::new("Tempo", 20.0, 999.0, 120.0),
        P::new("Start/Stop sync", 0.0, 1.0, 0.0).choices(vjay_domain::choices::SWITCH),
        P::new("Transport", -1.0, 1.0, -1.0).choices(&["Follow", "Stop", "Play"]),
    ],
    capabilities: Capabilities::NONE,
};
static REKORDBOX: NodeDescriptor = NodeDescriptor {
    type_id: "RekordboxHistory",
    name: "rekordbox History",
    category: "External",
    inputs: &[],
    outputs: &[
        Port::scalar("BPM"),
        Port::scalar("Duration"),
        Port::scalar("Changed"),
        Port::scalar("Ready"),
        Port::text("Title"),
        Port::text("Artist"),
        Port::text("Album"),
        Port::text("Genre"),
        Port::text("Label"),
        Port::text("Key"),
        Port::text("File path"),
        Port::text("Track ID"),
        Port::text("History time"),
        Port::text("Status"),
        Port::file("Album cover"),
        Port::scalar("Cover ready"),
    ],
    parameters: &[],
    capabilities: Capabilities::NONE,
};
fn copy(context: &ScalarContext<'_>, _: &mut [f64], output: &mut [f32]) {
    for (i, value) in output.iter_mut().enumerate() {
        *value = context.host.node_value(context.node, i);
    }
}
fn metadata(
    context: &TextContext<'_>,
    _: &mut [std::sync::Arc<str>],
    output: &mut [std::sync::Arc<str>],
) {
    for (i, value) in output.iter_mut().enumerate() {
        *value = context.host.node_text(context.node, i);
    }
}
impl Node for AbletonLinkNode {
    fn descriptor(&self) -> &'static NodeDescriptor {
        &LINK
    }
    fn service(&self) -> Option<&'static str> {
        Some("ableton-link")
    }
    fn scalar(&self) -> Option<ScalarKernel> {
        Some(ScalarKernel {
            evaluate: copy,
            state: Vec::new(),
            tap: None,
        })
    }
}
impl Node for RekordboxHistoryNode {
    fn file(&self) -> Option<vjay_domain::execution::FileEvaluate> {
        Some(|ctx, out| out[0] = ctx.host.node_file(ctx.node, 0))
    }
    fn descriptor(&self) -> &'static NodeDescriptor {
        &REKORDBOX
    }
    fn service(&self) -> Option<&'static str> {
        Some("rekordbox-history")
    }
    fn scalar(&self) -> Option<ScalarKernel> {
        Some(ScalarKernel {
            evaluate: copy,
            state: Vec::new(),
            tap: None,
        })
    }
    fn text(&self, _: &NodeSettings) -> Option<TextKernel> {
        Some(TextKernel {
            evaluate: metadata,
            state: vec![],
        })
    }
    fn default_settings(&self) -> NodeSettings {
        let mut settings = NodeSettings::default();
        settings.properties.extend([
            ("database".into(), "auto".into()),
            ("options_file".into(), "auto".into()),
            ("poll_seconds".into(), "2".into()),
        ]);
        settings
    }
    fn validate_settings(&self, s: &NodeSettings) -> bool {
        s.is_valid()
            && s.properties.get("poll_seconds").is_none_or(|p| {
                p.parse::<f64>()
                    .is_ok_and(|v| v.is_finite() && (0.25..=60.0).contains(&v))
            })
    }
}

#[cfg(test)]
mod tests;
