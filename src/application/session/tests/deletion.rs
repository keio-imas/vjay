use super::*;
use vjay_domain::builtins::ids;
use vjay_domain::graph::Connection;

fn modulated_fixture() -> (Session, Rc<RefCell<Counts>>, NodeId) {
    let (mut session, counts, _) = fixture(Project::demo(&builtin_registry()).unwrap().document());
    for _ in 0..3 {
        session
            .add_node(ids::SLIDER, Point { x: 0.0, y: 0.0 })
            .unwrap();
    }
    let lfo = session
        .add_node(ids::LFO, Point { x: 0.0, y: 0.0 })
        .unwrap();
    session
        .connect(Connection {
            from: lfo,
            output: 3,
            to: NodeId(3),
            input: 1,
        })
        .unwrap();
    session.render_at(0.0).unwrap();
    assert!(
        session
            .plan()
            .unwrap()
            .steps
            .iter()
            .flat_map(|s| &s.controls)
            .any(|(_, reference)| reference.source == Some(6))
    );
    (session, counts, lfo)
}
#[test]
fn repeated_deletion_after_missing_image_input_does_not_use_stale_signal_offsets() {
    let (mut session, counts, lfo) = modulated_fixture();
    session.remove_node(NodeId(0)).unwrap();
    assert!(session.render_at(0.1).is_err());
    session.remove_node(lfo).unwrap();
    assert!(session.render_at(0.2).is_err());
    assert_eq!(counts.borrow().frames, 1);
    session
        .connect(Connection {
            from: NodeId(1),
            output: 0,
            to: NodeId(2),
            input: 0,
        })
        .unwrap();
    session.render_at(0.3).unwrap();
    assert_eq!(counts.borrow().frames, 2);
}
#[test]
fn failed_gpu_prepare_cannot_leave_an_old_plan_bound_to_a_new_signal_bank() {
    let (mut session, counts, lfo) = modulated_fixture();
    counts.borrow_mut().fail_prepare = true;
    session.remove_node(lfo).unwrap();
    assert!(session.render_at(0.1).is_err());
    assert!(session.render_at(0.2).is_err());
    counts.borrow_mut().fail_prepare = false;
    session.set_extent(Extent::new(641, 360).unwrap()).unwrap();
    session.render_at(0.3).unwrap();
    assert_eq!(counts.borrow().frames, 2);
}
