use super::*;
use vjay_application::control_io::*;
use vjay_domain::builtins::ids;

pub(super) struct Inputs {
    enabled: bool,
    ports: Vec<MidiPort>,
}
impl Default for Inputs {
    fn default() -> Self {
        Self {
            enabled: false,
            ports: vec![MidiPort {
                id: "controller-a".into(),
                name: "Controller A".into(),
                error: None,
                virtual_input: false,
            }],
        }
    }
}
impl ControlIo for Inputs {
    fn configure(&mut self, settings: &InputSettings) -> PortResult<String> {
        self.enabled = settings.midi;
        Ok("MIDI: Controller A".into())
    }
    fn poll(&mut self, _: &mut Vec<ControlEvent>) -> PortResult<()> {
        Ok(())
    }
    fn midi_ports(&self) -> &[MidiPort] {
        if self.enabled { &self.ports } else { &[] }
    }
}
#[gpui::test]
fn midi_node_starts_receiving_and_learns_from_inspector(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(editor);
    cx.simulate_resize(size(px(1440.0), px(940.0)));
    let node = view.update(cx, |editor, cx| {
        let node = editor
            .session_mut()
            .add_node(ids::MIDI, Position { x: 40.0, y: 40.0 })
            .unwrap();
        editor.selection.only(node);
        cx.notify();
        node
    });
    assert!(cx.read(|cx| view.read(cx).session().input_settings.midi));
    let learn = center(cx, "midi-learn");
    cx.simulate_click(learn, Modifiers::none());
    view.update(cx, |editor, _| {
        assert_eq!(editor.session().midi_activity().learning, Some(node));
        editor.session_mut().input_event(ControlEvent::MidiFrom {
            source: "controller-a".into(),
            channel: 2,
            message: MidiMessage::Cc {
                controller: 74,
                value: 96,
            },
        });
        editor.session_mut().render_at(0.0).unwrap();
        let session = editor.session();
        let record = session.project().graph.node(node).unwrap();
        assert_eq!(record.parameters[0], 3.0);
        assert_eq!(record.parameters[1], 74.0);
        assert_eq!(record.settings.midi_source.as_deref(), Some("controller-a"));
        assert!((session.signal_value(node, 0).unwrap() - 96.0 / 127.0).abs() < 1e-6);
        assert!(session.midi_activity().learning.is_none());
        assert_eq!(session.midi_activity().received, 1);
    });
}
