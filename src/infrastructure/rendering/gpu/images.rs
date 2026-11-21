mod cache;
mod worker;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use vjay_application::{
    compiler::RenderPlan,
    media::{FrameDelivery, ImageLoader, PreparedImageFactory},
    ports::*,
};
use vjay_domain::{graph::NodeId, node::AssetId};
use worker::{Completed, Job, Pool, Source};

struct Input {
    step: usize,
    node: NodeId,
    texture: wgpu::Texture,
    source: Arc<Source>,
    applied: Option<Arc<Source>>,
    revision: u64,
    current: Arc<AtomicU64>,
    ticket: u64,
    finished: u64,
    active: Option<u64>,
    pending: Option<Job>,
}
impl Input {
    fn cancel(&self) {
        self.current.store(0, Ordering::Release);
    }
}
impl Drop for Input {
    fn drop(&mut self) {
        self.cancel();
    }
}

pub(super) struct ImageInputs {
    files: Option<Arc<dyn ImageLoader>>,
    generated: Option<Arc<dyn PreparedImageFactory>>,
    pool: Option<Pool>,
    inputs: Vec<Input>,
    extent: Option<Extent>,
    next_ticket: u64,
    pub errors: HashMap<NodeId, String>,
}
impl ImageInputs {
    pub fn new(
        files: Option<Arc<dyn ImageLoader>>,
        generated: Option<Arc<dyn PreparedImageFactory>>,
    ) -> Self {
        Self {
            files,
            generated,
            pool: None,
            inputs: Vec::new(),
            extent: None,
            next_ticket: 1,
            errors: HashMap::new(),
        }
    }
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        plan: &RenderPlan,
        extent: Extent,
        assets: &HashMap<AssetId, PathBuf>,
        textures: &mut [Option<wgpu::Texture>],
    ) -> PortResult<()> {
        if self.extent != Some(extent) {
            self.inputs.clear();
            self.extent = Some(extent);
        }
        let mut old = std::mem::take(&mut self.inputs);
        for (step, node) in plan.steps.iter().enumerate() {
            let source = if let Some(source) = &node.operation.prepared {
                Source::Prepared(source.clone())
            } else if node.still_image {
                Source::File {
                    asset: node.asset.ok_or("Choose an image file")?,
                    path: assets
                        .get(&node.asset.ok_or("Choose an image file")?)
                        .ok_or("Image asset is missing")?
                        .clone(),
                }
            } else {
                continue;
            };
            if self.pool.is_none() {
                self.pool = Some(Pool::new(self.files.clone(), self.generated.clone())?);
            }
            let mut input = if let Some(i) = old.iter().position(|i| i.node == node.node) {
                old.swap_remove(i)
            } else {
                Input {
                    step,
                    node: node.node,
                    texture: device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("asynchronous image input"),
                        size: wgpu::Extent3d {
                            width: extent.width,
                            height: extent.height,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING
                            | wgpu::TextureUsages::COPY_DST
                            | wgpu::TextureUsages::COPY_SRC,
                        view_formats: &[],
                    }),
                    source: Arc::new(source.clone()),
                    applied: None,
                    revision: node.prepared_revision,
                    current: Arc::new(AtomicU64::new(0)),
                    ticket: 0,
                    finished: 0,
                    active: None,
                    pending: None,
                }
            };
            input.step = step;
            input.revision = node.prepared_revision;
            self.request(&mut input, source)?;
            textures[step] = Some(input.texture.clone());
            self.inputs.push(input);
        }
        self.errors
            .retain(|node, _| self.inputs.iter().any(|i| i.node == *node));
        Ok(())
    }
    fn request(&mut self, input: &mut Input, source: Source) -> PortResult<()> {
        if input.ticket != 0 && *input.source == source {
            return Ok(());
        }
        let ticket = self.next_ticket;
        self.next_ticket = self
            .next_ticket
            .checked_add(1)
            .ok_or("Image job IDs exhausted")?;
        input.ticket = ticket;
        input.source = Arc::new(source);
        input.pending = None;
        if input
            .applied
            .as_ref()
            .is_some_and(|source| source == &input.source)
        {
            input.cancel();
            input.active = None;
            input.finished = ticket;
        } else {
            input.pending = Some(Job {
                ticket,
                current: input.current.clone(),
                source: input.source.clone(),
                extent: self.extent.ok_or("Missing image extent")?,
            });
        }
        self.errors.remove(&input.node);
        Ok(())
    }
    pub fn update(
        &mut self,
        queue: &wgpu::Queue,
        plan: &RenderPlan,
        delivery: FrameDelivery,
    ) -> PortResult<()> {
        // Move the schedule temporarily so requests can allocate only on content changes.
        let mut inputs = std::mem::take(&mut self.inputs);
        let revised: PortResult<()> = (|| {
            for input in &mut inputs {
                let step = &plan.steps[input.step];
                if input.revision != step.prepared_revision {
                    input.revision = step.prepared_revision;
                    if let Some(source) = &step.operation.prepared {
                        self.request(input, Source::Prepared(source.clone()))?;
                    }
                }
            }
            Ok(())
        })();
        self.inputs = inputs;
        revised?;
        let Some(pool) = &self.pool else {
            return Ok(());
        };
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        loop {
            for input in &mut self.inputs {
                if input.active.is_none()
                    && let Some(job) = input.pending.take()
                {
                    let ticket = job.ticket;
                    input.current.store(ticket, Ordering::Release);
                    input.pending = pool.submit(job)?;
                    if input.pending.is_none() {
                        input.active = Some(ticket);
                    }
                }
            }
            while let Some(done) = pool.poll()? {
                Self::apply(&mut self.inputs, &mut self.errors, queue, done);
            }
            for input in &mut self.inputs {
                if input.active.is_none()
                    && let Some(job) = input.pending.take()
                {
                    let ticket = job.ticket;
                    input.current.store(ticket, Ordering::Release);
                    input.pending = pool.submit(job)?;
                    if input.pending.is_none() {
                        input.active = Some(ticket);
                    }
                }
            }
            if delivery == FrameDelivery::Latest
                || self.inputs.iter().all(|i| i.finished == i.ticket)
            {
                break;
            }
            if std::time::Instant::now() >= deadline {
                return Err("Image preparation timed out".into());
            }
            if let Some(done) = pool.wait()? {
                Self::apply(&mut self.inputs, &mut self.errors, queue, done);
            }
        }
        for input in &self.inputs {
            if plan.steps[input.step].program_required
                && let Some(error) = self.errors.get(&input.node)
            {
                return Err(error.clone());
            }
        }
        Ok(())
    }
    fn apply(
        inputs: &mut [Input],
        errors: &mut HashMap<NodeId, String>,
        queue: &wgpu::Queue,
        result: Completed,
    ) {
        let Some(input) = inputs.iter_mut().find(|i| i.active == Some(result.ticket)) else {
            return;
        };
        input.active = None;
        // Coalesce generated revisions behind one in-flight job, allowing
        // monotonic progress even when editing faster than rasterization.
        // A superseded file selection must never replace the selected file.
        let compatible = result.source == input.source
            || matches!((result.source.as_ref(),input.source.as_ref()),
            (Source::Prepared(a),Source::Prepared(b)) if a.provider == b.provider);
        if !compatible {
            return;
        }
        input.finished = result.ticket;
        match result.pixels {
            Ok(pixels) => {
                queue.write_texture(
                    input.texture.as_image_copy(),
                    &pixels,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(input.texture.width() * 4),
                        rows_per_image: Some(input.texture.height()),
                    },
                    input.texture.size(),
                );
                input.applied = Some(result.source);
                errors.remove(&input.node);
            }
            Err(error) => {
                if input.ticket == result.ticket {
                    errors.insert(input.node, error);
                }
            }
        }
    }
}
