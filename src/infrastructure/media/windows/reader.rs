use super::device::Device;
use std::{
    os::windows::ffi::OsStrExt,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};
use vjay_application::ports::PortResult;
use windows::{
    Win32::{
        Graphics::Direct3D11::{D3D11_BIND_DECODER, D3D11_TEXTURE2D_DESC, ID3D11Texture2D},
        Media::MediaFoundation::*,
    },
    core::{GUID, Interface, PCWSTR},
};

const VIDEO: u32 = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
pub(super) const TICKS: f64 = 10_000_000.0;

#[derive(Clone)]
pub(super) struct Sample {
    pub sample: IMFSample,
    pub texture: ID3D11Texture2D,
    pub subresource: u32,
    pub time: u64,
    pub duration: u64,
}

pub(super) struct Reader {
    current: Option<Sample>,
    source: IMFSourceReader,
    _manager: IMFDXGIDeviceManager,
    pub media_type: IMFMediaType,
    pub duration: f64,
    pub fps: f64,
    pub seeks: u64,
    seen_frame: bool,
}

impl Reader {
    pub fn open(device: &Device, path: &Path) -> PortResult<Self> {
        // Restrict the adapter to local files. Canonical paths support Unicode and
        // extended-length Windows paths without interpreting them as URLs.
        let path = path
            .canonicalize()
            .map_err(|e| format!("Cannot open video: {e}"))?;
        if !path.is_file() {
            return Err("Choose a local video file".into());
        }
        let wide = path
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        let build = || -> windows::core::Result<Self> {
            unsafe {
                let (mut token, mut manager, mut attributes) = (0, None, None);
                MFCreateDXGIDeviceManager(&mut token, &mut manager)?;
                let manager = manager.expect("DXGI manager created");
                manager.ResetDevice(&device.d11, token)?;
                MFCreateAttributes(&mut attributes, 5)?;
                let attributes = attributes.expect("source attributes created");
                attributes.SetUnknown(&MF_SOURCE_READER_D3D_MANAGER, &manager)?;
                attributes.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1)?;
                attributes.SetUINT32(&MF_SOURCE_READER_DISABLE_DXVA, 0)?;
                attributes.SetUINT32(&MF_READWRITE_D3D_OPTIONAL, 0)?;
                // Software video processing would download the decoded GPU image.
                attributes.SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 0)?;
                let source = MFCreateSourceReaderFromURL(PCWSTR(wide.as_ptr()), &attributes)?;
                source.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)?;
                source.SetStreamSelection(VIDEO, true)?;
                let native = source.GetNativeMediaType(VIDEO, 0)?;
                let output = MFCreateMediaType()?;
                output.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
                // Preserve ten-bit media when its decoder advertises P010.
                let mut accepted = false;
                for subtype in [MFVideoFormat_P010, MFVideoFormat_NV12] {
                    output.SetGUID(&MF_MT_SUBTYPE, &subtype)?;
                    if source.SetCurrentMediaType(VIDEO, None, &output).is_ok() {
                        accepted = true;
                        break;
                    }
                }
                if !accepted {
                    return Err(windows::core::Error::new(
                        MF_E_INVALIDMEDIATYPE,
                        "No DXVA NV12/P010 decoder is available for this video",
                    ));
                }
                let media_type = source.GetCurrentMediaType(VIDEO)?;
                preserve_metadata(&native, &media_type)?;
                let rate = native
                    .GetUINT64(&MF_MT_FRAME_RATE)
                    .unwrap_or((30u64 << 32) | 1);
                let fps = (rate >> 32) as f64 / (rate as u32).max(1) as f64;
                let fps = if fps > 0.0 {
                    fps.clamp(1.0, 240.0)
                } else {
                    30.0
                };
                let duration = source.GetPresentationAttribute(
                    MF_SOURCE_READER_MEDIASOURCE.0 as u32,
                    &MF_PD_DURATION,
                )?;
                let duration = u64::try_from(&duration)? as f64 / TICKS;
                Ok(Self {
                    source,
                    _manager: manager,
                    current: None,
                    media_type,
                    duration,
                    fps,
                    seeks: 0,
                    seen_frame: false,
                })
            }
        };
        build().map_err(|e| format!(
            "Windows GPU video could not open this codec: {e}. Check GPU driver, codec support and the Windows Media Feature Pack/HEVC extension. No CPU pixel fallback was used."))
    }

    pub fn at(&mut self, target: u64, wanted: &AtomicU64) -> PortResult<Option<Sample>> {
        if (self.current.is_none() && target > 0)
            || self.current.as_ref().is_some_and(|s| {
                target < s.time || target.saturating_sub(s.time) > (TICKS * 0.5) as u64
            })
        {
            unsafe {
                self.source
                    .SetCurrentPosition(&GUID::zeroed(), &(target as i64).into())
            }
            .map_err(|e| format!("GPU video seek failed: {e}"))?;
            self.current = None;
            self.seeks += 1;
        }
        // Decode from the previous keyframe, retaining PTS rather than assigning
        // a normalized seek bar to frames. Ordinary forward playback never seeks.
        for _ in 0..16384 {
            if let Some(sample) = &self.current
                && target < sample.time.saturating_add(sample.duration)
            {
                return Ok(Some(sample.clone()));
            }
            if wanted.load(Ordering::Acquire).abs_diff(target) > (TICKS * 0.5) as u64 {
                return Ok(None);
            }
            let (mut flags, mut time, mut sample) = (0, 0, None);
            unsafe {
                self.source.ReadSample(
                    VIDEO,
                    0,
                    None,
                    Some(&mut flags),
                    Some(&mut time),
                    Some(&mut sample),
                )
            }
            .map_err(|e| format!("GPU video decode failed: {e}"))?;
            if flags & MF_SOURCE_READERF_ERROR.0 as u32 != 0 {
                return Err("Media Foundation reported a video decoder error".into());
            }
            if flags & MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED.0 as u32 != 0 {
                let changed =
                    unsafe { self.source.GetCurrentMediaType(VIDEO) }.map_err(|e| e.to_string())?;
                let same = unsafe {
                    changed.GetUINT64(&MF_MT_FRAME_SIZE)
                        == self.media_type.GetUINT64(&MF_MT_FRAME_SIZE)
                        && changed.GetGUID(&MF_MT_SUBTYPE)
                            == self.media_type.GetGUID(&MF_MT_SUBTYPE)
                };
                if !same && self.seen_frame {
                    return Err("Video changed format during playback; reload the source".into());
                }
                preserve_metadata(&self.media_type, &changed).map_err(|e| e.to_string())?;
                self.media_type = changed;
            }
            if let Some(sample) = sample {
                let decoded = (|| -> windows::core::Result<Sample> {
                    unsafe {
                        let buffer: IMFDXGIBuffer = sample.GetBufferByIndex(0)?.cast()?;
                        let mut texture = std::ptr::null_mut();
                        buffer.GetResource(&ID3D11Texture2D::IID, &mut texture)?;
                        let texture = ID3D11Texture2D::from_raw(texture);
                        let mut desc = D3D11_TEXTURE2D_DESC::default();
                        texture.GetDesc(&mut desc);
                        if desc.BindFlags & D3D11_BIND_DECODER.0 as u32 == 0 || desc.CPUAccessFlags != 0 {
                            return Err(windows::core::Error::new(MF_E_UNSUPPORTED_D3D_TYPE,
                                "A hardware decoder target is required; software-upload surfaces are rejected"));
                        }
                        let duration = sample
                            .GetSampleDuration()
                            .ok()
                            .filter(|d| *d > 0)
                            .map(|d| d as u64)
                            .unwrap_or((TICKS / self.fps) as u64);
                        Ok::<_, windows::core::Error>(Sample {
                            sample,
                            texture,
                            subresource: buffer.GetSubresourceIndex()?,
                            time: time.max(0) as u64,
                            duration,
                        })
                    }
                })()
                .map_err(|e| {
                    format!(
                        "Decoder did not return a D3D11 GPU surface: {e}. CPU download is disabled."
                    )
                })?;
                self.current = Some(decoded);
                self.seen_frame = true;
            }
            if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                return self
                    .current
                    .clone()
                    .map(Some)
                    .ok_or("Video has no decoded frames".into());
            }
        }
        Err("Video seek exceeded the decode limit".into())
    }
}

fn preserve_metadata(source: &IMFMediaType, target: &IMFMediaType) -> windows::core::Result<()> {
    for key in [
        MF_MT_VIDEO_ROTATION,
        MF_MT_VIDEO_NOMINAL_RANGE,
        MF_MT_YUV_MATRIX,
        MF_MT_TRANSFER_FUNCTION,
        MF_MT_INTERLACE_MODE,
        MF_MT_PIXEL_ASPECT_RATIO,
        MF_MT_MINIMUM_DISPLAY_APERTURE,
        MF_MT_GEOMETRIC_APERTURE,
    ] {
        unsafe {
            let mut value = windows::Win32::System::Com::StructuredStorage::PROPVARIANT::default();
            if target.GetItemType(&key).is_err() && source.GetItem(&key, Some(&mut value)).is_ok() {
                target.SetItem(&key, &value)?;
            }
        }
    }
    Ok(())
}
