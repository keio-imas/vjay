use super::*;
use crate::value_display::{ControlDisplay, ValueFormat};

#[test]
fn display_format_is_generic_and_does_not_change_values_or_ranges() {
    let (mut s, _, _) = fixture(Project::demo(&builtin_registry()).unwrap().document());
    let id = s.add_control(WidgetKind::Number).unwrap();
    let mut control = s
        .project()
        .controls
        .iter()
        .find(|c| c.id == id)
        .unwrap()
        .clone();
    s.set_parameter(control.node, 0, 3661.234).unwrap();
    control.range = Some([0.0, 7200.0]);
    control.display.format = ValueFormat::Timecode;
    s.configure_control(control.clone()).unwrap();
    s.render_at(0.0).unwrap();
    assert_eq!(s.control_readout(&control, 3), "01:01:01.234");
    assert_eq!(s.control_value(&control), 3661.234);
    assert_eq!(s.control_range(&control), [0.0, 7200.0]);
    control.display.output = Some(0);
    s.configure_control(control.clone()).unwrap();
    assert_eq!(s.control_readout(&control, 3), "01:01:01.234");
    let restored = Project::restore(s.project().document(), &builtin_registry()).unwrap();
    assert_eq!(restored.controls.last().unwrap().display, control.display);
    control.display.total_output = Some(999);
    assert!(s.configure_control(control).is_err());
}

#[test]
fn seek_timecode_uses_observed_seconds_in_a_subrange_and_survives_encapsulation() {
    let (mut s, counts, _) = fixture(Project::demo(&builtin_registry()).unwrap().document());
    counts.borrow_mut().duration = Some(30.0);
    let video = s.import_video("/fixtures/clip.mov".into(), None).unwrap();
    s.set_parameter(video, 3, 0.2).unwrap();
    s.set_parameter(video, 4, 0.8).unwrap();
    let seek = s.setup_seek_control(video).unwrap();
    s.render_at(0.0).unwrap();
    s.set_parameter(seek, 0, 0.25).unwrap();
    s.render_at(0.0).unwrap();
    s.render_at(0.0).unwrap();
    let control = s.project().controls.last().unwrap().clone();
    assert_eq!(s.control_range(&control), [0.0, 1.0]);
    assert!((s.control_value(&control) - 0.25).abs() < 1e-5);
    assert_eq!(
        s.control_readout(&control, 3),
        "00:00:10.500 / 00:00:30.000"
    );
    let mut raw = control.clone();
    raw.display = ControlDisplay::default();
    assert_eq!(s.control_readout(&raw, 3), "0.250");
    let patch = s.encapsulate(&[seek]).unwrap();
    s.enter_subpatch(patch).unwrap();
    s.render_at(0.0).unwrap();
    let control = s.project().controls.first().unwrap();
    assert_eq!(s.control_readout(control, 3), "00:00:10.500 / 00:00:30.000");
    s.leave_subpatch().unwrap();
    s.save().unwrap();
    s.load().unwrap();
    s.enter_subpatch(patch).unwrap();
    s.render_at(0.0).unwrap();
    s.render_at(0.0).unwrap();
    assert_eq!(
        s.control_readout(s.project().controls.first().unwrap(), 3),
        "00:00:10.500 / 00:00:30.000"
    );
}
