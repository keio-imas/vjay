use super::*;
use vjay_application::{project::WidgetKind, value_display::ValueFormat};

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let point = center(cx, selector);
    cx.simulate_click(point, Modifiers::none());
}

#[gpui::test]
fn control_readout_dropdowns_change_presentation_without_modifying_the_signal(
    cx: &mut TestAppContext,
) {
    let (view, cx) = cx.add_window_view(editor);
    cx.simulate_resize(size(px(1800.0), px(1260.0)));
    let node = view.update(cx, |this, cx| {
        let id = this.session_mut().add_control(WidgetKind::Number).unwrap();
        let mut control = this
            .session()
            .project()
            .controls
            .iter()
            .find(|c| c.id == id)
            .unwrap()
            .clone();
        control.range = Some([0.0, 7200.0]);
        this.session_mut()
            .configure_control(control.clone())
            .unwrap();
        this.session_mut()
            .set_parameter(control.node, 0, 3661.234)
            .unwrap();
        this.session_mut().render_at(0.0).unwrap();
        this.selected_control = Some(id);
        this.selection.clear();
        cx.notify();
        control.node
    });
    click(cx, "display-format");
    click(cx, "option-1");
    cx.read(|cx| {
        let session = view.read(cx).session();
        let control = session.project().controls.last().unwrap();
        assert_eq!(control.display.format, ValueFormat::Timecode);
        assert_eq!(session.control_readout(control, 3), "01:01:01.234");
        assert_eq!(
            session.project().graph.node(node).unwrap().parameters[0],
            3661.234
        );
    });
    click(cx, "display-value");
    click(cx, "option-1");
    click(cx, "display-total");
    click(cx, "option-1");
    cx.read(|cx| {
        let session = view.read(cx).session();
        let control = session.project().controls.last().unwrap();
        assert_eq!(control.display.output, Some(0));
        assert_eq!(
            session.control_readout(control, 3),
            "01:01:01.234 / 01:01:01.234"
        );
    });
}
