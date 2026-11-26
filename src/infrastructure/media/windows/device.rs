use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE, HMODULE},
        Graphics::{
            Direct3D::{D3D_DRIVER_TYPE_UNKNOWN, D3D_FEATURE_LEVEL_11_0},
            Direct3D11::*,
            Direct3D12::*,
            Dxgi::{CreateDXGIFactory1, IDXGIAdapter, IDXGIFactory4},
        },
        Media::MediaFoundation::{MF_VERSION, MFSTARTUP_FULL, MFShutdown, MFStartup},
        System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize},
    },
    core::{Interface, PCWSTR, Result},
};

/// Constructed and dropped on the decode worker, after its COM objects.
pub(super) struct Apartment;
impl Apartment {
    pub fn new() -> Result<Self> {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
            if let Err(error) = MFStartup(MF_VERSION, MFSTARTUP_FULL) {
                CoUninitialize();
                return Err(error);
            }
        }
        Ok(Self)
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe {
            let _ = MFShutdown();
            CoUninitialize();
        }
    }
}

pub(super) struct SharedHandle(pub HANDLE);
impl Drop for SharedHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

pub(super) struct Device {
    pub d11: ID3D11Device,
    pub context: ID3D11DeviceContext4,
    pub d12: ID3D12Device,
}
impl Device {
    pub fn new(device: &wgpu::Device) -> Result<Self> {
        let native = unsafe { device.as_hal::<wgpu::hal::api::Dx12>() }
            .expect("DX12 factory checked the backend");
        let d12 = native.raw_device().clone();
        // Use the rendering GPU's exact LUID, including on hybrid systems.
        let factory: IDXGIFactory4 = unsafe { CreateDXGIFactory1()? };
        let adapter: IDXGIAdapter = unsafe { factory.EnumAdapterByLuid(d12.GetAdapterLuid())? };
        let (mut d11, mut context) = (None, None);
        unsafe {
            D3D11CreateDevice(
                &adapter,
                D3D_DRIVER_TYPE_UNKNOWN,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_VIDEO_SUPPORT | D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(&[D3D_FEATURE_LEVEL_11_0]),
                D3D11_SDK_VERSION,
                Some(&mut d11),
                None,
                Some(&mut context),
            )?;
        }
        let d11 = d11.expect("D3D11CreateDevice succeeded");
        let context: ID3D11DeviceContext4 = context.expect("immediate context").cast()?;
        let multithread: ID3D11Multithread = context.cast()?;
        let _ = unsafe { multithread.SetMultithreadProtected(true) };
        Ok(Self { d11, context, d12 })
    }
}

#[derive(Clone)]
pub(super) struct Fence {
    pub d11: ID3D11Fence,
    pub d12: ID3D12Fence,
}
impl Fence {
    pub fn new(device: &Device) -> Result<Self> {
        let d12: ID3D12Fence = unsafe { device.d12.CreateFence(0, D3D12_FENCE_FLAG_SHARED)? };
        let handle = SharedHandle(unsafe {
            device
                .d12
                .CreateSharedHandle(&d12, None, 0x10000000, PCWSTR::null())?
        });
        let mut d11 = None;
        unsafe {
            device
                .d11
                .cast::<ID3D11Device5>()?
                .OpenSharedFence(handle.0, &mut d11)?
        };
        Ok(Self {
            d11: d11.expect("shared fence opened"),
            d12,
        })
    }
}
