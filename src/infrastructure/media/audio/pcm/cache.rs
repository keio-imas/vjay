use super::{Buffer, FRAMES};
use std::collections::HashMap;

// Eight MiB per source at most. CLOCK replacement reuses page allocations.
const PAGES: usize = 256;
struct Page {
    key: u64,
    samples: Buffer,
    length: usize,
    used: bool,
}
pub(super) struct Cache {
    indices: HashMap<u64, usize>,
    pages: Vec<Page>,
    hand: usize,
}
impl Default for Cache {
    fn default() -> Self {
        Self {
            indices: HashMap::with_capacity(PAGES),
            pages: Vec::with_capacity(PAGES),
            hand: 0,
        }
    }
}
impl Cache {
    pub fn contains(&self, key: u64) -> bool {
        self.indices.contains_key(&key)
    }
    pub fn get(&mut self, key: u64) -> Option<&[[f32; 2]]> {
        let index = *self.indices.get(&key)?;
        let page = &mut self.pages[index];
        page.used = true;
        Some(&page.samples[..page.length])
    }
    pub fn insert(&mut self, key: u64, samples: &[[f32; 2]]) {
        let index = if self.pages.len() < PAGES {
            self.pages.push(Page {
                key,
                samples: vec![[0.0; 2]; FRAMES].into_boxed_slice(),
                length: 0,
                used: false,
            });
            self.pages.len() - 1
        } else {
            while self.pages[self.hand].used {
                self.pages[self.hand].used = false;
                self.hand = (self.hand + 1) % PAGES;
            }
            let index = self.hand;
            self.hand = (self.hand + 1) % PAGES;
            self.indices.remove(&self.pages[index].key);
            index
        };
        let page = &mut self.pages[index];
        page.key = key;
        page.length = samples.len();
        page.used = true;
        page.samples[..samples.len()].copy_from_slice(samples);
        self.indices.insert(key, index);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn eviction_reuses_bounded_storage_without_corrupting_new_pages() {
        let mut cache = Cache::default();
        for key in 0..PAGES as u64 {
            cache.insert(key, &[[key as f32; 2]]);
        }
        let allocations: std::collections::HashSet<_> =
            cache.pages.iter().map(|p| p.samples.as_ptr()).collect();
        for key in PAGES as u64..PAGES as u64 * 4 {
            cache.insert(key, &[[key as f32; 2]]);
            assert_eq!(cache.get(key).unwrap(), &[[key as f32; 2]]);
        }
        assert_eq!(cache.pages.len(), PAGES);
        assert_eq!(cache.indices.len(), PAGES);
        assert!(
            cache
                .pages
                .iter()
                .all(|p| allocations.contains(&p.samples.as_ptr()))
        );
    }
}
