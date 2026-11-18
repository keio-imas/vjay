use super::*;
use crate::gpu::presentation::PreviewRegion;
use objc2_core_graphics::CGMutablePath;
use objc2_quartz_core::CAShapeLayer;
impl MacGpuPreview {
    pub(super) fn apply_regions(&mut self, regions: &[PreviewRegion]) {
        let Some(bounds) = self.last_bounds else {
            return;
        };
        let dimensions = [bounds.width, bounds.height, bounds.scale];
        if self.regions == regions && self.mask_bounds == Some(dimensions) {
            return;
        }
        self.mask_bounds = Some(dimensions);
        self.regions = regions.to_vec();
        let path = CGMutablePath::new();
        for region in regions {
            let rect = NSRect::new(
                NSPoint::new(
                    region.x as f64,
                    (bounds.height - region.y - region.height) as f64,
                ),
                NSSize::new(region.width as f64, region.height as f64),
            );
            // A null transform uses the layer's point coordinates.
            unsafe {
                CGMutablePath::add_rect(Some(&path), std::ptr::null(), rect);
            }
        }
        let mask = CAShapeLayer::new();
        mask.setFrame(NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(bounds.width as f64, bounds.height as f64),
        ));
        mask.setContentsScale(bounds.scale as f64);
        mask.setPath(Some(&path));
        // The newly created mask is not in any layer tree, so this cannot form a cycle.
        unsafe {
            self.layer.setMask(Some(&mask));
        }
    }
}
