use super::C;
pub(crate) fn add(c: &C<'_>, _: &mut [f64], out: &mut [f32]) {
    out[0] = c.parameters[0] + c.parameters[1];
}
pub(crate) fn multiply(c: &C<'_>, _: &mut [f64], out: &mut [f32]) {
    out[0] = c.parameters[0] * c.parameters[1];
}
pub(crate) fn map_range(c: &C<'_>, _: &mut [f64], out: &mut [f32]) {
    let p = c.parameters;
    out[0] = if (p[2] - p[1]).abs() < 1e-8 {
        p[3]
    } else {
        p[3] + (p[0] - p[1]) / (p[2] - p[1]) * (p[4] - p[3])
    };
}
pub(crate) fn clamp(c: &C<'_>, _: &mut [f64], out: &mut [f32]) {
    let p = c.parameters;
    out[0] = p[0].clamp(p[1].min(p[2]), p[1].max(p[2]));
}
pub(crate) fn smooth(c: &C<'_>, s: &mut [f64], out: &mut [f32]) {
    let p = c.parameters;
    let factor = if p[1] <= 0.0 {
        1.0
    } else {
        1.0 - (-c.delta / p[1] as f64).exp()
    };
    s[0] += (p[0] as f64 - s[0]) * factor;
    out[0] = s[0] as f32;
}
pub(crate) fn counter(c: &C<'_>, s: &mut [f64], out: &mut [f32]) {
    let trigger = c.inputs[0].unwrap_or(0.0) as f64;
    let rising = trigger > 0.0 && s[1] <= 0.0;
    if c.parameters[0] > 0.5 {
        s[0] = 0.0;
    } else if rising {
        s[0] += 1.0;
    }
    s[1] = trigger;
    out[0] = s[0] as f32;
    out[1] = f32::from(rising);
}
