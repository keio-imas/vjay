use std::{
    path::Path,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use vjay_application::{
    compiler::{compile, compile_with_previews},
    media::{FrameDelivery, ImageLoader, MediaAsset, PreparedImageFactory},
    ports::*,
};
use vjay_domain::{
    builtins::{ids, registry},
    execution::PreparedImage,
    graph::{Connection, Graph},
    node::AssetId,
};
use vjay_infrastructure::{gpu::WgpuRuntimeFactory, media::FfmpegDecoderFactory};
fn runtime() -> WgpuRuntimeFactory {
    WgpuRuntimeFactory::headless(
        Arc::new(FfmpegDecoderFactory::new("ffmpeg".into(), "ffprobe".into())),
        vjay_infrastructure::gpu::programs::builtin_programs(),
    )
    .unwrap()
}
struct Blocked {
    gate: Arc<(Mutex<bool>, Condvar)>,
}
impl ImageLoader for Blocked {
    fn load(&self, path: &Path, e: Extent) -> PortResult<Vec<u8>> {
        let lock = self.gate.0.lock().unwrap();
        let _ = self
            .gate
            .1
            .wait_timeout_while(lock, Duration::from_secs(2), |open| !*open)
            .unwrap();
        let c = if path == Path::new("/a") {
            [255, 0, 0, 255]
        } else {
            [0, 0, 255, 255]
        };
        Ok(c.repeat((e.width * e.height) as usize))
    }
}
#[test]
#[ignore = "requires native GPU; tests asynchronous image replacement and last completed frame"]
fn latest_frames_keep_rendering_while_an_image_worker_is_blocked() {
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let runtime = runtime().with_images(Arc::new(Blocked { gate: gate.clone() }));
    let mut r = runtime.create_renderer().unwrap();
    let f = registry();
    let mut g = Graph::default();
    let image = g.add("Image", &f).unwrap();
    g.bind_asset(image, AssetId(0), &f).unwrap();
    let out = g.add(ids::OUTPUT, &f).unwrap();
    g.connect(Connection {
        from: image,
        to: out,
        input: 0,
        output: 0,
    })
    .unwrap();
    let plan = compile(&g, Some(out), &f).unwrap();
    let extent = Extent::new(17, 13).unwrap();
    let frame = |delivery| FrameContext {
        seconds: 0.0,
        timeline_seconds: 0.0,
        extent,
        delivery,
    };
    r.set_assets(&[MediaAsset {
        id: AssetId(0),
        path: "/a".into(),
    }]);
    r.prepare(&plan, extent).unwrap();
    let begin = Instant::now();
    r.render(&plan, frame(FrameDelivery::Latest)).unwrap();
    assert!(begin.elapsed() < Duration::from_millis(100));
    *gate.0.lock().unwrap() = true;
    gate.1.notify_all();
    r.render(&plan, frame(FrameDelivery::Exact)).unwrap();
    assert_eq!(&r.capture().unwrap().rgba[..4], &[255, 0, 0, 255]);
    *gate.0.lock().unwrap() = false;
    r.set_assets(&[MediaAsset {
        id: AssetId(0),
        path: "/b".into(),
    }]);
    r.prepare(&plan, extent).unwrap();
    r.render(&plan, frame(FrameDelivery::Latest)).unwrap();
    assert_eq!(&r.capture().unwrap().rgba[..4], &[255, 0, 0, 255]);
    *gate.0.lock().unwrap() = true;
    gate.1.notify_all();
    r.render(&plan, frame(FrameDelivery::Exact)).unwrap();
    assert_eq!(&r.capture().unwrap().rgba[..4], &[0, 0, 255, 255]);
    assert_eq!(r.render_metrics().forwarded_sources, 1);
    assert_eq!(r.render_metrics().image_passes, 0);
}
struct Generated(AtomicUsize);
impl PreparedImageFactory for Generated {
    fn load(&self, _: &PreparedImage, e: Extent) -> PortResult<Vec<u8>> {
        self.0.fetch_add(1, Ordering::Relaxed);
        Ok(vec![255; (e.width * e.height * 4) as usize])
    }
}
#[test]
#[ignore = "requires native GPU; exact delivery must drain more jobs than queue capacity"]
fn exact_delivery_drains_a_saturated_image_queue_without_deadlock() {
    let generated = Arc::new(Generated(AtomicUsize::new(0)));
    let runtime = runtime().with_prepared_images(generated.clone());
    let mut r = runtime.create_renderer().unwrap();
    let f = registry();
    let mut g = Graph::default();
    for i in 0..40 {
        let n = g.add("Text", &f).unwrap();
        let mut settings = g.node(n).unwrap().settings.clone();
        settings
            .properties
            .insert("content.text".into(), format!("Text {i}"));
        g.set_settings(n, settings, &f).unwrap();
    }
    let plan = compile_with_previews(&g, None, &f).unwrap();
    let extent = Extent::new(17, 13).unwrap();
    r.prepare(&plan, extent).unwrap();
    r.render(
        &plan,
        FrameContext {
            seconds: 0.0,
            timeline_seconds: 0.0,
            extent,
            delivery: FrameDelivery::Exact,
        },
    )
    .unwrap();
    assert_eq!(generated.0.load(Ordering::Relaxed), 40);
    r.render(
        &plan,
        FrameContext {
            seconds: 1.0,
            timeline_seconds: 1.0,
            extent,
            delivery: FrameDelivery::Exact,
        },
    )
    .unwrap();
    assert_eq!(generated.0.load(Ordering::Relaxed), 40);
}

struct SlowGenerated;
impl PreparedImageFactory for SlowGenerated {
    fn load(&self, source: &PreparedImage, e: Extent) -> PortResult<Vec<u8>> {
        std::thread::sleep(Duration::from_millis(35));
        let value = source
            .properties
            .get("content.text")
            .and_then(|s| s.parse::<u8>().ok())
            .unwrap_or(1);
        Ok([value, 0, 0, 255].repeat((e.width * e.height) as usize))
    }
}
#[test]
#[ignore = "requires native GPU; rapidly edited generated images must not starve"]
fn generated_revisions_make_progress_when_edits_outpace_rasterization() {
    let runtime = runtime().with_prepared_images(Arc::new(SlowGenerated));
    let mut r = runtime.create_renderer().unwrap();
    let f = registry();
    let mut g = Graph::default();
    let text = g.add("Text", &f).unwrap();
    let out = g.add(ids::OUTPUT, &f).unwrap();
    g.connect(Connection {
        from: text,
        to: out,
        input: 0,
        output: 0,
    })
    .unwrap();
    let mut plan = compile(&g, Some(out), &f).unwrap();
    let extent = Extent::new(17, 13).unwrap();
    r.prepare(&plan, extent).unwrap();
    let frame = |delivery| FrameContext {
        seconds: 0.0,
        timeline_seconds: 0.0,
        extent,
        delivery,
    };
    let mut displayed = 0;
    let mut changes = 0;
    for i in 1..=40 {
        let step = &mut plan.steps[0];
        step.prepared_revision += 1;
        step.operation
            .prepared
            .as_mut()
            .unwrap()
            .properties
            .insert("content.text".into(), i.to_string());
        r.render(&plan, frame(FrameDelivery::Latest)).unwrap();
        let value = r.capture().unwrap().rgba[0];
        if value > displayed {
            changes += 1;
            displayed = value;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        changes >= 3,
        "no progress while requests were coalesced: {changes}"
    );
    r.render(&plan, frame(FrameDelivery::Exact)).unwrap();
    assert_eq!(r.capture().unwrap().rgba[0], 40);
}
