use super::*;
impl Source {
    pub(super) fn effective_speed(&self) -> f32 {
        if self.paused {
            0.0
        } else {
            self.rate
                * (1.0
                    + 0.02
                        * (self.last_commands[5].clamp(0.0, 1.0)
                            - self.last_commands[4].clamp(0.0, 1.0)
                            + self.user_nudge)
                            .clamp(-1.0, 1.0))
        }
    }
    pub(super) fn state(&self) -> MediaState {
        let (start, end) = self.window.bounds(self.duration);
        let (seconds, ended) = if self.duration > 0.0 {
            self.window
                .resolve(self.base, self.last_position, self.duration)
        } else {
            (0.0, false)
        };
        MediaState {
            seconds,
            duration: self.duration,
            position: ((seconds - start) / (end - start).max(1e-9)).clamp(0.0, 1.0) as f32,
            speed: if ended { 0.0 } else { self.effective_speed() },
            playing: !self.paused && self.rate > 0.0 && self.ready && self.duration > 0.0 && !ended,
            ready: self.ready && self.duration > 0.0,
            cue: self.cue,
        }
    }
    fn play(&mut self) {
        if !self.window.looping
            && self.duration > 0.0
            && self
                .window
                .resolve(self.base, self.last_position, self.duration)
                .1
        {
            self.seek_to(0.0);
        }
        self.paused = false;
    }
    pub(super) fn seek_to(&mut self, target: f32) {
        let (start, end) = self.window.bounds(self.duration);
        self.base = (f64::from(target.clamp(0.0, 1.0)).min(1.0 - 1e-7)
            - f64::from(self.last_position))
            * (end - start);
        self.seek_revision = self.seek_revision.wrapping_add(1);
    }
    pub(super) fn read_commands(&mut self, signals: &SignalEngine) {
        let values = self
            .commands
            .map(|v| v.map_or(0.0, |v| v.read(&signals.values)));
        for i in [0, 1, 2, 3, 6, 7] {
            if values[i] != 0.0 && values[i] != self.last_commands[i] && self.actions.len() < 64 {
                self.actions.push(match i {
                    0 => TransportAction::Play,
                    1 => TransportAction::Pause,
                    2 => TransportAction::Toggle,
                    3 => TransportAction::Jog(f64::from(values[i])),
                    6 => TransportAction::SetCue,
                    _ => TransportAction::RecallCue,
                });
            }
        }
        self.last_commands = values;
    }
    pub(super) fn apply_actions(&mut self) {
        for index in 0..self.actions.len() {
            match self.actions[index] {
                TransportAction::Play => self.play(),
                TransportAction::Pause => self.paused = true,
                TransportAction::Toggle => {
                    if self.state().playing {
                        self.paused = true;
                    } else {
                        self.play();
                    }
                }
                TransportAction::Seek(p) => self.seek_to(p),
                TransportAction::Jog(seconds) => {
                    let (a, b) = self.window.bounds(self.duration);
                    // Jog is a time delta. Keep it in f64 seconds instead of
                    // quantizing every motion through a normalized f32 slider.
                    let offset =
                        (self.state().seconds - a + seconds).clamp(0.0, (b - a) * (1.0 - 1e-7));
                    self.base = offset - f64::from(self.last_position) * (b - a);
                    self.seek_revision = self.seek_revision.wrapping_add(1);
                }
                TransportAction::SetCue => self.cue = self.state().position,
                TransportAction::RecallCue => self.seek_to(self.cue),
            }
        }
        self.actions.clear();
    }
}
