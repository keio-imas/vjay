use image::ImageDecoder;
use std::path::Path;
use vjay_application::{
    media::ImageLoader,
    ports::{Extent, PortResult},
};
pub struct RasterImageLoader;
impl ImageLoader for RasterImageLoader {
    fn load(&self, path: &Path, extent: Extent) -> PortResult<Vec<u8>> {
        Extent::new(extent.width, extent.height)?;
        let mut reader = image::ImageReader::open(path)
            .map_err(|e| format!("Cannot open image: {e}"))?
            .with_guessed_format()
            .map_err(|e| e.to_string())?;
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(256 * 1024 * 1024);
        reader.limits(limits);
        let mut decoder = reader
            .into_decoder()
            .map_err(|e| format!("Cannot decode image: {e}"))?;
        let orientation = decoder.orientation().map_err(|e| e.to_string())?;
        let mut image = image::DynamicImage::from_decoder(decoder)
            .map_err(|e| format!("Cannot decode image: {e}"))?;
        image.apply_orientation(orientation);
        let source = image.to_rgba8();
        let ratio = (extent.width as f64 / source.width() as f64)
            .min(extent.height as f64 / source.height() as f64);
        let width = (source.width() as f64 * ratio).round().max(1.0) as u32;
        let height = (source.height() as f64 * ratio).round().max(1.0) as u32;
        let scaled = image::imageops::resize(
            &source,
            width,
            height,
            image::imageops::FilterType::Triangle,
        );
        let mut output = image::RgbaImage::new(extent.width, extent.height);
        image::imageops::overlay(
            &mut output,
            &scaled,
            ((extent.width - width) / 2) as i64,
            ((extent.height - height) / 2) as i64,
        );
        Ok(output.into_raw())
    }
}
