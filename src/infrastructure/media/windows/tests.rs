use super::*;
use std::{
    path::Path,
    sync::{Arc, mpsc},
    time::Duration,
};
use vjay_application::{media::FrameDelivery, ports::Extent};

fn fixture(path: &Path) {
    let output = std::process::Command::new("ffmpeg").args([
        "-v", "error", "-f", "lavfi", "-i",
        "color=red:s=1920x1080:r=30:d=1[r];color=blue:s=1920x1080:r=30:d=1[b];[r][b]concat=n=2:v=1:a=0",
        "-c:v", "libx264", "-pix_fmt", "yuv420p", "-g", "30", "-bf", "3", "-threads", "1",
    ]).arg(path).output().expect("FFmpeg on PATH");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn pixel(device: &wgpu::Device, queue: &wgpu::Queue, image: &wgpu::Texture, y: u32) -> [u8; 4] {
    // Deliberate test-only readback checks external synchronization and BGRA order.
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("GPU video test readback"),
        size: 256,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    let mut source = image.as_image_copy();
    source.origin = wgpu::Origin3d {
        x: image.width() / 2,
        y,
        z: 0,
    };
    encoder.copy_texture_to_buffer(
        source,
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(256),
                rows_per_image: Some(1),
            },
        },
        wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
    let submission = queue.submit([encoder.finish()]);
    let (send, receive) = mpsc::sync_channel(1);
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = send.send(result);
        });
    device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: Some(Duration::from_secs(20)),
        })
        .unwrap();
    receive
        .recv_timeout(Duration::from_secs(20))
        .unwrap()
        .unwrap();
    let map = buffer.slice(..).get_mapped_range().unwrap();
    let rgba = [map[2], map[1], map[0], map[3]];
    drop(map);
    buffer.unmap();
    rgba
}

#[test]
#[ignore = "requires a Windows DX12/DXVA GPU, H.264 decoder and FFmpeg; no WARP fallback"]
fn gpu_video_seek_two_sources_without_cpu_pixels() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("動画.mp4");
    fixture(&path);
    let gpu = crate::gpu::WgpuRuntimeFactory::headless_with_backends(
        Arc::new(crate::media::FfmpegDecoderFactory::new(
            "ffmpeg".into(),
            "ffprobe".into(),
        )),
        crate::gpu::programs::builtin_programs(),
        wgpu::Backends::DX12,
    )
    .unwrap();
    let context = gpu.context();
    assert_ne!(
        context.adapter.get_info().device_type,
        wgpu::DeviceType::Cpu
    );
    let mut runtime = Dx12MovieFactory
        .create(&context.device, &context.queue)
        .unwrap();
    let extent = Extent::new(320, 200).unwrap();
    let mut a = runtime.open(&path, extent).unwrap();
    let mut b = runtime.open(&path, extent).unwrap();
    for (time_a, time_b) in [(0.0, 1.5), (1.5, 0.0), (0.0, 1.5), (1.8, 0.2)] {
        a.request_frame(time_a).unwrap();
        b.request_frame(time_b).unwrap();
        runtime.begin_frame().unwrap();
        a.frame_at(time_a, FrameDelivery::Exact, &context.queue)
            .unwrap();
        b.frame_at(time_b, FrameDelivery::Exact, &context.queue)
            .unwrap();
        runtime.end_frame().unwrap();
        for (source, time) in [(&a, time_a), (&b, time_b)] {
            let p = pixel(&context.device, &context.queue, source.texture(), 100);
            assert!(
                if time < 1.0 {
                    p[0] > 220 && p[2] < 30
                } else {
                    p[0] < 30 && p[2] > 220
                },
                "at {time}: {p:?}"
            );
            assert_eq!(p[3], 255);
            assert_eq!(
                pixel(&context.device, &context.queue, source.texture(), 0),
                [0, 0, 0, 255],
                "letterbox"
            );
            assert_eq!(source.metrics().cpu_pixel_bytes, 0);
            assert_eq!(source.metrics().decoder_starts, 1);
        }
    }
    // A seek storm must coalesce to its final target, then keep the last frame
    // available for repeated exact requests without starting another decoder.
    for frame in 0..300 {
        a.request_frame((frame % 60) as f64 / 30.0).unwrap();
    }
    a.frame_at(0.1, FrameDelivery::Exact, &context.queue)
        .unwrap();
    assert!(pixel(&context.device, &context.queue, a.texture(), 100)[0] > 220);
    let count = a.metrics().frames_delivered;
    assert!(
        !a.frame_at(0.1, FrameDelivery::Exact, &context.queue)
            .unwrap()
    );
    assert_eq!(a.metrics().frames_delivered, count);
    let start = std::time::Instant::now();
    drop(a);
    drop(b);
    assert!(
        start.elapsed() < Duration::from_millis(100),
        "node deletion joined native I/O"
    );
}
