use super::C;
pub(crate) fn lfo(c: &C<'_>, s: &mut [f64], out: &mut [f32]) {
    let p = c.parameters;
    s[0] = c.inputs[0].map_or_else(
        || (s[0] + c.delta * p[0] as f64).rem_euclid(1.0),
        |v| v.rem_euclid(1.0) as f64,
    );
    let phase = (s[0] as f32 + p[3]).rem_euclid(1.0);
    out.copy_from_slice(&[
        (phase * std::f32::consts::TAU).sin() * p[1] + p[2],
        (1.0 - 4.0 * (phase - 0.5).abs()) * p[1] + p[2],
        phase * p[1] + p[2],
        if phase < 0.5 { p[1] + p[2] } else { p[2] },
    ]);
}
pub(crate) fn tap(s: &mut [f64], seconds: f64) {
    if !seconds.is_finite() {
        return;
    }
    if s[7] != 0.0 {
        let interval = seconds - s[3];
        if !(0.2..=3.0).contains(&interval) {
            s[4] = 0.0;
            s[5] = 0.0;
            s[6] = 0.0;
        } else {
            let next = s[5] as usize;
            s[8 + next] = interval;
            s[5] = ((next + 1) % 8) as f64;
            s[4] = (s[4] + 1.0).min(8.0);
            let count = s[4] as usize;
            let mut sorted: [f64; 8] = std::array::from_fn(|i| s[8 + i]);
            sorted[..count].sort_by(f64::total_cmp);
            s[6] = (60.0 / sorted[count / 2]).clamp(20.0, 300.0);
        }
    }
    s[3] = seconds;
    s[7] = 1.0;
}
pub(crate) fn tempo(c: &C<'_>, s: &mut [f64], out: &mut [f32]) {
    s[2] += c.delta;
    let trigger = c.inputs[0].unwrap_or(0.0) as f64;
    if trigger > 0.0 && s[1] <= 0.0 {
        tap(s, s[2]);
    }
    s[1] = trigger;
    let bpm = if s[6] > 0.0 {
        s[6]
    } else {
        c.parameters[0] as f64
    };
    let next = s[0] + c.delta * bpm / 60.0 * c.parameters[1].round() as f64;
    out.copy_from_slice(&[
        f32::from(next.floor() > s[0].floor()),
        next.fract() as f32,
        bpm as f32,
    ]);
    s[0] = next;
}
