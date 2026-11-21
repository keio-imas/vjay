use super::{clipping, surface::SurfacePresenter};
use crate::gpu::presentation::{PreviewBounds, PreviewRegion};
use ::windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        System::{LibraryLoader::GetModuleHandleW, Threading::GetCurrentThreadId},
        UI::WindowsAndMessaging::*,
    },
    core::w,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::OnceLock,
};
use vjay_application::ports::PortResult;

const CLASS: ::windows::core::PCWSTR = w!("VJAY.GpuViewport");
pub(super) struct WindowState {
    hwnd: Cell<Option<HWND>>,
    pub gpu: RefCell<Option<SurfacePresenter>>,
}
pub(super) struct ChildWindow {
    pub state: Rc<WindowState>,
    host: HWND,
    hidden: bool,
    bounds: Option<PreviewBounds>,
    regions: Option<Vec<PreviewRegion>>,
    mask_scale: f32,
}

pub(super) fn host_handle(window: &gpui::Window) -> PortResult<HWND> {
    let handle = HasWindowHandle::window_handle(window).map_err(|e| e.to_string())?;
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return Err("Expected a Win32 window".into());
    };
    Ok(HWND(handle.hwnd.get() as *mut _))
}

impl ChildWindow {
    pub fn new(host: HWND) -> PortResult<Self> {
        // SAFETY: Only inspect the borrowed parent. Every adapter is confined to
        // the creating GUI thread by Rc; the parent may close before this owner.
        unsafe {
            if !IsWindow(Some(host)).as_bool()
                || GetWindowThreadProcessId(host, None) != GetCurrentThreadId()
            {
                return Err("Windows preview requires a live window on its GUI thread".into());
            }
        }
        static REGISTERED: OnceLock<PortResult<()>> = OnceLock::new();
        REGISTERED
            .get_or_init(|| {
                // SAFETY: The class has a process-long name and a static window procedure.
                unsafe {
                    let class = WNDCLASSW {
                        lpfnWndProc: Some(window_proc),
                        hInstance: GetModuleHandleW(None).map_err(|e| e.to_string())?.into(),
                        lpszClassName: CLASS,
                        ..Default::default()
                    };
                    if RegisterClassW(&class) == 0 {
                        return Err(std::io::Error::last_os_error().to_string());
                    }
                }
                Ok(())
            })
            .clone()?;
        let state = Rc::new(WindowState {
            hwnd: Cell::new(None),
            gpu: RefCell::new(None),
        });
        // SAFETY: WM_NCCREATE copies the Rc before this stack reference expires.
        // The window holds its own strong reference until WM_NCDESTROY.
        unsafe {
            CreateWindowExW(
                WS_EX_NOACTIVATE | WS_EX_TRANSPARENT,
                CLASS,
                w!(""),
                WS_CHILD | WS_CLIPSIBLINGS,
                0,
                0,
                1,
                1,
                Some(host),
                None,
                Some(GetModuleHandleW(None).map_err(|e| e.to_string())?.into()),
                Some((&state as *const Rc<WindowState>).cast()),
            )
            .map_err(|e| format!("Cannot create Windows GPU viewport: {e}"))?;
        }
        Ok(Self {
            state,
            host,
            hidden: false,
            bounds: None,
            regions: None,
            mask_scale: 0.0,
        })
    }

    pub fn hwnd(&self) -> HWND {
        self.state.hwnd.get().unwrap_or_default()
    }

    pub fn set_hidden(&mut self, hidden: bool) {
        if self.hidden == hidden {
            return;
        }
        self.hidden = hidden;
        self.update_visibility();
    }

    fn update_visibility(&self) {
        if let Some(hwnd) = self.state.hwnd.get() {
            let visible =
                !self.hidden && self.bounds.is_some_and(|b| b.width > 0.0 && b.height > 0.0);
            // SAFETY: The destruction callback invalidates this handle synchronously.
            unsafe {
                let _ = ShowWindow(hwnd, if visible { SW_SHOWNA } else { SW_HIDE });
            }
        }
    }

    pub fn set_bounds(&mut self, bounds: PreviewBounds) -> PortResult<[u32; 2]> {
        let rect = clipping::physical_bounds(bounds)?;
        if self.bounds != Some(bounds) {
            if let Some(hwnd) = self.state.hwnd.get() {
                // SAFETY: Same-thread child, no activation or focus transfer.
                unsafe {
                    SetWindowPos(
                        hwnd,
                        None,
                        rect[0],
                        rect[1],
                        rect[2],
                        rect[3],
                        SWP_NOACTIVATE | SWP_NOZORDER,
                    )
                    .map_err(|e| e.to_string())?;
                }
            }
            self.bounds = Some(bounds);
            self.update_visibility();
            if let (Some(hwnd), Some(regions)) = (self.state.hwnd.get(), &self.regions) {
                clipping::set_mask(hwnd, regions, bounds.scale)?;
                self.mask_scale = bounds.scale;
            }
        }
        Ok([rect[2] as u32, rect[3] as u32])
    }

    pub fn set_regions(&mut self, regions: &[PreviewRegion]) -> PortResult<()> {
        let Some(bounds) = self.bounds else {
            return Ok(());
        };
        if self.regions.as_deref() == Some(regions) && self.mask_scale == bounds.scale {
            return Ok(());
        }
        if self.state.hwnd.get().is_some() {
            clipping::set_mask(self.hwnd(), regions, bounds.scale)?;
        }
        self.regions = Some(regions.to_vec());
        self.mask_scale = bounds.scale;
        Ok(())
    }

    pub fn drawable(&self) -> bool {
        if self.hidden
            || self.state.hwnd.get().is_none()
            || self
                .bounds
                .is_none_or(|b| b.width <= 0.0 || b.height <= 0.0)
        {
            return false;
        }
        // SAFETY: Read-only queries on the parent; no GPU state is borrowed.
        unsafe { IsWindowVisible(self.host).as_bool() && !IsIconic(self.host).as_bool() }
    }
}

impl Drop for ChildWindow {
    fn drop(&mut self) {
        if let Some(hwnd) = self.state.hwnd.get() {
            // SAFETY: Only destroy our own child, on its creating thread.
            // WM_DESTROY releases the swapchain before the HWND becomes invalid.
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
        }
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: Win32 supplies valid message data. Only this class writes its user
    // data, which holds exactly one Rc reference between NCCREATE and NCDESTROY.
    unsafe {
        if message == WM_NCCREATE {
            let create = &*(lparam.0 as *const CREATESTRUCTW);
            let state = &*(create.lpCreateParams as *const Rc<WindowState>);
            state.hwnd.set(Some(hwnd));
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, Rc::into_raw(state.clone()) as isize);
        }
        let pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const WindowState;
        match message {
            WM_NCHITTEST => return LRESULT(HTTRANSPARENT as isize),
            WM_MOUSEACTIVATE => return LRESULT(MA_NOACTIVATE as isize),
            WM_ERASEBKGND => return LRESULT(1),
            WM_DESTROY if !pointer.is_null() => {
                (*pointer).gpu.borrow_mut().take();
            }
            WM_NCDESTROY if !pointer.is_null() => {
                (*pointer).hwnd.set(None);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                drop(Rc::from_raw(pointer));
            }
            _ => {}
        }
        DefWindowProcW(hwnd, message, wparam, lparam)
    }
}
