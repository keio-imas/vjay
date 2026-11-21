use super::worker::Source;
use std::sync::Arc;
use vjay_application::ports::Extent;

pub(super) type Pixels = Arc<Vec<u8>>;
struct Entry {
    source: Arc<Source>,
    extent: Extent,
    pixels: Pixels,
    age: u64,
}
#[derive(Default)]
pub(super) struct RasterCache {
    entries: Vec<Entry>,
    bytes: usize,
    age: u64,
}
impl RasterCache {
    const BUDGET: usize = 64 * 1024 * 1024;
    pub fn get(&mut self, source: &Source, extent: Extent) -> Option<Pixels> {
        let entry = self
            .entries
            .iter_mut()
            .find(|e| e.source.as_ref() == source && e.extent == extent)?;
        self.age += 1;
        entry.age = self.age;
        Some(entry.pixels.clone())
    }
    pub fn insert(&mut self, source: Arc<Source>, extent: Extent, pixels: Pixels) {
        if pixels.len() > Self::BUDGET || self.get(&source, extent).is_some() {
            return;
        }
        while self.bytes + pixels.len() > Self::BUDGET || self.entries.len() >= 4 {
            let Some(oldest) = self
                .entries
                .iter()
                .enumerate()
                .min_by_key(|(_, e)| e.age)
                .map(|(i, _)| i)
            else {
                break;
            };
            self.bytes -= self.entries.swap_remove(oldest).pixels.len();
        }
        self.age += 1;
        self.bytes += pixels.len();
        self.entries.push(Entry {
            source,
            extent,
            pixels,
            age: self.age,
        });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_lru_reuses_pixels_and_preserves_external_ownership() {
        let mut cache = RasterCache::default();
        let extent = Extent::new(1, 1).unwrap();
        let key = |i| {
            Arc::new(Source::File {
                path: format!("{i}.png").into(),
                asset: vjay_domain::node::AssetId(i),
            })
        };
        let pixels = Arc::new(vec![7; 4]);
        cache.insert(key(0), extent, pixels.clone());
        for i in 1..4 {
            cache.insert(key(i), extent, Arc::new(vec![i as u8; 4]));
        }
        assert!(Arc::ptr_eq(&cache.get(&key(0), extent).unwrap(), &pixels));
        cache.insert(key(4), extent, Arc::new(vec![4; 4]));
        assert!(cache.get(&key(1), extent).is_none());
        assert_eq!(cache.entries.len(), 4);
        assert_eq!(cache.bytes, 16);
        assert_eq!(pixels.as_slice(), &[7; 4]);
    }
}
