use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use vjay_application::{compiler::compile, media::FrameDelivery, ports::*};
use vjay_domain::{
    builtins::{ids, registry},
    execution::StreamRequest,
    graph::{Connection, Graph},
};
use vjay_infrastructure::{
    gpu::WgpuRuntimeFactory,
    media::FfmpegDecoderFactory,
    video_io::{VideoIoFactory, VideoIoRegistry},
};
fn request(provider: &'static str, name: String) -> StreamRequest {
    StreamRequest {
        provider,
        name,
        options: Default::default(),
    }
}
fn loopback(provider: &'static str, factory: Arc<dyn VideoIoFactory>) {
    let runtime = WgpuRuntimeFactory::headless(
        Arc::new(FfmpegDecoderFactory::new("ffmpeg".into(), "ffprobe".into())),
        vjay_infrastructure::gpu::programs::builtin_programs(),
    )
    .unwrap()
    .with_video_io(Arc::new(VideoIoRegistry(vec![(provider, factory.clone())])));
    let context = runtime.context();
    let tx_extent = Extent::new(128, 72).unwrap();
    let output_extent = Extent::new(64, 40).unwrap();
    let image = context.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("loopback source"),
        size: wgpu::Extent3d {
            width: 128,
            height: 72,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let name = format!("VJAY-loopback-{}", std::process::id());
    let mut sender = factory
        .sender(
            &request(provider, name.clone()),
            &context.device,
            &context.queue,
            tx_extent,
        )
        .unwrap();
    {
        let mut encoder = context.device.create_command_encoder(&Default::default());
        sender.encode(&mut encoder, &image).unwrap();
        context.queue.submit([encoder.finish()]);
        sender.submitted(&context.device, &context.queue).unwrap();
    }
    let started = Instant::now();
    let source = if let Some(name) = sender.source_name() {
        name.to_owned()
    } else {
        loop {
            let sources = factory.sources();
            if let Some(source) = sources.into_iter().find(|n| n.contains(&name)) {
                break source;
            }
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "Syphon discovery timed out"
            );
            pump();
        }
    };
    let nodes = registry();
    let mut graph = Graph::default();
    let input = graph
        .add(
            if provider == "ndi" {
                "NdiIn"
            } else {
                "SyphonIn"
            },
            &nodes,
        )
        .unwrap();
    let out = graph.add(ids::OUTPUT, &nodes).unwrap();
    let mut settings = graph.node(input).unwrap().settings.clone();
    settings.properties.insert("source".into(), source);
    graph.set_settings(input, settings, &nodes).unwrap();
    graph
        .connect(Connection {
            from: input,
            output: 0,
            to: out,
            input: 0,
        })
        .unwrap();
    let plan = compile(&graph, out, &nodes).unwrap();
    let mut renderer = runtime.create_renderer().unwrap();
    renderer.prepare(&plan, output_extent).unwrap();
    let frame = FrameContext {
        seconds: 0.0,
        timeline_seconds: 0.0,
        extent: output_extent,
        delivery: FrameDelivery::Latest,
    };
    loop {
        let mut encoder = context.device.create_command_encoder(&Default::default());
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &image.create_view(&Default::default()),
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 1.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
        }
        sender.encode(&mut encoder, &image).unwrap();
        context.queue.submit([encoder.finish()]);
        sender.submitted(&context.device, &context.queue).unwrap();
        renderer.render(&plan, frame).unwrap();
        let bytes = renderer.capture().unwrap().rgba;
        if bytes[0] > 220 && bytes[1] < 20 && bytes[2] < 20 {
            break;
        }
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "{provider} loopback did not produce red pixels: {:?}",
            &bytes[..4]
        );
        pump();
    }
}
fn pump() {
    #[cfg(target_os = "macos")]
    unsafe {
        use objc2::{msg_send, rc::Retained, runtime::AnyObject};
        let date: Retained<AnyObject> =
            msg_send![objc2::class!(NSDate),dateWithTimeIntervalSinceNow:0.02f64];
        let runloop: Retained<AnyObject> = msg_send![objc2::class!(NSRunLoop), currentRunLoop];
        let _: () = msg_send![&*runloop,runUntilDate:&*date];
    }
    #[cfg(not(target_os = "macos"))]
    std::thread::sleep(Duration::from_millis(20));
}
#[test]
#[ignore = "requires native GPU and NDI Runtime; performs localhost NDI loopback"]
fn ndi_gpu_output_and_input_loopback_scale_pixels() {
    let path = std::env::var_os("NDI_RUNTIME_PATH")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| "/usr/local/lib/libndi.dylib".into());
    let f = vjay_infrastructure::video_io::ndi::NdiFactory::new(&path).unwrap();
    loopback("ndi", Arc::new(f));
}
#[test]
#[ignore = "requires macOS Metal and built Syphon framework"]
#[cfg(target_os = "macos")]
fn syphon_gpu_output_and_input_loopback_scale_pixels() {
    let path = std::env::var_os("SYPHON_FRAMEWORK_PATH")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/syphon/Syphon.framework/Syphon")
        });
    let f = vjay_infrastructure::video_io::syphon::SyphonFactory::new(&path).unwrap();
    loopback("syphon", Arc::new(f));
}
