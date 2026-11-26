use super::*;
use ::windows::{
    Win32::{
        Foundation::{HWND, LPARAM, WPARAM},
        Graphics::Gdi::*,
        UI::WindowsAndMessaging::*,
    },
    core::w,
};

fn bounds(scale: f32) -> PreviewBounds {
    PreviewBounds {
        x: 12.0,
        y: 18.0,
        width: 128.0,
        height: 72.0,
        window_height: 400.0,
        scale,
    }
}

#[test]
fn dpi_coordinates_reject_invalid_bounds_and_allow_empty_or_offscreen_viewports() {
    assert_eq!(
        clipping::physical_bounds(bounds(1.5)).unwrap(),
        [18, 27, 192, 108]
    );
    assert!(clipping::physical_bounds(bounds(f32::NAN)).is_err());
    assert!(clipping::physical_bounds(bounds(0.0)).is_err());
    let mut rect = bounds(2.0);
    rect.x = -20.0;
    rect.height = 0.0;
    assert_eq!(clipping::physical_bounds(rect).unwrap(), [-40, 36, 256, 0]);
    rect.width = f32::MAX;
    assert!(clipping::physical_bounds(rect).is_err());
}

struct Host(HWND);
impl Host {
    fn new() -> Self {
        // SAFETY: A private test-owned native parent, created on this test thread.
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_NOACTIVATE,
                w!("STATIC"),
                w!("VJAY GPU test"),
                WS_OVERLAPPEDWINDOW,
                40,
                40,
                400,
                300,
                None,
                None,
                None,
                None,
            )
            .unwrap()
        };
        Self(hwnd)
    }
    fn close(&mut self) {
        if !self.0.is_invalid() {
            // SAFETY: The test owns this HWND and all its children.
            unsafe {
                DestroyWindow(self.0).unwrap();
            }
            self.0 = HWND::default();
        }
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        self.close();
    }
}

#[test]
#[ignore = "requires a Windows desktop; checks HWND clipping, input routing and parent destruction"]
fn child_clipping_and_hit_testing_keep_patch_input_on_the_parent() {
    let mut host = Host::new();
    let mut child = ChildWindow::new(host.0).unwrap();
    child.set_bounds(bounds(2.0)).unwrap();
    child
        .set_regions(&[PreviewRegion {
            x: 4.0,
            y: 6.0,
            width: 16.0,
            height: 12.0,
        }])
        .unwrap();
    let handle = child.hwnd();
    // SAFETY: Read-only inspection of the test-owned child and clip region.
    unsafe {
        assert_eq!(
            SendMessageW(handle, WM_NCHITTEST, Some(WPARAM(0)), Some(LPARAM(0))).0,
            HTTRANSPARENT as isize
        );
        assert_eq!(
            SendMessageW(handle, WM_MOUSEACTIVATE, Some(WPARAM(0)), Some(LPARAM(0))).0,
            MA_NOACTIVATE as isize
        );
        let region = CreateRectRgn(0, 0, 0, 0);
        assert_eq!(GetWindowRgn(handle, region), SIMPLEREGION);
        let mut rect = Default::default();
        GetRgnBox(region, &mut rect);
        assert_eq!(
            [rect.left, rect.top, rect.right, rect.bottom],
            [8, 12, 40, 36]
        );
        let _ = DeleteObject(region.into());
    }
    host.close();
    assert!(child.hwnd().is_invalid());
    assert!(!child.drawable());
}

#[test]
#[ignore = "requires a Windows desktop and DX12 runtime; no GPU adapter or device is requested"]
fn surface_creation_supplies_display_for_shared_instances_without_a_gpu_device() {
    let mut host = Host::new();
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = wgpu::Backends::DX12;
    assert!(descriptor.display.is_none());
    let instance = wgpu::Instance::new(descriptor);
    let options = WindowsPreviewOptions::default();
    let first = WindowsGpuPreview::attach_host(host.0, instance.clone(), options)
        .expect("A Win32 surface must supply its display handle when the instance has none");
    let second = WindowsGpuPreview::attach_host(host.0, instance, options)
        .expect("Additional viewports must work with the same instance and Windows display");
    assert!(first.window.state.gpu.borrow().is_some());
    assert!(second.window.state.gpu.borrow().is_some());
    host.close();
    assert!(first.window.state.gpu.borrow().is_none());
    assert!(second.window.state.gpu.borrow().is_none());
}

#[test]
#[ignore = "requires a Windows desktop and DX12 GPU; exercises two swapchains, DPI changes and teardown"]
fn dx12_previews_share_a_queue_and_release_the_swapchain_before_parent_destruction() {
    let mut host = Host::new();
    let options = WindowsPreviewOptions {
        skip_busy_previews: true,
    };
    let instance = preview_instance(options);
    let mut first = WindowsGpuPreview::attach_host(host.0, instance.clone(), options).unwrap();
    let mut second = WindowsGpuPreview::attach_host(host.0, instance, options).unwrap();
    let adapter = first.request_adapter().unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let image = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Windows preview test source"),
        size: wgpu::Extent3d {
            width: 32,
            height: 32,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        image.as_image_copy(),
        &[40, 80, 160, 255].repeat(32 * 32),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(128),
            rows_per_image: Some(32),
        },
        image.size(),
    );
    let source = image.create_view(&Default::default());
    for preview in [&mut first, &mut second] {
        preview.initialize(&device, &adapter).unwrap();
        preview.prepare(&device, &source).unwrap();
    }
    // SAFETY: Show only this test's parent; do not activate another application's window.
    unsafe {
        let _ = ShowWindow(host.0, SW_SHOWNOACTIVATE);
    }
    let has_frame = |preview: &WindowsGpuPreview| {
        preview
            .window
            .state
            .gpu
            .borrow()
            .as_ref()
            .unwrap()
            .has_pending_frame()
    };
    second.set_vsync(false);
    for scale in [1.0, 1.5, 2.0, 1.0] {
        let mut a = bounds(scale);
        a.width = 100.0;
        first.set_bounds(a);
        a.x = 140.0;
        second.set_bounds(a);
        let mut encoder = device.create_command_encoder(&Default::default());
        first.encode(&device, &mut encoder).unwrap();
        assert!(has_frame(&first));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
        loop {
            second.encode(&device, &mut encoder).unwrap();
            if has_frame(&second) {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "preview never became ready"
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        queue.submit([encoder.finish()]);
        first.present(&queue);
        second.present(&queue);
    }
    // Simulate a graph error after acquiring the first swapchain image.
    let mut abandoned = device.create_command_encoder(&Default::default());
    first.encode(&device, &mut abandoned).unwrap();
    assert!(has_frame(&first));
    drop(abandoned);
    let mut recovered = device.create_command_encoder(&Default::default());
    first.encode(&device, &mut recovered).unwrap();
    assert!(has_frame(&first));
    queue.submit([recovered.finish()]);
    first.present(&queue);
    first.set_hidden(true);
    assert!(!first.window.drawable());
    first.set_hidden(false);
    assert!(first.window.drawable());
    host.close();
    assert!(first.window.state.gpu.borrow().is_none());
    assert!(second.window.state.gpu.borrow().is_none());
    let mut encoder = device.create_command_encoder(&Default::default());
    first.encode(&device, &mut encoder).unwrap();
    queue.submit([encoder.finish()]);
}
