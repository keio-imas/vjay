use crate::execution::ScalarContext as C;
mod arithmetic;
mod timing;
pub(crate) use arithmetic::*;
pub(crate) use timing::*;

pub(crate) fn copy(c: &C<'_>, _: &mut [f64], out: &mut [f32]) {
    out[0] = c.parameters[0];
}
pub(crate) fn xy(c: &C<'_>, _: &mut [f64], out: &mut [f32]) {
    out.copy_from_slice(&c.parameters[..2]);
}
pub(crate) fn button(c: &C<'_>, s: &mut [f64], out: &mut [f32]) {
    let value = c.parameters[0];
    out[0] = value;
    out[1] = f32::from(c.host.trigger(c.node) || value > 0.0 && s[0] <= 0.0);
    s[0] = value as f64;
}
pub(crate) fn keyboard(c: &C<'_>, _: &mut [f64], out: &mut [f32]) {
    out.copy_from_slice(&c.host.key(&c.settings.key));
}
pub(crate) fn osc(c: &C<'_>, _: &mut [f64], out: &mut [f32]) {
    out.copy_from_slice(&c.host.osc(&c.settings.osc_address));
}
pub(crate) fn midi(c: &C<'_>, _: &mut [f64], out: &mut [f32]) {
    let p = c.parameters;
    out.copy_from_slice(&c.host.midi(
        c.settings.midi_source.as_deref(),
        p[0].round().clamp(1.0, 16.0) as usize - 1,
        p[1].round() as usize,
        (p[2] >= 0.0).then_some(p[2].round() as usize),
    ));
}
