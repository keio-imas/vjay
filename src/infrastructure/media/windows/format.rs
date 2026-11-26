use vjay_application::ports::Extent;
use windows::{
    Win32::{
        Foundation::RECT,
        Graphics::{Direct3D11::*, Dxgi::Common::*},
        Media::MediaFoundation::*,
    },
    core::{Error, Result},
};

pub(super) fn configure(
    context: &ID3D11VideoContext1,
    processor: &ID3D11VideoProcessor,
    media: &IMFMediaType,
    extent: Extent,
) -> Result<()> {
    unsafe {
        let interlace = media.GetUINT32(&MF_MT_INTERLACE_MODE).unwrap_or(2);
        if interlace != 0 && interlace != 2 {
            return Err(Error::new(
                MF_E_INVALIDMEDIATYPE,
                "Windows GPU video currently requires progressive frames",
            ));
        }
        let transfer = media.GetUINT32(&MF_MT_TRANSFER_FUNCTION).unwrap_or(0);
        if [
            MFVideoTransFunc_2084.0 as u32,
            MFVideoTransFunc_HLG.0 as u32,
        ]
        .contains(&transfer)
        {
            return Err(Error::new(
                MF_E_INVALIDMEDIATYPE,
                "HDR video needs SDR conversion; Windows HDR tone mapping is not implemented",
            ));
        }
        let size = media.GetUINT64(&MF_MT_FRAME_SIZE)?;
        let (width, height) = ((size >> 32) as u32, size as u32);
        let source = visible_rect(media, width, height);
        let par = media
            .GetUINT64(&MF_MT_PIXEL_ASPECT_RATIO)
            .unwrap_or((1u64 << 32) | 1);
        let aspect = (par >> 32).max(1) as f64 / (par as u32).max(1) as f64;
        let rotation = media.GetUINT32(&MF_MT_VIDEO_ROTATION).unwrap_or(0);
        let mut display = (
            (source.right - source.left) as f64 * aspect,
            (source.bottom - source.top) as f64,
        );
        if rotation == 90 || rotation == 270 {
            display = (display.1, display.0);
        }
        let scale = (extent.width as f64 / display.0).min(extent.height as f64 / display.1);
        let fitted = (
            (display.0 * scale).round() as u32,
            (display.1 * scale).round() as u32,
        );
        let left = extent.width.saturating_sub(fitted.0) / 2;
        let top = extent.height.saturating_sub(fitted.1) / 2;
        let target = RECT {
            left: left as i32,
            top: top as i32,
            right: (left + fitted.0) as i32,
            bottom: (top + fitted.1) as i32,
        };
        let viewport = RECT {
            left: 0,
            top: 0,
            right: extent.width as i32,
            bottom: extent.height as i32,
        };
        context.VideoProcessorSetStreamSourceRect(processor, 0, true, Some(&source));
        context.VideoProcessorSetStreamDestRect(processor, 0, true, Some(&target));
        context.VideoProcessorSetOutputTargetRect(processor, true, Some(&viewport));
        context.VideoProcessorSetOutputBackgroundColor(
            processor,
            false,
            &D3D11_VIDEO_COLOR {
                Anonymous: D3D11_VIDEO_COLOR_0 {
                    RGBA: D3D11_VIDEO_COLOR_RGBA {
                        R: 0.0,
                        G: 0.0,
                        B: 0.0,
                        A: 1.0,
                    },
                },
            },
        );
        context.VideoProcessorSetStreamRotation(
            processor,
            0,
            rotation != 0,
            D3D11_VIDEO_PROCESSOR_ROTATION((rotation / 90) as i32),
        );
        context.VideoProcessorSetStreamFrameFormat(
            processor,
            0,
            D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE,
        );
        context.VideoProcessorSetStreamAutoProcessingMode(processor, 0, false);
        context.VideoProcessorSetOutputAlphaFillMode(
            processor,
            D3D11_VIDEO_PROCESSOR_ALPHA_FILL_MODE_OPAQUE,
            0,
        );
        let full = media.GetUINT32(&MF_MT_VIDEO_NOMINAL_RANGE).ok() == Some(1);
        let bt601 = media
            .GetUINT32(&MF_MT_YUV_MATRIX)
            .unwrap_or(if height <= 576 { 2 } else { 1 })
            == 2;
        let color = match (full, bt601) {
            (true, true) => DXGI_COLOR_SPACE_YCBCR_FULL_G22_LEFT_P601,
            (true, false) => DXGI_COLOR_SPACE_YCBCR_FULL_G22_LEFT_P709,
            (false, true) => DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_LEFT_P601,
            (false, false) => DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_LEFT_P709,
        };
        context.VideoProcessorSetStreamColorSpace1(processor, 0, color);
        context
            .VideoProcessorSetOutputColorSpace1(processor, DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P709);
        Ok(())
    }
}

fn visible_rect(media: &IMFMediaType, width: u32, height: u32) -> RECT {
    let fallback = RECT {
        left: 0,
        top: 0,
        right: width as i32,
        bottom: height as i32,
    };
    for key in [MF_MT_MINIMUM_DISPLAY_APERTURE, MF_MT_GEOMETRIC_APERTURE] {
        let mut area = MFVideoArea::default();
        let size = std::mem::size_of::<MFVideoArea>();
        // MFVideoArea is a C POD containing integer offsets and a SIZE. Require
        // the exact blob length before letting Media Foundation fill it.
        let result = unsafe {
            if media.GetBlobSize(&key).ok() != Some(size as u32) {
                continue;
            }
            let bytes =
                std::slice::from_raw_parts_mut((&mut area as *mut MFVideoArea).cast(), size);
            media.GetBlob(&key, bytes, None)
        };
        if result.is_ok() && area.Area.cx > 0 && area.Area.cy > 0 {
            let left = (area.OffsetX.value as i32).max(0);
            let top = (area.OffsetY.value as i32).max(0);
            let right = left.saturating_add(area.Area.cx).min(width as i32);
            let bottom = top.saturating_add(area.Area.cy).min(height as i32);
            if right > left && bottom > top {
                return RECT {
                    left,
                    top,
                    right,
                    bottom,
                };
            }
        }
    }
    fallback
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn display_aperture_excludes_decoder_padding() {
        let _apartment = crate::media::windows::device::Apartment::new().unwrap();
        let media = unsafe { MFCreateMediaType() }.unwrap();
        let area = MFVideoArea {
            Area: windows::Win32::Foundation::SIZE { cx: 1920, cy: 1080 },
            ..Default::default()
        };
        unsafe {
            let bytes = std::slice::from_raw_parts(
                (&area as *const MFVideoArea).cast(),
                std::mem::size_of_val(&area),
            );
            media
                .SetBlob(&MF_MT_MINIMUM_DISPLAY_APERTURE, bytes)
                .unwrap();
        }
        let rect = visible_rect(&media, 1920, 1088);
        assert_eq!(
            (rect.left, rect.top, rect.right, rect.bottom),
            (0, 0, 1920, 1080)
        );
        unsafe { media.DeleteItem(&MF_MT_MINIMUM_DISPLAY_APERTURE) }.unwrap();
        assert_eq!(visible_rect(&media, 1920, 1088).bottom, 1088);
    }
}
