use super::{
    decoder::{Chunk, Playback, Shared},
    pcm::Buffer,
};
use cpal::traits::DeviceTrait;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use vjay_application::ports::PortResult;

pub(super) struct SourceConsumer {
    chunks: rtrb::Consumer<Chunk>,
    recycle: rtrb::Producer<Buffer>,
    shared: Arc<Shared>,
    current: Option<Chunk>,
    epoch: u64,
    requested: u64,
    cursor: f64,
    rate: f64,
    gain: f32,
    consumed: f64,
    last: [f32; 2],
    fade_from: [f32; 2],
    fade: u32,
    sample_rate: u32,
}
impl SourceConsumer {
    pub fn new(playback: Playback, shared: Arc<Shared>, sample_rate: u32) -> Self {
        Self {
            chunks: playback.chunks,
            recycle: playback.recycle,
            shared,
            current: None,
            epoch: 0,
            requested: 0,
            cursor: 0.0,
            rate: 0.0,
            gain: 0.0,
            consumed: 0.0,
            last: [0.0; 2],
            fade_from: [0.0; 2],
            fade: 0,
            sample_rate,
        }
    }
    fn release(&mut self, chunk: Chunk) {
        // The recycle ring holds the entire four-buffer pool, including this token.
        let _ = self.recycle.push(chunk.frames);
    }
    fn adopt(&mut self, chunk: Chunk) {
        let changed = self.epoch != chunk.epoch;
        let remainder = self
            .current
            .as_ref()
            .filter(|_| !changed)
            .map_or(0.0, |old| (self.cursor - old.frames.len() as f64).max(0.0));
        if changed && self.current.is_some() {
            self.fade_from = self.last;
            self.fade = 64;
        }
        self.epoch = chunk.epoch;
        self.cursor = remainder;
        if let Some(old) = self.current.replace(chunk) {
            self.release(old);
        }
    }
    pub(super) fn begin_buffer(&mut self) {
        self.rate = f32::from_bits(self.shared.rate.load(Ordering::Acquire)).clamp(0.0, 2.0) as f64;
        self.gain = f32::from_bits(self.shared.gain.load(Ordering::Acquire));
        let requested = self.shared.epoch.load(Ordering::Acquire);
        self.requested = requested;
        // Keep current PCM while a seek is decoding. Only complete replacement
        // windows may switch generations; at most two queued tokens are visited.
        for _ in 0..2 {
            let Ok(next) = self.chunks.peek() else {
                break;
            };
            let adopt = next.epoch != self.epoch
                || self
                    .current
                    .as_ref()
                    .is_none_or(|chunk| self.cursor >= chunk.frames.len() as f64);
            if adopt {
                if let Ok(chunk) = self.chunks.pop() {
                    self.adopt(chunk);
                }
            } else if requested != self.epoch {
                if let Ok(chunk) = self.chunks.pop() {
                    self.release(chunk);
                }
            } else {
                break;
            }
        }
    }
    pub(super) fn frame(&mut self) -> [f32; 2] {
        if self.rate <= 0.0 {
            return [0.0; 2];
        }
        if self
            .current
            .as_ref()
            .is_none_or(|chunk| self.cursor >= chunk.frames.len() as f64)
        {
            match self.chunks.pop() {
                Ok(chunk) => self.adopt(chunk),
                Err(_) if self.requested != self.epoch && self.current.is_some() => {
                    // A cold seek can take longer than one window. Repeat the
                    // current grain until replacement PCM is ready, without a gap.
                    self.cursor %=
                        self.current.as_ref().expect("existing grain").frames.len() as f64;
                    self.fade_from = self.last;
                    self.fade = 64;
                }
                Err(_) => {
                    self.last = [0.0; 2];
                    return self.last;
                }
            }
        }
        let chunk = self.current.as_ref().expect("current PCM window");
        let index = self.cursor as usize;
        let fraction = (self.cursor - index as f64) as f32;
        let a = chunk.frames[index];
        let b = if index + 1 < chunk.frames.len() {
            chunk.frames[index + 1]
        } else {
            self.chunks
                .peek()
                .ok()
                .filter(|next| next.epoch == self.epoch)
                .map_or(a, |next| next.frames[0])
        };
        let mut sample = [
            a[0] + (b[0] - a[0]) * fraction,
            a[1] + (b[1] - a[1]) * fraction,
        ];
        if self.fade > 0 {
            let amount = (65 - self.fade) as f32 / 64.0;
            for (value, from) in sample.iter_mut().zip(self.fade_from) {
                *value = from + (*value - from) * amount;
            }
            self.fade -= 1;
        }
        self.cursor += self.rate;
        self.consumed += self.rate;
        self.last = sample;
        sample.map(|value| value * self.gain)
    }
    pub(super) fn finish_buffer(&mut self) {
        let consumed = self.consumed.floor() as u64;
        self.consumed -= consumed as f64;
        self.shared.consumed.fetch_add(consumed, Ordering::Relaxed);
        if let Some(chunk) = &self.current {
            let raw =
                chunk.start + self.cursor.min(chunk.frames.len() as f64) / self.sample_rate as f64;
            let start = f64::from_bits(self.shared.range_start.load(Ordering::Acquire));
            let end = f64::from_bits(self.shared.range_end.load(Ordering::Acquire));
            let seconds = if self.shared.looping.load(Ordering::Acquire) {
                start + (raw - start).rem_euclid((end - start).max(1e-9))
            } else {
                raw.min(end)
            };
            self.shared
                .playhead
                .store(seconds.to_bits(), Ordering::Release);
            self.shared
                .active_epoch
                .store(self.epoch, Ordering::Release);
        }
    }
}

pub(super) fn open<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut sources: Vec<SourceConsumer>,
    playing: Arc<AtomicBool>,
    failed: Arc<AtomicBool>,
) -> PortResult<cpal::Stream>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    let channels = config.channels as usize;
    // Fixed PCM windows and recycled tokens: no allocations, mutex, decode or I/O.
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                if !playing.load(Ordering::Acquire) {
                    data.fill(T::from_sample(0.0));
                    return;
                }
                for source in &mut sources {
                    source.begin_buffer();
                }
                for frame in data.chunks_mut(channels) {
                    let mut stereo = [0.0_f32; 2];
                    for source in &mut sources {
                        let value = source.frame();
                        stereo[0] += value[0];
                        stereo[1] += value[1];
                    }
                    for (channel, sample) in frame.iter_mut().enumerate() {
                        let value = if channels == 1 {
                            (stereo[0] + stereo[1]) * 0.5
                        } else if channel < 2 {
                            stereo[channel]
                        } else {
                            0.0
                        };
                        *sample = T::from_sample(value.clamp(-1.0, 1.0));
                    }
                }
                for source in &mut sources {
                    source.finish_buffer();
                }
            },
            move |_| {
                failed.store(true, Ordering::Release);
            },
            None,
        )
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests;
