mod args;

use std::process::ExitCode;
use std::sync::Arc;
use vjay_application::media::VideoDecoderFactory;
use vjay_application::{
    ports::{Platform, PortResult, SessionFactory},
    session::Session,
};
use vjay_domain::builtins::registry as builtin_registry;
use vjay_infrastructure::media::FfmpegDecoderFactory;
use vjay_infrastructure::{
    gpu::WgpuRuntimeFactory,
    platform::{DesktopRuntime, GpuiPlatform, HeadlessPlatform},
    storage::JsonProjectStore,
};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("vjay: {error}");
            ExitCode::FAILURE
        }
    }
}

// the main function (global entry-point) is the dirtiest job, tasked with recognizing all dependencies and injecting them into each component.
fn run() -> PortResult<()> {
    let Some(args) = args::Args::parse()? else {
        return Ok(());
    };
    let mut video_io = vjay_infrastructure::video_io::VideoIoRegistry::default();
    let ndi_path = std::env::var_os("NDI_RUNTIME_PATH")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(if cfg!(target_os = "windows") {
                "Processing.NDI.Lib.x64.dll"
            } else if cfg!(target_os = "macos") {
                "/usr/local/lib/libndi.dylib"
            } else {
                "libndi.so"
            })
        });
    let ndi: Arc<dyn vjay_infrastructure::video_io::VideoIoFactory> =
        match vjay_infrastructure::video_io::ndi::NdiFactory::new(&ndi_path) {
            Ok(f) => Arc::new(f),
            Err(e) => Arc::new(vjay_infrastructure::video_io::Unavailable(e)),
        };
    video_io.0.push(("ndi", ndi));
    #[cfg(target_os = "macos")]
    {
        let path = std::env::var_os("SYPHON_FRAMEWORK_PATH")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                let bundled = std::env::current_exe().ok().and_then(|p| {
                    p.parent()?
                        .parent()
                        .map(|p| p.join("Frameworks/Syphon.framework/Syphon"))
                });
                bundled.filter(|p| p.is_file()).unwrap_or_else(|| {
                    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("target/syphon/Syphon.framework/Syphon")
                })
            });
        let syphon: Arc<dyn vjay_infrastructure::video_io::VideoIoFactory> =
            match vjay_infrastructure::video_io::syphon::SyphonFactory::new(&path) {
                Ok(f) => Arc::new(f),
                Err(e) => Arc::new(vjay_infrastructure::video_io::Unavailable(e)),
            };
        video_io.0.push(("syphon", syphon));
        video_io.0.push((
            "spout",
            Arc::new(vjay_infrastructure::video_io::Unavailable(
                "Spout is Windows-only; use Syphon IN/OUT on macOS".into(),
            )),
        ));
    }
    #[cfg(target_os = "windows")]
    {
        let path = std::env::var_os("SPOUT_BRIDGE_PATH")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("target/spout/Release/vjay_spout_bridge.dll")
            });
        let spout: Arc<dyn vjay_infrastructure::video_io::VideoIoFactory> =
            match vjay_infrastructure::video_io::spout::SpoutFactory::new(&path) {
                Ok(f) => Arc::new(f),
                Err(e) => Arc::new(vjay_infrastructure::video_io::Unavailable(e)),
            };
        video_io.0.push(("spout", spout));
    }
    #[cfg(target_os = "macos")]
    video_io.0.push((
        "capture",
        Arc::new(vjay_infrastructure::video_io::camera::CameraFactory),
    ));
    #[cfg(not(target_os = "macos"))]
    video_io.0.push((
        "capture",
        Arc::new(vjay_infrastructure::video_io::Unavailable(
            "Camera capture is currently supported on macOS".into(),
        )),
    ));
    let settings: Arc<dyn vjay_application::preferences::SettingsStore> =
        Arc::new(vjay_infrastructure::preferences::JsonSettingsStore::new(
            vjay_infrastructure::preferences::default_path(),
        ));
    let headless_settings = settings.clone();
    let video_io = Arc::new(video_io);
    let headless_video_io = video_io.clone();
    let service_factories: Arc<Vec<Box<dyn vjay_infrastructure::services::ServiceFactory>>> =
        Arc::new(vec![
            Box::new(vjay_infrastructure::services::link::LinkFactory),
            Box::new(vjay_infrastructure::services::rekordbox::RekordboxFactory {
                paths: vjay_infrastructure::services::rekordbox::RekordboxPaths::discover(),
            }),
        ]);
    let headless_services = service_factories.clone();
    let text_images: Arc<dyn vjay_application::media::PreparedImageFactory> = Arc::new(
        vjay_infrastructure::text_images::PreparedImageRegistry(vec![Box::new(
            vjay_infrastructure::text_images::TextImageProvider::default(),
        )]),
    );
    let headless_text = text_images.clone();
    let web_assets: Arc<dyn vjay_application::asset_sources::AssetSourceFactory> = Arc::new(
        vjay_infrastructure::web_media::YtdlpAssetFactory::new(args.ytdlp, args.ffmpeg.clone()),
    );
    let files: Arc<dyn vjay_application::asset_sources::AssetSourceFactory> =
        Arc::new(vjay_infrastructure::file_collection::FileCollectionFactory::default());
    let web_assets: Arc<dyn vjay_application::asset_sources::AssetSourceFactory> = Arc::new(
        vjay_infrastructure::file_collection::AssetSourceRegistry(vec![
            ("web-video", web_assets),
            ("video-collection", files.clone()),
            ("file-collection", files),
        ]),
    );
    let headless_web = web_assets.clone();
    let audio_ffmpeg = args.ffmpeg.clone();
    let audio_ffprobe = args.ffprobe.clone();
    #[cfg(target_os = "macos")]
    let native_video: Option<Arc<dyn vjay_infrastructure::gpu::GpuVideoFactory>> = Some(Arc::new(
        vjay_infrastructure::media::macos::MetalMovieFactory::new(
            args.ffmpeg.clone(),
            args.ffprobe.clone(),
        ),
    ));
    #[cfg(not(target_os = "macos"))]
    let native_video: Option<Arc<dyn vjay_infrastructure::gpu::GpuVideoFactory>> = None;
    #[cfg(target_os = "macos")]
    let media: Arc<dyn VideoDecoderFactory> = Arc::new(FfmpegDecoderFactory::videotoolbox(
        args.ffmpeg,
        args.ffprobe,
    ));
    #[cfg(not(target_os = "macos"))]
    let media: Arc<dyn VideoDecoderFactory> =
        Arc::new(FfmpegDecoderFactory::new(args.ffmpeg, args.ffprobe));
    // The composition root is the only place that selects concrete adapters.
    let platform: Box<dyn Platform> = if args.headless {
        Box::new(HeadlessPlatform {
            runtime: Box::new(
                WgpuRuntimeFactory::headless_with_backends(
                    media.clone(),
                    vjay_infrastructure::gpu::programs::builtin_programs(),
                    if cfg!(target_os = "windows") {
                        wgpu::Backends::DX12
                    } else {
                        wgpu::Backends::all()
                    },
                )?
                .with_settings(headless_settings)
                .with_gpu_video(native_video.clone())
                .with_images(Arc::new(vjay_infrastructure::images::RasterImageLoader))
                .with_prepared_images(headless_text)
                .with_video_io(headless_video_io)
                .with_asset_sources(headless_web)
                .with_controls(move || {
                    Box::new(vjay_infrastructure::services::ServiceControlIo::new(
                        Box::new(vjay_application::control_io::DisabledControlIo),
                        headless_services.clone(),
                    ))
                }),
            ),
            frames: args.frames,
            output: args.output,
        })
    } else {
        Box::new(
            GpuiPlatform::new(move |window| {
                #[cfg(target_os = "macos")]
                {
                    use std::{cell::RefCell, rc::Rc};
                    use vjay_infrastructure::{
                        gpu::presentation_group::PresentationGroup,
                        macos::{MacGpuPreview, MacOutputFactory},
                        platform::SharedPreview,
                    };
                    let group = Rc::new(RefCell::new(PresentationGroup::new(Box::new(
                        MacGpuPreview::attach(window)?,
                    ))));
                    let preview: SharedPreview = group.clone();
                    let runtime = WgpuRuntimeFactory::for_desktop(
                        preview.clone(),
                        media.clone(),
                        vjay_infrastructure::gpu::programs::builtin_programs(),
                    )?
                    .with_settings(settings)
                    .with_images(Arc::new(vjay_infrastructure::images::RasterImageLoader))
                    .with_prepared_images(text_images)
                    .with_gpu_video(native_video.clone())
                    .with_video_io(video_io)
                    .with_asset_sources(web_assets)
                    .with_controls(move || {
                        Box::new(vjay_infrastructure::services::ServiceControlIo::new(
                            Box::new(vjay_infrastructure::control_io::NativeControlIo::default()),
                            service_factories.clone(),
                        ))
                    })
                    .with_audio(Rc::new(
                        vjay_infrastructure::audio::CpalAudioFactory::new(
                            audio_ffmpeg,
                            audio_ffprobe,
                        ),
                    ));
                    group.borrow_mut().set_node_factory(Box::new(
                        vjay_infrastructure::macos::MacNodePreviewFactory::new(
                            window,
                            runtime.context(),
                        )?,
                    ));
                    let outputs = Rc::new(MacOutputFactory::new(runtime.context(), group));
                    Ok(DesktopRuntime {
                        factory: Box::new(runtime),
                        preview: Some(preview),
                        outputs: Some(outputs),
                    })
                }
                #[cfg(not(target_os = "macos"))]
                {
                    let _ = window;
                    Ok(DesktopRuntime {
                        factory: Box::new(
                            WgpuRuntimeFactory::headless_with_backends(
                                media.clone(),
                                vjay_infrastructure::gpu::programs::builtin_programs(),
                                if cfg!(target_os = "windows") {
                                    wgpu::Backends::DX12
                                } else {
                                    wgpu::Backends::all()
                                },
                            )?
                            .with_settings(settings)
                            .with_images(Arc::new(vjay_infrastructure::images::RasterImageLoader))
                            .with_prepared_images(text_images)
                            .with_video_io(video_io)
                            .with_asset_sources(web_assets)
                            .with_controls(move || {
                                Box::new(vjay_infrastructure::services::ServiceControlIo::new(
                                    Box::new(
                                        vjay_infrastructure::control_io::NativeControlIo::default(),
                                    ),
                                    service_factories.clone(),
                                ))
                            })
                            .with_audio(std::rc::Rc::new(
                                vjay_infrastructure::audio::CpalAudioFactory::new(
                                    audio_ffmpeg,
                                    audio_ffprobe,
                                ),
                            )),
                        ),
                        preview: None,
                        outputs: None,
                    })
                }
            })
            .with_output_window(args.output_window),
        )
    };
    let create: SessionFactory = Box::new(move |runtime| {
        let load = args.project.exists();
        let mut session = Session::new(
            Box::new({
                let mut nodes = builtin_registry();
                nodes.register(vjay_infrastructure::services::nodes::AbletonLinkNode)?;
                nodes.register(vjay_infrastructure::services::nodes::RekordboxHistoryNode)?;
                nodes
            }),
            runtime,
            Box::new(JsonProjectStore::new(args.project)),
            args.extent,
        )?;
        if let Err(e) = session.load_preferences() {
            eprintln!(
                "Settings could not be applied: {e}. Open Settings to choose an available device."
            );
        }
        if !args.size_explicit {
            let video = &session.preferences().video;
            session.set_extent(vjay_application::ports::Extent::new(
                video.width,
                video.height,
            )?)?;
        }
        if load {
            session.load()?;
        }
        if let Some(video) = args.video {
            let path = video
                .canonicalize()
                .map_err(|e| format!("Cannot open video: {e}"))?;
            session.import_video(path, None)?;
        }
        Ok(session)
    });
    platform.run(create)
}
