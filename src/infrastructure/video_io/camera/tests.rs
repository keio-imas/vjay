use super::*;
#[test]
#[ignore = "requires a native Metal GPU; uses a GPU-generated IOSurface, no camera permission"]
fn iosurface_camera_frames_scale_and_convert_on_gpu() {
    let runtime = crate::gpu::WgpuRuntimeFactory::headless(
        Arc::new(crate::media::FfmpegDecoderFactory::new(
            "ffmpeg".into(),
            "ffprobe".into(),
        )),
        crate::gpu::programs::builtin_programs(),
    )
    .unwrap();
    let context = runtime.context();
    let extent = Extent::new(64, 40).unwrap();
    let request = StreamRequest {
        provider: "capture",
        name: "synthetic".into(),
        options: Default::default(),
    };
    let mut receiver =
        Receiver::new(&request, &context.device, &context.queue, extent, true).unwrap();
    let mut encoder = context.device.create_command_encoder(&Default::default());
    receiver
        .update(&context.device, &context.queue, &mut encoder)
        .unwrap();
    let output = context.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 256 * 40,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    // Use a blit to a COPY_SRC target because the persistent capture input is not a readback texture.
    let target = context.device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: 64,
            height: 40,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let mut blit = crate::gpu::blit::Blit::new(&context.device, wgpu::TextureFormat::Rgba8Unorm);
    blit.prepare(
        &context.device,
        &receiver.texture().create_view(&Default::default()),
    );
    blit.encode(&mut encoder, &target.create_view(&Default::default()));
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &output,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(256),
                rows_per_image: Some(40),
            },
        },
        target.size(),
    );
    context.queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    output
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
    context
        .device
        .poll(wgpu::PollType::wait_indefinitely())
        .unwrap();
    rx.recv().unwrap().unwrap();
    let bytes = output.slice(..).get_mapped_range().unwrap();
    assert!(
        bytes
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| *p == [255, 0, 0, 255]),
        "first={:?}, center={:?}, last={:?}",
        &bytes[..4],
        &bytes[20 * 256 + 32 * 4..20 * 256 + 32 * 4 + 4],
        &bytes[39 * 256 + 63 * 4..][..4]
    );
}
