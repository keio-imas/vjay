use super::easing::ease;
#[test]
fn easing_curves_have_endpoints_finite_samples_and_expected_shapes() {
    for curve in 0..=20 {
        assert_eq!(ease(0.0, curve), 0.0);
        assert_eq!(ease(1.0, curve), 1.0);
        for i in 0..100 {
            assert!(ease(i as f32 / 100.0, curve).is_finite());
        }
    }
    assert_eq!(ease(0.5, 1), 0.25);
    assert_eq!(ease(0.5, 2), 0.75);
    assert!((ease(0.5, 9) - 0.5).abs() < 1e-6);
    assert!(ease(0.5, 16) < 0.0);
    assert!(ease(0.5, 17) > 1.0);
}
