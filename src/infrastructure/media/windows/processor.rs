use super::{
    device::Device,
    reader::Sample,
    transfer::{Recycled, Slot},
};
use std::{collections::VecDeque, mem::ManuallyDrop, sync::Arc};
use vjay_application::ports::Extent;
use windows::{
    Win32::{
        Graphics::{Direct3D11::*, Dxgi::Common::*},
        Media::MediaFoundation::*,
    },
    core::{Interface, Result},
};

struct Input {
    texture: ID3D11Texture2D,
    subresource: u32,
    view: ID3D11VideoProcessorInputView,
}
impl Drop for Processor {
    fn drop(&mut self) {
        // Only this worker drains native work; dropping a node never joins it on
        // the UI thread. Device loss must not make shutdown wait indefinitely.
        let _ = self.finish_reads();
    }
}
pub(super) struct Processor {
    device: ID3D11VideoDevice,
    context: ID3D11VideoContext1,
    enumerator: ID3D11VideoProcessorEnumerator,
    processor: ID3D11VideoProcessor,
    inputs: Vec<Input>,
    outputs: Vec<(Arc<Slot>, ID3D11VideoProcessorOutputView)>,
    // Retain decoder pool samples until the video processor finished reading.
    pending: VecDeque<(IMFSample, Arc<Slot>, u64)>,
}
impl Processor {
    pub fn finish_reads(&mut self) -> Result<()> {
        let start = std::time::Instant::now();
        while !self.pending.is_empty() {
            self.reap();
            if self.pending.is_empty() {
                break;
            }
            if start.elapsed() > std::time::Duration::from_secs(5) {
                return Err(windows::core::Error::new(
                    windows::Win32::Foundation::E_FAIL,
                    "GPU video processing timed out",
                ));
            }
            std::thread::sleep(std::time::Duration::from_micros(200));
        }
        Ok(())
    }

    pub fn new(
        device: &Device,
        media: &IMFMediaType,
        extent: Extent,
        slots: &[Arc<Slot>],
    ) -> Result<Self> {
        unsafe {
            let video: ID3D11VideoDevice = device.d11.cast()?;
            let context: ID3D11VideoContext1 = device.context.cast()?;
            let size = media.GetUINT64(&MF_MT_FRAME_SIZE)?;
            let (width, height) = ((size >> 32) as u32, size as u32);
            let enumerator =
                video.CreateVideoProcessorEnumerator(&D3D11_VIDEO_PROCESSOR_CONTENT_DESC {
                    InputFrameFormat: D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE,
                    InputWidth: width,
                    InputHeight: height,
                    OutputWidth: extent.width,
                    OutputHeight: extent.height,
                    InputFrameRate: DXGI_RATIONAL {
                        Numerator: 30,
                        Denominator: 1,
                    },
                    OutputFrameRate: DXGI_RATIONAL {
                        Numerator: 30,
                        Denominator: 1,
                    },
                    Usage: D3D11_VIDEO_USAGE_PLAYBACK_NORMAL,
                })?;
            let processor = video.CreateVideoProcessor(&enumerator, 0)?;
            super::format::configure(&context, &processor, media, extent)?;
            let mut outputs = Vec::with_capacity(slots.len());
            for slot in slots {
                let mut output = None;
                video.CreateVideoProcessorOutputView(
                    &slot.texture,
                    &enumerator,
                    &D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC {
                        ViewDimension: D3D11_VPOV_DIMENSION_TEXTURE2D,
                        ..Default::default()
                    },
                    Some(&mut output),
                )?;
                outputs.push((slot.clone(), output.expect("processor output view")));
            }
            Ok(Self {
                device: video,
                context,
                enumerator,
                processor,
                inputs: Vec::with_capacity(32),
                outputs,
                pending: VecDeque::with_capacity(8),
            })
        }
    }

    pub fn reap(&mut self) {
        while self.pending.front().is_some_and(|(_, slot, value)| unsafe {
            slot.producer.d11.GetCompletedValue() >= *value
        }) {
            self.pending.pop_front();
        }
    }

    pub fn blit(
        &mut self,
        device: &Device,
        sample: &Sample,
        recycled: &Recycled,
        value: u64,
    ) -> Result<()> {
        self.reap();
        unsafe {
            device
                .context
                .Wait(&recycled.slot.consumer.d11, recycled.consumed)?;
            let index = self.inputs.iter().position(|input| {
                input.texture == sample.texture && input.subresource == sample.subresource
            });
            let index = match index {
                Some(index) => index,
                None => {
                    // A decoder normally uses a fixed texture-array pool. Bound
                    // descriptor retention when it replaces that pool after seeks.
                    if self.inputs.len() == 64 {
                        self.inputs.clear();
                    }
                    let mut desc = D3D11_TEXTURE2D_DESC::default();
                    sample.texture.GetDesc(&mut desc);
                    let mut view = None;
                    self.device.CreateVideoProcessorInputView(
                        &sample.texture,
                        &self.enumerator,
                        &D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC {
                            ViewDimension: D3D11_VPIV_DIMENSION_TEXTURE2D,
                            Anonymous: D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC_0 {
                                Texture2D: D3D11_TEX2D_VPIV {
                                    MipSlice: sample.subresource % desc.MipLevels.max(1),
                                    ArraySlice: sample.subresource / desc.MipLevels.max(1),
                                },
                            },
                            ..Default::default()
                        },
                        Some(&mut view),
                    )?;
                    self.inputs.push(Input {
                        texture: sample.texture.clone(),
                        subresource: sample.subresource,
                        view: view.expect("processor input view"),
                    });
                    self.inputs.len() - 1
                }
            };
            let output = &self
                .outputs
                .iter()
                .find(|(slot, _)| Arc::ptr_eq(slot, &recycled.slot))
                .expect("persistent output slot")
                .1;
            let mut stream = D3D11_VIDEO_PROCESSOR_STREAM {
                Enable: true.into(),
                pInputSurface: ManuallyDrop::new(Some(self.inputs[index].view.clone())),
                ..Default::default()
            };
            let result = self.context.VideoProcessorBlt(
                &self.processor,
                output,
                0,
                std::slice::from_ref(&stream),
            );
            ManuallyDrop::drop(&mut stream.pInputSurface);
            result?;
            device.context.Signal(&recycled.slot.producer.d11, value)?;
            device.context.Flush();
            self.pending
                .push_back((sample.sample.clone(), recycled.slot.clone(), value));
            Ok(())
        }
    }
}
