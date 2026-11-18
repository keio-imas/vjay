#[derive(Clone, Debug)]
pub struct TapTempo {
    last: Option<f64>,
    intervals: [f64; 8],
    count: usize,
    next: usize,
    pub bpm: Option<f64>,
}
impl Default for TapTempo {
    fn default() -> Self {
        Self {
            last: None,
            intervals: [0.0; 8],
            count: 0,
            next: 0,
            bpm: None,
        }
    }
}
impl TapTempo {
    pub fn tap(&mut self, seconds: f64) -> Option<f64> {
        if !seconds.is_finite() {
            return self.bpm;
        }
        if let Some(last) = self.last {
            let interval = seconds - last;
            if !(0.2..=3.0).contains(&interval) {
                self.count = 0;
                self.next = 0;
                self.bpm = None;
            } else {
                self.intervals[self.next] = interval;
                self.next = (self.next + 1) % 8;
                self.count = (self.count + 1).min(8);
                let mut sorted = self.intervals;
                sorted[..self.count].sort_by(f64::total_cmp);
                self.bpm = Some((60.0 / sorted[self.count / 2]).clamp(20.0, 300.0));
            }
        }
        self.last = Some(seconds);
        self.bpm
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn median_rejects_tap_outliers_and_long_gaps_restart_estimation() {
        let mut tempo = TapTempo::default();
        for t in [0.0, 0.5, 1.0, 1.8, 2.3, 2.8] {
            tempo.tap(t);
        }
        assert_eq!(tempo.bpm, Some(120.0));
        assert_eq!(tempo.tap(f64::NAN), Some(120.0));
        assert_eq!(tempo.tap(20.0), None);
        assert_eq!(tempo.tap(21.0), Some(60.0));
    }
}
