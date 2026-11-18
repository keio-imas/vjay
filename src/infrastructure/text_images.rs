use cosmic_text::{
    Align, Attrs, Buffer, CacheKeyFlags, Color, Family, FontSystem, Metrics, Shaping, Style,
    SwashCache, Weight,
};
use std::sync::Mutex;
use vjay_application::{
    media::PreparedImageFactory,
    ports::{Extent, PortResult},
};
use vjay_domain::execution::PreparedImage;

pub trait ImageProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn load(&self, source: &PreparedImage, extent: Extent) -> PortResult<Vec<u8>>;
}
pub struct PreparedImageRegistry(pub Vec<Box<dyn ImageProvider>>);
impl PreparedImageFactory for PreparedImageRegistry {
    fn load(&self, source: &PreparedImage, extent: Extent) -> PortResult<Vec<u8>> {
        self.0
            .iter()
            .find(|p| p.id() == source.provider)
            .ok_or("Image provider is unavailable")?
            .load(source, extent)
    }
}
#[derive(Default)]
pub struct TextImageProvider(Mutex<Option<FontSystem>>);
impl ImageProvider for TextImageProvider {
    fn id(&self) -> &'static str {
        "text"
    }
    fn load(&self, source: &PreparedImage, extent: Extent) -> PortResult<Vec<u8>> {
        Extent::new(extent.width, extent.height)?;
        let props = &source.properties;
        let text = props.get("content.text").map_or("", String::as_str);
        let font = props
            .get("font.family")
            .map_or("sans-serif", String::as_str);
        let size = props
            .get("font.size")
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(64.0);
        if !size.is_finite() || !(4.0..=512.0).contains(&size) || text.len() > 16384 {
            return Err("Invalid text layout".into());
        }
        let size = size * extent.height as f32 / 540.0;
        let mut lock = self
            .0
            .lock()
            .map_err(|_| "Text font system is unavailable")?;
        let system = lock.get_or_insert_with(FontSystem::new);
        let mut buffer = Buffer::new(system, Metrics::new(size, size * 1.25));
        buffer.set_size(
            system,
            Some(extent.width as f32),
            Some(extent.height as f32),
        );
        let family = match font {
            "sans-serif" => Family::SansSerif,
            "serif" => Family::Serif,
            "monospace" => Family::Monospace,
            _ => Family::Name(font),
        };
        let weight = props
            .get("font.weight.index")
            .and_then(|v| v.parse::<f32>().ok())
            .map(|v| Weight(((v.round() as u16) + 1) * 100))
            .unwrap_or(Weight::NORMAL);
        let style = match props
            .get("font.style.index")
            .and_then(|v| v.parse::<f32>().ok())
            .map(|v| v.round() as i32)
        {
            Some(1) => Style::Italic,
            Some(2) => Style::Oblique,
            _ => Style::Normal,
        };
        let attrs = Attrs::new().family(family).weight(weight).style(style);
        buffer.set_text(system, text, &attrs, Shaping::Advanced);
        let align = if let Some(index) = props
            .get("alignment.index")
            .and_then(|v| v.parse::<f32>().ok())
        {
            match index.round() as i32 {
                0 => Align::Left,
                2 => Align::Right,
                _ => Align::Center,
            }
        } else {
            match props.get("alignment").map(String::as_str) {
                Some("left") => Align::Left,
                Some("right") => Align::Right,
                _ => Align::Center,
            }
        };
        for line in &mut buffer.lines {
            line.set_align(Some(align));
        }
        buffer.shape_until_scroll(system, false);
        resolve_missing_style(&mut buffer, system, &attrs);
        let height = buffer
            .layout_runs()
            .map(|r| r.line_y + r.line_height)
            .fold(0.0_f32, f32::max);
        let offset = ((extent.height as f32 - height) * 0.5).max(0.0) as i32;
        let mut rgba = vec![0_u8; (extent.width * extent.height * 4) as usize];
        let mut cache = SwashCache::new();
        buffer.draw(
            system,
            &mut cache,
            Color::rgb(255, 255, 255),
            |x, y, _, _, color| {
                let y = y + offset;
                if x < 0 || y < 0 || x >= extent.width as i32 || y >= extent.height as i32 {
                    return;
                }
                let index = (y as usize * extent.width as usize + x as usize) * 4;
                let src = color.as_rgba();
                let dst = &mut rgba[index..index + 4];
                let alpha = src[3] as f32 / 255.0;
                let old = dst[3] as f32 / 255.0;
                let output = alpha + old * (1.0 - alpha);
                if output > 0.0 {
                    for i in 0..3 {
                        dst[i] = ((src[i] as f32 * alpha + dst[i] as f32 * old * (1.0 - alpha))
                            / output)
                            .round() as u8;
                    }
                }
                dst[3] = (output * 255.0).round() as u8;
            },
        );
        Ok(rgba)
    }
}

// Some scripts have no italic face. Preserve the chosen face where it works,
// and synthesize slant only for clusters that could not be shaped with it.
fn resolve_missing_style(buffer: &mut Buffer, system: &mut FontSystem, attrs: &Attrs<'_>) {
    if attrs.style == Style::Normal {
        return;
    }
    let missing: Vec<_> = buffer
        .layout_runs()
        .flat_map(|run| {
            run.glyphs
                .iter()
                .filter(|g| g.glyph_id == 0)
                .map(move |g| (run.line_i, g.start..g.end))
        })
        .collect();
    if missing.is_empty() {
        return;
    }
    let fallback = attrs
        .clone()
        .style(Style::Normal)
        .cache_key_flags(CacheKeyFlags::FAKE_ITALIC);
    for (line, range) in missing {
        let line = &mut buffer.lines[line];
        let mut list = line.attrs_list().clone();
        list.add_span(range, &fallback);
        line.set_attrs_list(list);
    }
    buffer.shape_until_scroll(system, false);
}

#[cfg(test)]
mod tests;
