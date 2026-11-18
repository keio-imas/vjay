use vjay_application::ports::PortResult;
/// Native PAL capability gate. Do not ask VideoToolbox to decode a codec that
/// this machine can only decode in software (notably ProRes on older Macs).
pub(super) fn require_hardware(codec: &str) -> PortResult<()> {
    #[cfg(target_os = "macos")]
    {
        #[link(name = "VideoToolbox", kind = "framework")]
        unsafe extern "C" {
            fn VTIsHardwareDecodeSupported(codec_type: u32) -> u8;
        }
        let code = match codec {
            "h264" => *b"avc1",
            "hevc" => *b"hvc1",
            "prores" => *b"apch",
            _ => return Err("Unsupported VideoToolbox codec".into()),
        };
        // The framework function is thread safe and takes a FourCC by value.
        if unsafe { VTIsHardwareDecodeSupported(u32::from_be_bytes(code)) } != 0 {
            return Ok(());
        }
        Err(format!("This Mac has no hardware {codec} decoder"))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = codec;
        Err("VideoToolbox is available only on macOS".into())
    }
}
