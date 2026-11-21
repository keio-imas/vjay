use crate::gpu::presentation::PreviewRegion as R;
fn intersection(a: R, b: R) -> Option<R> {
    let x = a.x.max(b.x);
    let y = a.y.max(b.y);
    let width = (a.x + a.width).min(b.x + b.width) - x;
    let height = (a.y + a.height).min(b.y + b.height) - y;
    (width > 0.0 && height > 0.0).then_some(R {
        x,
        y,
        width,
        height,
    })
}
pub(super) fn visible(bounds: R, viewport: R, occluders: impl Iterator<Item = R>) -> Vec<R> {
    let Some(initial) = intersection(bounds, viewport) else {
        return Vec::new();
    };
    let mut regions = vec![initial];
    for cut in occluders {
        let mut next = Vec::new();
        for region in regions {
            let Some(hole) = intersection(region, cut) else {
                next.push(region);
                continue;
            };
            for r in [
                R {
                    x: region.x,
                    y: region.y,
                    width: region.width,
                    height: hole.y - region.y,
                },
                R {
                    x: region.x,
                    y: hole.y + hole.height,
                    width: region.width,
                    height: region.y + region.height - hole.y - hole.height,
                },
                R {
                    x: region.x,
                    y: hole.y,
                    width: hole.x - region.x,
                    height: hole.height,
                },
                R {
                    x: hole.x + hole.width,
                    y: hole.y,
                    width: region.x + region.width - hole.x - hole.width,
                    height: hole.height,
                },
            ] {
                if r.width > 0.0 && r.height > 0.0 {
                    next.push(r);
                }
            }
        }
        regions = next;
        if regions.is_empty() {
            break;
        }
    }
    for region in &mut regions {
        region.x -= bounds.x;
        region.y -= bounds.y;
    }
    regions
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clipping_subtracts_foreground_cards_without_overlap_or_area_loss() {
        let bounds = R {
            x: 10.0,
            y: 20.0,
            width: 100.0,
            height: 100.0,
        };
        let cut = R {
            x: 60.0,
            y: 45.0,
            width: 100.0,
            height: 50.0,
        };
        let result = visible(bounds, bounds, std::iter::once(cut));
        assert_eq!(
            result.iter().map(|r| r.width * r.height).sum::<f32>(),
            7500.0
        );
        for (i, r) in result.iter().enumerate() {
            assert!(r.x >= 0.0 && r.y >= 0.0);
            for other in &result[..i] {
                assert!(intersection(*r, *other).is_none());
            }
        }
    }
}
