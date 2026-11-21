use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use vjay_application::{
    compiler::compile_with_previews,
    media::FrameDelivery,
    ports::*,
    project::{Project, ProjectDocument},
    signals::SignalEngine,
};
use vjay_domain::{builtins::registry, node::NodeFactory};
use vjay_infrastructure::{
    gpu::WgpuRuntimeFactory,
    media::FfmpegDecoderFactory,
    text_images::{PreparedImageRegistry, TextImageProvider},
};

fn samples() -> Vec<PathBuf> {
    let mut files: Vec<_> =
        std::fs::read_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../sample"))
            .unwrap()
            .map(|p| p.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .collect();
    files.sort();
    assert_eq!(
        files
            .iter()
            .filter(|p| p
                .file_name()
                .unwrap()
                .to_string_lossy()
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_digit()))
            .count(),
        10,
        "Keep the ten shipped sample projects loadable"
    );
    assert!(
        files
            .iter()
            .any(|p| p.file_name().unwrap() == "anime-club.vjay.json")
    );
    files
}

fn load(path: &Path) -> Project {
    let document: ProjectDocument = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    Project::restore(document, &registry()).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn samples_restore_compile_and_expose_editable_controls_without_external_assets() {
    let nodes = registry();
    for path in samples() {
        let project = load(&path);
        assert!(project.assets.is_empty());
        assert!(project.controls.len() >= 4, "{}", path.display());
        let mut plan = compile_with_previews(&project.graph, project.output, &nodes).unwrap();
        assert!(plan.program_active);
        let mut signals = SignalEngine::default();
        signals.rebuild(&project.graph, &nodes).unwrap();
        signals.bind(&mut plan);
        for node in project.graph.nodes() {
            let definition = nodes.get(&node.type_id).unwrap();
            assert!(node.asset.is_none());
            assert!(definition.asset_source(&node.settings).is_none());
            assert!(definition.service().is_none());
        }
        for control in &project.controls {
            if control.target == vjay_application::project::ControlTarget::Output {
                continue;
            }
            assert!(
                signals
                    .parameter(control.node, control.parameter)
                    .unwrap()
                    .source
                    .is_none(),
                "{}: {} must remain editable",
                path.display(),
                control.label
            );
        }
        for i in 0..91 {
            signals.tick(i as f64 / 60.0);
            signals.modulate(&mut plan);
            assert!(
                plan.steps
                    .iter()
                    .all(|s| s.operation.parameters.iter().all(|v| v.is_finite()))
            );
        }
    }
}

#[test]
#[ignore = "requires a native GPU and installed fonts"]
fn every_sample_animates_and_renders_all_outputs_and_the_title_button_works() {
    let nodes = registry();
    let runtime = WgpuRuntimeFactory::headless(
        Arc::new(FfmpegDecoderFactory::new("ffmpeg".into(), "ffprobe".into())),
        vjay_infrastructure::gpu::programs::builtin_programs(),
    )
    .unwrap()
    .with_prepared_images(Arc::new(PreparedImageRegistry(vec![Box::new(
        TextImageProvider::default(),
    )])));
    let mut renderer = runtime.create_renderer().unwrap();
    let extent = Extent::new(640, 360).unwrap();
    for path in samples() {
        let project = load(&path);
        let mut plan = compile_with_previews(&project.graph, project.output, &nodes).unwrap();
        let mut signals = SignalEngine::default();
        signals.rebuild(&project.graph, &nodes).unwrap();
        signals.bind(&mut plan);
        signals.tick(0.0);
        signals.modulate(&mut plan);
        renderer.prepare(&plan, extent).unwrap();
        let frame = |seconds| FrameContext {
            seconds: seconds as f32,
            timeline_seconds: seconds,
            extent,
            delivery: FrameDelivery::Exact,
        };
        renderer.render(&plan, frame(0.0)).unwrap();
        let initial = renderer.capture().unwrap().rgba;
        for i in 1..=91 {
            let seconds = i as f64 / 60.0;
            signals.tick(seconds);
            signals.modulate(&mut plan);
            renderer.render(&plan, frame(seconds)).unwrap();
        }
        let pixels = renderer.capture().unwrap().rgba;
        assert_ne!(initial, pixels, "{} must animate", path.display());
        let mut outputs = Vec::new();
        for output in &plan.outputs {
            let image = renderer.capture_output(output.node).unwrap();
            let min = image
                .rgba
                .as_chunks::<4>()
                .0
                .iter()
                .flat_map(|p| &p[..3])
                .min()
                .unwrap();
            let max = image
                .rgba
                .as_chunks::<4>()
                .0
                .iter()
                .flat_map(|p| &p[..3])
                .max()
                .unwrap();
            assert!(
                max - min > 40,
                "{}: output must contain an image",
                path.display()
            );
            outputs.push(image.rgba);
        }
        if outputs.len() == 2 {
            assert_ne!(outputs[0], outputs[1]);
        }
        if path.file_name().unwrap() == "06-live-type.vjay.json" {
            let button = project
                .controls
                .iter()
                .find(|c| c.label == "PUNCH")
                .unwrap();
            signals
                .events
                .push(vjay_application::control_io::ControlEvent::Trigger(
                    button.node,
                ));
            for i in 92..=96 {
                signals.tick(i as f64 / 60.0);
                signals.modulate(&mut plan);
            }
            let composite = plan
                .steps
                .iter()
                .find(|s| s.operation.parameters.len() == 6 && s.operation.parameters[2] > 1.01);
            assert!(composite.is_some(), "PUNCH must reach the image scale");
        }
    }
}
