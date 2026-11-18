use super::*;
use std::{fs, path::PathBuf};
use vjay_domain::builtins::ids;

struct Clip {
    directory: PathBuf,
    path: PathBuf,
    decoder: PathBuf,
}
impl Clip {
    fn new() -> Self {
        use std::os::unix::fs::PermissionsExt;
        static NEXT_CLIP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let directory = std::env::temp_dir().join(format!(
            "vjay-scrub-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT_CLIP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).unwrap();
        let path = directory.join("clip.mkv");
        assert!(
            Command::new("ffmpeg")
                .args([
                    "-v",
                    "error",
                    "-y",
                    "-f",
                    "lavfi",
                    "-i",
                    "color=c=black:s=32x32:r=10:d=4",
                    "-f",
                    "lavfi",
                    "-i",
                    "aevalsrc=0.1+0.1*t|0.1+0.1*t:s=48000:d=4",
                    "-c:v",
                    "ffv1",
                    "-c:a",
                    "pcm_f32le",
                    "-shortest"
                ])
                .arg(&path)
                .status()
                .unwrap()
                .success()
        );
        let decoder = directory.join("slow-ffmpeg");
        fs::write(&decoder, "#!/bin/sh\nsleep 0.06\nexec ffmpeg \"$@\"\n").unwrap();
        fs::set_permissions(&decoder, fs::Permissions::from_mode(0o700)).unwrap();
        Self {
            directory,
            path,
            decoder,
        }
    }
}
impl Drop for Clip {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

#[test]
#[ignore = "requires ffmpeg and ffprobe; exercises real PCM without opening an audio device"]
fn lfo_position_at_60hz_does_not_starve_audio_when_decoder_startup_is_slower_than_a_frame() {
    let clip = Clip::new();
    let f = builtin_registry();
    let mut graph = Graph::default();
    let video = graph.add(ids::VIDEO, &f).unwrap();
    graph.bind_asset(video, AssetId(0), &f).unwrap();
    let out = graph.add(ids::AUDIO_OUT, &f).unwrap();
    let lfo = graph.add(ids::LFO, &f).unwrap();
    graph.set_parameter(lfo, 1, 0.9, &f).unwrap();
    graph.set_parameter(lfo, 2, 0.05, &f).unwrap();
    for edge in [
        Connection {
            from: video,
            output: 1,
            to: out,
            input: 0,
        },
        Connection {
            from: lfo,
            output: 2,
            to: video,
            input: 1,
        },
    ] {
        graph.connect(edge).unwrap();
    }
    let mut signals = SignalEngine::default();
    signals.rebuild(&graph, &f).unwrap();
    let mut timeline = MediaTimeline::default();
    timeline.rebuild(&graph, &signals).unwrap();
    let mut plan = AudioPlan::compile(&graph, &signals).unwrap();
    let (worker, output) = AudioWorker::open(
        &clip.decoder,
        std::path::Path::new("ffprobe"),
        &clip.path,
        48000,
    )
    .unwrap();
    let mut consumer =
        mixer::SourceConsumer::new(output, worker.shared.clone(), worker.sample_rate);
    let mut backend = CpalAudio {
        device_name: String::new(),
        ffmpeg: clip.decoder.clone(),
        ffprobe: "ffprobe".into(),
        stream: None,
        sources: vec![Source {
            window: Default::default(),
            node: video,
            worker,
            position: f32::NAN,
        }],
        keys: vec![(video, AssetId(0))],
        playing: Arc::new(AtomicBool::new(false)),
        failed: Arc::new(AtomicBool::new(false)),
    };
    let mut non_silent = 0;
    for frame in 0..120 {
        let seconds = frame as f64 / 60.0;
        signals.tick(seconds);
        timeline.tick(seconds, &signals);
        plan.update(&signals, &timeline);
        backend.update(&plan, false).unwrap();
        std::thread::sleep(Duration::from_micros(16_667));
        consumer.begin_buffer();
        for _ in 0..800 {
            let sample = consumer.frame();
            if frame >= 30 && sample[0].abs() > 0.01 {
                non_silent += 1;
            }
        }
        consumer.finish_buffer();
    }
    eprintln!(
        "non-silent frames: {non_silent} / {}; decoder starts: {}",
        90 * 800,
        backend.sources[0]
            .worker
            .shared
            .decoder_starts
            .load(Ordering::Relaxed)
    );
    assert!(
        non_silent > 90 * 800 * 8 / 10,
        "LFO Position modulation starved the callback: {non_silent} / {} non-silent frames",
        90 * 800
    );
}

#[test]
#[ignore = "requires ffmpeg and ffprobe; verifies seek contents without opening an audio device"]
fn pcm_windows_follow_seek_position_cross_loop_boundary_and_reuse_decoded_pages() {
    let clip = Clip::new();
    let shared = Arc::new(decoder::Shared::default());
    let child = Arc::new(std::sync::Mutex::new(None));
    let mut reader = pcm::PcmReader::new(
        "ffmpeg".into(),
        clip.path.clone(),
        48000,
        4.0,
        child,
        shared.clone(),
    );
    let mut frames = vec![[0.0; 2]; pcm::FRAMES];
    reader.read_window(0.125, &mut frames).unwrap();
    assert!(
        (frames[0][0] - 0.1125).abs() < 0.001,
        "wrong seek sample: {:?}",
        frames[0]
    );
    let first = frames.clone();
    reader.read_window(3.99, &mut frames).unwrap();
    assert!((frames[0][0] - 0.499).abs() < 0.001);
    assert!(
        (frames[481][0] - 0.1).abs() < 0.001,
        "loop boundary: {:?}",
        frames[481]
    );
    let wrapped = frames.clone();
    let starts = shared.decoder_starts.load(Ordering::Relaxed);
    for _ in 0..10 {
        reader.read_window(0.125, &mut frames).unwrap();
        assert_eq!(frames, first);
        reader.read_window(3.99, &mut frames).unwrap();
        assert_eq!(frames, wrapped);
    }
    assert_eq!(
        shared.decoder_starts.load(Ordering::Relaxed),
        starts,
        "repeated scrubbing must use cached PCM"
    );
}

#[test]
#[ignore = "requires ffmpeg; verifies optional audio range boundaries"]
fn audio_subrange_wraps_without_reading_past_the_range_and_stops_when_loop_is_off() {
    let clip = Clip::new();
    let shared = Arc::new(decoder::Shared::default());
    shared
        .range_start
        .store(1.0f64.to_bits(), Ordering::Release);
    shared.range_end.store(2.0f64.to_bits(), Ordering::Release);
    let mut reader = pcm::PcmReader::new(
        "ffmpeg".into(),
        clip.path.clone(),
        48000,
        4.0,
        Arc::new(std::sync::Mutex::new(None)),
        shared.clone(),
    );
    let mut frames = vec![[0.0; 2]; pcm::FRAMES];
    reader.read_range(1.99, &mut frames).unwrap();
    assert!((frames[0][0] - 0.299).abs() < 0.001);
    assert!(
        (frames[481][0] - 0.2).abs() < 0.001,
        "wrong subrange wrap: {:?}",
        frames[481]
    );
    assert!(frames.iter().all(|f| f[0] >= 0.199 && f[0] < 0.301));
    shared.looping.store(false, Ordering::Release);
    reader.read_range(1.99, &mut frames).unwrap();
    assert!(frames[482..].iter().all(|f| *f == [0.0; 2]));
}
