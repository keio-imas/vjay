use super::*;
use std::{
    process::Command,
    time::{Duration, Instant},
};
use vjay_application::{media_timeline::MediaTimeline, signals::SignalEngine};
use vjay_domain::builtins::ids;
use vjay_domain::{
    builtins::registry as builtin_registry,
    graph::{Connection, Graph},
};
#[test]
#[ignore = "requires ffmpeg, ffprobe and a native audio output device; emits silence"]
fn native_audio_decodes_modulates_pauses_and_releases_device_only_when_connected() {
    let clip = std::env::temp_dir().join(format!("vjay-audio-test-{}.mkv", std::process::id()));
    assert!(
        Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=black:s=32x32:r=10:d=2",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:duration=2",
                "-c:v",
                "ffv1",
                "-c:a",
                "pcm_f32le",
                "-shortest"
            ])
            .arg(&clip)
            .status()
            .unwrap()
            .success()
    );
    let f = builtin_registry();
    let mut graph = Graph::default();
    let video = graph.add(ids::VIDEO, &f).unwrap();
    let out = graph.add(ids::AUDIO_OUT, &f).unwrap();
    graph.set_parameter(out, 0, 0.0, &f).unwrap();
    graph.bind_asset(video, AssetId(0), &f).unwrap();
    graph
        .connect(Connection {
            from: video,
            output: 1,
            to: out,
            input: 0,
        })
        .unwrap();
    let mut signals = SignalEngine::default();
    signals.rebuild(&graph, &f).unwrap();
    signals.tick(0.0);
    let mut timeline = MediaTimeline::default();
    timeline.rebuild(&graph, &signals).unwrap();
    timeline.tick(0.0, &signals);
    let mut plan = AudioPlan::compile(&graph, &signals).unwrap();
    plan.update(&signals, &timeline);
    let mut backend = CpalAudio {
        device_name: String::new(),
        ffmpeg: "ffmpeg".into(),
        ffprobe: "ffprobe".into(),
        stream: None,
        sources: Vec::new(),
        keys: Vec::new(),
        playing: Arc::new(AtomicBool::new(false)),
        failed: Arc::new(AtomicBool::new(false)),
    };
    assert!(backend.stream.is_none());
    backend
        .prepare(
            &plan,
            &[MediaAsset {
                id: AssetId(0),
                path: clip.clone(),
            }],
        )
        .unwrap();
    backend.update(&plan, false).unwrap();
    let start = Instant::now();
    while backend.sources[0]
        .worker
        .shared
        .consumed
        .load(Ordering::Acquire)
        == 0
        && start.elapsed() < Duration::from_secs(3)
    {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        backend.sources[0]
            .worker
            .shared
            .consumed
            .load(Ordering::Acquire)
            > 0
    );
    backend.update(&plan, true).unwrap();
    std::thread::sleep(Duration::from_millis(50));
    let paused = backend.sources[0]
        .worker
        .shared
        .consumed
        .load(Ordering::Acquire);
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(
        paused,
        backend.sources[0]
            .worker
            .shared
            .consumed
            .load(Ordering::Acquire)
    );
    // Exercise the real CPAL callback under the same 60 Hz Position modulation.
    // Volume stays zero: this validates the device path without emitting sound.
    let lfo = graph.add(ids::LFO, &f).unwrap();
    graph
        .connect(Connection {
            from: lfo,
            output: 2,
            to: video,
            input: 1,
        })
        .unwrap();
    signals.rebuild(&graph, &f).unwrap();
    timeline.rebuild(&graph, &signals).unwrap();
    let mut plan = AudioPlan::compile(&graph, &signals).unwrap();
    let before = backend.sources[0]
        .worker
        .shared
        .consumed
        .load(Ordering::Acquire);
    for frame in 0..90 {
        let time = frame as f64 / 60.0;
        signals.tick(time);
        timeline.tick(time, &signals);
        plan.update(&signals, &timeline);
        backend.update(&plan, false).unwrap();
        std::thread::sleep(Duration::from_micros(16_667));
    }
    let supplied = backend.sources[0]
        .worker
        .shared
        .consumed
        .load(Ordering::Acquire)
        - before;
    assert!(
        supplied > backend.sources[0].worker.sample_rate as u64 * 3 / 4,
        "native callback starved during Position modulation: {supplied}"
    );
    backend.prepare(&AudioPlan::default(), &[]).unwrap();
    assert!(backend.stream.is_none());
    assert!(backend.sources.is_empty());
    std::fs::remove_file(clip).unwrap();
}

#[cfg(unix)]
mod scrub;
