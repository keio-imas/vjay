use cpal::traits::{DeviceTrait, HostTrait};
use vjay_application::{ports::PortResult, preferences::AudioSettings};

pub(super) fn configuration(
    settings: &AudioSettings,
) -> PortResult<(cpal::Device, cpal::StreamConfig, cpal::SampleFormat)> {
    let host = cpal::default_host();
    let device = if settings.device.is_empty() {
        host.default_output_device()
    } else {
        host.output_devices()
            .map_err(|e| e.to_string())?
            .find(|d| d.name().is_ok_and(|n| n == settings.device))
    }
    .ok_or("Audio output device is unavailable")?;
    let default = device.default_output_config().map_err(|e| e.to_string())?;
    let required = if settings.channel_pair == 0 && default.channels() == 1 {
        1
    } else {
        settings.channel_pair * 2 + 2
    };
    let supported = if settings.sample_rate == 0
        && (settings.channel_pair == 0 || default.channels() >= required)
    {
        default
    } else {
        let rate = if settings.sample_rate == 0 {
            default.sample_rate().0
        } else {
            settings.sample_rate
        };
        device
            .supported_output_configs()
            .map_err(|e| e.to_string())?
            .find(|c| {
                c.channels() >= required
                    && c.min_sample_rate().0 <= rate
                    && c.max_sample_rate().0 >= rate
            })
            .ok_or("The selected device does not support this sample rate / channel pair")?
            .with_sample_rate(cpal::SampleRate(rate))
    };
    let format = supported.sample_format();
    if !matches!(
        format,
        cpal::SampleFormat::F32
            | cpal::SampleFormat::F64
            | cpal::SampleFormat::I16
            | cpal::SampleFormat::U16
            | cpal::SampleFormat::I32
    ) {
        return Err(format!("Unsupported audio device format: {format}"));
    }
    if settings.buffer_frames != 0
        && let cpal::SupportedBufferSize::Range { min, max } = supported.buffer_size()
        && !(*min..=*max).contains(&settings.buffer_frames)
    {
        return Err(format!(
            "Buffer size must be {min}–{max} frames for this device"
        ));
    }
    let mut config: cpal::StreamConfig = supported.into();
    if settings.buffer_frames != 0 {
        config.buffer_size = cpal::BufferSize::Fixed(settings.buffer_frames);
    }
    Ok((device, config, format))
}
