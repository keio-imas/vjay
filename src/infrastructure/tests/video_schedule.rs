use std::{
    path::Path,
    sync::{Arc, Mutex},
};
use vjay_application::{
    compiler::compile,
    media::{FrameDelivery, MediaAsset, MediaMetrics},
    ports::*,
};
use vjay_domain::{
    builtins::{ids, registry},
    graph::{Connection, Graph},
    node::AssetId,
};
use vjay_infrastructure::{
    gpu::{GpuVideoDecoder, GpuVideoFactory, GpuVideoRuntime, WgpuRuntimeFactory},
    media::FfmpegDecoderFactory,
};

#[derive(Default)]
struct State {
    requests: [bool; 2],
    opens: usize,
}
struct Factory(Arc<Mutex<State>>);
struct Runtime {
    device: wgpu::Device,
    state: Arc<Mutex<State>>,
}
struct Decoder {
    texture: wgpu::Texture,
    id: usize,
    state: Arc<Mutex<State>>,
}
impl GpuVideoFactory for Factory {
    fn create(
        &self,
        device: &wgpu::Device,
        _: &wgpu::Queue,
    ) -> PortResult<Box<dyn GpuVideoRuntime>> {
        Ok(Box::new(Runtime {
            device: device.clone(),
            state: self.0.clone(),
        }))
    }
}
impl GpuVideoRuntime for Runtime {
    fn open(&mut self, _: &Path, e: Extent) -> PortResult<Box<dyn GpuVideoDecoder>> {
        let mut state = self.state.lock().unwrap();
        let id = state.opens;
        state.opens += 1;
        Ok(Box::new(Decoder {
            id,
            state: self.state.clone(),
            texture: self.device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width: e.width,
                    height: e.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            }),
        }))
    }
}
impl GpuVideoDecoder for Decoder {
    fn request_frame(&mut self, _: f64) -> PortResult<()> {
        self.state.lock().unwrap().requests[self.id] = true;
        Ok(())
    }
    fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }
    fn duration(&self) -> f64 {
        1.0
    }
    fn ready(&self) -> bool {
        true
    }
    fn failed(&self) -> bool {
        false
    }
    fn status(&self) -> Option<&str> {
        None
    }
    fn metrics(&self) -> MediaMetrics {
        Default::default()
    }
    fn frame_at(
        &mut self,
        _: f64,
        delivery: FrameDelivery,
        queue: &wgpu::Queue,
    ) -> PortResult<bool> {
        if delivery == FrameDelivery::Exact && self.state.lock().unwrap().requests != [true; 2] {
            return Err("A decoder was waited before all targets were issued".into());
        }
        let color = if self.id == 0 {
            [255, 0, 0, 255]
        } else {
            [0, 0, 255, 255]
        };
        let pixels = color.repeat((self.texture.width() * self.texture.height()) as usize);
        queue.write_texture(
            self.texture.as_image_copy(),
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.texture.width() * 4),
                rows_per_image: Some(self.texture.height()),
            },
            self.texture.size(),
        );
        Ok(true)
    }
}

#[test]
#[ignore = "requires native GPU; verifies decoder fanout and copy elision without timing assumptions"]
fn independent_sources_are_requested_before_waiting_and_forwarded_without_copy_passes() {
    let state = Arc::new(Mutex::new(State::default()));
    let runtime = WgpuRuntimeFactory::headless(
        Arc::new(FfmpegDecoderFactory::new("ffmpeg".into(), "ffprobe".into())),
        vjay_infrastructure::gpu::programs::builtin_programs(),
    )
    .unwrap()
    .with_gpu_video(Some(Arc::new(Factory(state.clone()))));
    let f = registry();
    let mut g = Graph::default();
    let a = g.add(ids::VIDEO, &f).unwrap();
    let b = g.add(ids::VIDEO, &f).unwrap();
    let mix = g.add(ids::MIX, &f).unwrap();
    let out = g.add(ids::OUTPUT, &f).unwrap();
    for (id, asset) in [(a, AssetId(0)), (b, AssetId(1))] {
        g.bind_asset(id, asset, &f).unwrap();
    }
    for (from, to, input) in [(a, mix, 0), (b, mix, 1), (mix, out, 0)] {
        g.connect(Connection {
            from,
            to,
            input,
            output: 0,
        })
        .unwrap();
    }
    let plan = compile(&g, Some(out), &f).unwrap();
    let extent = Extent::new(17, 13).unwrap();
    let mut renderer = runtime.create_renderer().unwrap();
    renderer.set_assets(&[
        MediaAsset {
            id: AssetId(0),
            path: "/fake-a".into(),
        },
        MediaAsset {
            id: AssetId(1),
            path: "/fake-b".into(),
        },
    ]);
    renderer.prepare(&plan, extent).unwrap();
    let metrics = renderer.render_metrics();
    assert_eq!(metrics.forwarded_sources, 2);
    assert_eq!(metrics.image_passes, 1);
    for i in 0..3 {
        state.lock().unwrap().requests = [false; 2];
        renderer
            .render(
                &plan,
                FrameContext {
                    seconds: i as f32 / 60.0,
                    timeline_seconds: i as f64 / 60.0,
                    extent,
                    delivery: FrameDelivery::Exact,
                },
            )
            .unwrap();
        let pixels = renderer.capture().unwrap().rgba;
        assert!(
            pixels
                .as_chunks::<4>()
                .0
                .iter()
                .all(|p| (p[0] as i32 - 128).abs() <= 1
                    && p[1] == 0
                    && (p[2] as i32 - 128).abs() <= 1
                    && p[3] == 255)
        );
    }
    assert_eq!(renderer.render_metrics().submitted_frames, 3);
    assert_eq!(state.lock().unwrap().opens, 2);
}
