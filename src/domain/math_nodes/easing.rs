use crate::execution::ScalarContext;
use std::f32::consts::PI;
pub(super) fn evaluate(c: &ScalarContext<'_>, _: &mut [f64], o: &mut [f32]) {
    o[0] = ease(c.parameters[0], c.parameters[1].round() as u32);
}
pub(crate) fn ease(t: f32, curve: u32) -> f32 {
    if t <= 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }
    match curve {
        1 => t * t,
        2 => 1.0 - (1.0 - t).powi(2),
        3 => {
            if t < 0.5 {
                2.0 * t * t
            } else {
                1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
            }
        }
        4 => t.powi(3),
        5 => 1.0 - (1.0 - t).powi(3),
        6 => {
            if t < 0.5 {
                4.0 * t.powi(3)
            } else {
                1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
            }
        }
        7 => 1.0 - (t * PI / 2.0).cos(),
        8 => (t * PI / 2.0).sin(),
        9 => -(PI * t).cos() / 2.0 + 0.5,
        10 => 2.0f32.powf(10.0 * t - 10.0),
        11 => 1.0 - 2.0f32.powf(-10.0 * t),
        12 => {
            if t < 0.5 {
                2.0f32.powf(20.0 * t - 10.0) / 2.0
            } else {
                (2.0 - 2.0f32.powf(-20.0 * t + 10.0)) / 2.0
            }
        }
        13 => 1.0 - (1.0 - t * t).sqrt(),
        14 => (1.0 - (t - 1.0).powi(2)).sqrt(),
        15 => {
            if t < 0.5 {
                (1.0 - (1.0 - (2.0 * t).powi(2)).sqrt()) / 2.0
            } else {
                ((1.0 - (-2.0 * t + 2.0).powi(2)).sqrt() + 1.0) / 2.0
            }
        }
        16 => 2.70158 * t.powi(3) - 1.70158 * t * t,
        17 => 1.0 + 2.70158 * (t - 1.0).powi(3) + 1.70158 * (t - 1.0).powi(2),
        18 => {
            let k = 1.70158 * 1.525;
            if t < 0.5 {
                (2.0 * t).powi(2) * ((k + 1.0) * 2.0 * t - k) / 2.0
            } else {
                ((2.0 * t - 2.0).powi(2) * ((k + 1.0) * (2.0 * t - 2.0) + k) + 2.0) / 2.0
            }
        }
        19 => bounce(t),
        20 => 2.0f32.powf(-10.0 * t) * ((t * 10.0 - 0.75) * (2.0 * PI / 3.0)).sin() + 1.0,
        _ => t,
    }
}
fn bounce(mut t: f32) -> f32 {
    let n = 7.5625;
    let d = 2.75;
    if t < 1.0 / d {
        n * t * t
    } else if t < 2.0 / d {
        t -= 1.5 / d;
        n * t * t + 0.75
    } else if t < 2.5 / d {
        t -= 2.25 / d;
        n * t * t + 0.9375
    } else {
        t -= 2.625 / d;
        n * t * t + 0.984375
    }
}
