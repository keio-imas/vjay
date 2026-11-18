use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use vjay_application::{
    audio::{AudioBackend, AudioFactory, AudioPlan},
    media::MediaAsset,
    ports::PortResult,
};
use vjay_domain::{graph::NodeId, node::AssetId};
mod decoder;
mod mixer;
mod pcm;
use decoder::AudioWorker;

pub struct CpalAudioFactory {
    ffmpeg: PathBuf,
    ffprobe: PathBuf,
}
impl CpalAudioFactory {
    pub fn new(ffmpeg: PathBuf, ffprobe: PathBuf) -> Self {
        Self { ffmpeg, ffprobe }
    }
}
impl AudioFactory for CpalAudioFactory {
    fn create(&self) -> Box<dyn AudioBackend> {
        Box::new(CpalAudio {
            ffmpeg: self.ffmpeg.clone(),
            ffprobe: self.ffprobe.clone(),
            device_name: String::new(),
            stream: None,
            sources: Vec::new(),
            keys: Vec::new(),
            playing: Arc::new(AtomicBool::new(false)),
            failed: Arc::new(AtomicBool::new(false)),
        })
    }
}
struct Source {
    node: NodeId,
    worker: AudioWorker,
    position: f32,
    window: vjay_application::media::PlaybackWindow,
}
struct CpalAudio {
    ffmpeg: PathBuf,
    ffprobe: PathBuf,
    device_name: String,
    stream: Option<cpal::Stream>,
    sources: Vec<Source>,
    keys: Vec<(NodeId, AssetId)>,
    playing: Arc<AtomicBool>,
    failed: Arc<AtomicBool>,
}
impl AudioBackend for CpalAudio {
    fn selected_device(&self) -> &str {
        &self.device_name
    }
    fn devices(&self) -> Vec<String> {
        let mut names = cpal::default_host()
            .output_devices()
            .map(|d| d.filter_map(|d| d.name().ok()).collect::<Vec<_>>())
            .unwrap_or_default();
        names.sort();
        names.dedup();
        names
    }
    fn select_device(&mut self, name: &str) -> PortResult<()> {
        if !name.is_empty() && !self.devices().iter().any(|n| n == name) {
            return Err("Audio device not found".into());
        }
        self.playing.store(false, Ordering::Release);
        self.stream = None;
        self.keys.clear();
        self.device_name = name.into();
        Ok(())
    }
    fn media_duration(&self, node: NodeId) -> Option<f64> {
        self.sources
            .iter()
            .find(|s| s.node == node)
            .map(|s| s.worker.duration)
    }
    fn prepare(&mut self, plan: &AudioPlan, assets: &[MediaAsset]) -> PortResult<()> {
        let mut keys: Vec<_> = plan
            .routes
            .iter()
            .map(|route| (route.node, route.asset))
            .collect();
        keys.sort_by_key(|key| key.0.0);
        keys.dedup();
        if keys == self.keys && !self.failed.load(Ordering::Acquire) {
            return Ok(());
        }
        if keys.len() > 8 {
            return Err("At most eight audio sources are supported".into());
        }
        self.playing.store(false, Ordering::Release);
        self.failed.store(false, Ordering::Release);
        self.stream = None;
        self.sources.clear();
        self.keys.clear();
        if keys.is_empty() {
            return Ok(());
        }
        let host = cpal::default_host();
        let device = if self.device_name.is_empty() {
            host.default_output_device()
        } else {
            host.output_devices()
                .map_err(|e| e.to_string())?
                .find(|d| d.name().is_ok_and(|n| n == self.device_name))
        }
        .ok_or("No matching monitor audio device")?;
        let supported = device.default_output_config().map_err(|e| e.to_string())?;
        let format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        let mut inputs = Vec::new();
        for &(node, asset) in &keys {
            let path = &assets
                .iter()
                .find(|a| a.id == asset)
                .ok_or("Audio media asset missing")?
                .path;
            let (worker, consumer) =
                AudioWorker::open(&self.ffmpeg, &self.ffprobe, path, config.sample_rate.0)?;
            inputs.push(mixer::SourceConsumer::new(
                consumer,
                worker.shared.clone(),
                worker.sample_rate,
            ));
            self.sources.push(Source {
                node,
                worker,
                position: f32::NAN,
                window: Default::default(),
            });
        }
        let stream = match format {
            cpal::SampleFormat::F32 => mixer::open::<f32>(
                &device,
                &config,
                inputs,
                self.playing.clone(),
                self.failed.clone(),
            ),
            cpal::SampleFormat::F64 => mixer::open::<f64>(
                &device,
                &config,
                inputs,
                self.playing.clone(),
                self.failed.clone(),
            ),
            cpal::SampleFormat::I16 => mixer::open::<i16>(
                &device,
                &config,
                inputs,
                self.playing.clone(),
                self.failed.clone(),
            ),
            cpal::SampleFormat::U16 => mixer::open::<u16>(
                &device,
                &config,
                inputs,
                self.playing.clone(),
                self.failed.clone(),
            ),
            cpal::SampleFormat::I32 => mixer::open::<i32>(
                &device,
                &config,
                inputs,
                self.playing.clone(),
                self.failed.clone(),
            ),
            _ => Err(format!("Unsupported audio device format: {format}")),
        }?;
        stream.play().map_err(|e| e.to_string())?;
        self.stream = Some(stream);
        self.keys = keys;
        Ok(())
    }
    fn update(&mut self, plan: &AudioPlan, paused: bool) -> PortResult<()> {
        if self.failed.load(Ordering::Acquire) {
            self.playing.store(false, Ordering::Release);
            return Err("Audio device stream failed; reconnect Audio Out to retry".into());
        }
        self.playing.store(!paused, Ordering::Release);
        for source in &mut self.sources {
            if let Some(error) = source
                .worker
                .shared
                .error
                .lock()
                .map_err(|e| e.to_string())?
                .clone()
            {
                self.playing.store(false, Ordering::Release);
                return Err(error);
            }
            let mut gain = 0.0;
            let mut route_state = None;
            for route in plan.routes.iter().filter(|route| route.node == source.node) {
                let (_, ended) =
                    route
                        .window
                        .resolve(route.seconds, route.position, source.worker.duration);
                if route.ready && !ended {
                    gain += route.gain;
                }
                route_state = Some((
                    route.seconds,
                    if route.ready { route.speed } else { 0.0 },
                    route.position,
                    route.window,
                ));
            }
            source
                .worker
                .shared
                .gain
                .store(gain.clamp(0.0, 4.0).to_bits(), Ordering::Release);
            if let Some((base, rate, position, window)) = route_state {
                source
                    .worker
                    .shared
                    .rate
                    .store(rate.to_bits(), Ordering::Release);
                let (target, _) = window.resolve(base, position, source.worker.duration);
                let (start, end) = window.bounds(source.worker.duration);
                source
                    .worker
                    .shared
                    .range_start
                    .store(start.to_bits(), Ordering::Release);
                source
                    .worker
                    .shared
                    .range_end
                    .store(end.to_bits(), Ordering::Release);
                source
                    .worker
                    .shared
                    .looping
                    .store(window.looping, Ordering::Release);
                let shared = &source.worker.shared;
                let predicted = f64::from_bits(shared.playhead.load(Ordering::Acquire));
                let pending = shared.active_epoch.load(Ordering::Acquire)
                    != shared.epoch.load(Ordering::Acquire);
                let length = (end - start).max(1e-9);
                let drift = if window.looping {
                    ((target - predicted + length * 0.5).rem_euclid(length) - length * 0.5).abs()
                } else {
                    (target - predicted).abs()
                };
                if position != source.position
                    || source.window != window
                    || (!paused && !pending && predicted.is_finite() && drift > 0.25)
                {
                    source.position = position;
                    source.window = window;
                    source.worker.seek(target);
                }
            }
        }
        Ok(())
    }
}
impl Drop for CpalAudio {
    fn drop(&mut self) {
        self.playing.store(false, Ordering::Release);
        self.stream = None;
    }
}

#[cfg(test)]
mod tests;
