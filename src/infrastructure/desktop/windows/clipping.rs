use crate::gpu::presentation::{PreviewBounds, PreviewRegion};
use ::windows::Win32::{Foundation::HWND, Graphics::Gdi::*};
use vjay_application::ports::PortResult;

/// Convert logical GPUI coordinates to the child HWND's physical client coordinates.
pub(super) fn physical_bounds(bounds: PreviewBounds) -> PortResult<[i32; 4]> {
    let values = [
        bounds.x,
        bounds.y,
        bounds.width,
        bounds.height,
        bounds.scale,
    ];
    if values.iter().any(|v| !v.is_finite())
        || bounds.scale <= 0.0
        || bounds.width < 0.0
        || bounds.height < 0.0
    {
        return Err("Invalid Windows viewport bounds".into());
    }
    let values = [
        bounds.x * bounds.scale,
        bounds.y * bounds.scale,
        bounds.width * bounds.scale,
        bounds.height * bounds.scale,
    ];
    if values
        .iter()
        .any(|v| !v.is_finite() || v.abs() > (i32::MAX / 2) as f32)
    {
        return Err("Windows viewport bounds exceed the native coordinate range".into());
    }
    Ok(values.map(|v| v.round() as i32))
}

struct Region(Option<HRGN>);
impl Region {
    fn new(rect: [i32; 4]) -> PortResult<Self> {
        // SAFETY: Coordinates are checked before reaching GDI.
        let handle = unsafe { CreateRectRgn(rect[0], rect[1], rect[2], rect[3]) };
        if handle.is_invalid() {
            return Err("Cannot allocate Windows preview clip region".into());
        }
        Ok(Self(Some(handle)))
    }
}
impl Drop for Region {
    fn drop(&mut self) {
        if let Some(handle) = self.0.take() {
            // SAFETY: Only delete a region still owned by this scope.
            unsafe {
                let _ = DeleteObject(handle.into());
            }
        }
    }
}

pub(super) fn set_mask(hwnd: HWND, regions: &[PreviewRegion], scale: f32) -> PortResult<()> {
    let mut mask = Region::new([0; 4])?;
    for region in regions {
        if [region.x, region.y, region.width, region.height, scale]
            .iter()
            .any(|v| !v.is_finite())
            || region.width < 0.0
            || region.height < 0.0
            || scale <= 0.0
        {
            return Err("Invalid Windows preview clip region".into());
        }
        let piece = Region::new([
            (region.x * scale).floor() as i32,
            (region.y * scale).floor() as i32,
            ((region.x + region.width) * scale).ceil() as i32,
            ((region.y + region.height) * scale).ceil() as i32,
        ])?;
        // SAFETY: Both regions are owned locally; union only updates mask.
        if unsafe { CombineRgn(mask.0, mask.0, piece.0, RGN_OR) } == RGN_ERROR {
            return Err("Cannot combine Windows preview clip regions".into());
        }
    }
    // SAFETY: On success the HWND owns mask. On failure our guard deletes it.
    if unsafe { SetWindowRgn(hwnd, mask.0, true) } == 0 {
        return Err("Cannot apply Windows preview clip region".into());
    }
    mask.0.take();
    Ok(())
}
