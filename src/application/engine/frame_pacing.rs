use std::time::Duration;

/// Fixed deadlines exclude render/UI work from the requested frame period.
/// Overruns skip deadlines rather than generating catch-up bursts.
#[derive(Default)]
pub struct FramePacer {
    deadline: Option<f64>,
    period: f64,
    pub missed_deadlines: u64,
    pub maximum_lateness: f64,
}
impl FramePacer {
    pub fn delay(&mut self, now: f64, interval: Duration) -> Duration {
        let period = interval.as_secs_f64().max(0.000_001);
        if self.deadline.is_none() || self.period != period {
            self.period = period;
            self.deadline = Some(now + period);
        }
        Duration::from_secs_f64((self.deadline.unwrap() - now).max(0.0))
    }
    pub fn started(&mut self, now: f64) {
        self.maximum_lateness = self
            .maximum_lateness
            .max((now - self.deadline.unwrap_or(now)).max(0.0));
    }
    pub fn finished(&mut self, now: f64) {
        let mut next = self.deadline.unwrap_or(now) + self.period;
        if next <= now {
            let missed = ((now - next) / self.period).floor() as u64 + 1;
            self.missed_deadlines += missed;
            next += missed as f64 * self.period;
        }
        self.deadline = Some(next);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rendering_time_does_not_accumulate_into_frame_intervals() {
        let mut p = FramePacer::default();
        let period = Duration::from_secs_f64(1.0 / 60.0);
        let mut now = 0.0;
        for frame in 1..=600 {
            now += p.delay(now, period).as_secs_f64();
            p.started(now);
            assert!((now - frame as f64 / 60.0).abs() < 1e-6);
            now += 0.003;
            p.finished(now);
        }
        assert_eq!(p.missed_deadlines, 0);
    }
    #[test]
    fn overruns_skip_deadlines_and_rate_changes_start_a_new_period() {
        let mut p = FramePacer::default();
        let interval = Duration::from_millis(10);
        p.delay(0.0, interval);
        p.started(0.012);
        p.finished(0.036);
        assert_eq!(p.missed_deadlines, 2);
        assert!((p.delay(0.036, interval).as_secs_f64() - 0.004).abs() < 1e-9);
        assert_eq!(
            p.delay(0.04, Duration::from_millis(20)),
            Duration::from_millis(20)
        );
    }
}
