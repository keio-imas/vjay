/// Adapter metrics describe actual encoded work, independently of graph nodes.
#[derive(Clone, Copy, Debug, Default)]
pub struct RenderMetrics {
    pub submitted_frames: u64,
    pub image_passes: usize,
    pub forwarded_sources: usize,
    pub pooled_texture_bytes: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct FrameSample {
    pub monotonic_seconds: f64,
    pub interval_seconds: f64,
    pub target_seconds: f64,
    pub control_seconds: f64,
    pub render_seconds: f64,
    pub total_seconds: f64,
    pub failed: bool,
    pub render: RenderMetrics,
}

/// Implementations must accept or drop a sample without blocking the frame loop.
/// These are CPU submission timings, not display scanout timestamps.
pub trait FrameTrace: Send + Sync {
    fn record(&self, sample: FrameSample);
    fn dropped(&self) -> u64 {
        0
    }
}
