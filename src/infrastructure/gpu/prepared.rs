use std::collections::HashMap;
use vjay_application::{
    compiler::RenderPlan,
    media::PreparedImageFactory,
    ports::{Extent, PortResult},
};
use vjay_domain::{execution::PreparedImage, graph::NodeId};
#[derive(Clone)]
struct Entry {
    source: PreparedImage,
    texture: wgpu::Texture,
    revision: u64,
}
#[derive(Default)]
pub(super) struct PreparedInputs {
    entries: HashMap<NodeId, Entry>,
    extent: Option<Extent>,
    active: Vec<(usize, NodeId, Entry)>,
    pub errors: HashMap<NodeId, String>,
}
impl PreparedInputs {
    pub fn prepare(
        &mut self,
        gpu: (&wgpu::Device, &wgpu::Queue),
        plan: &RenderPlan,
        extent: Extent,
        factory: Option<&dyn PreparedImageFactory>,
        views: &mut [Option<wgpu::TextureView>],
    ) -> PortResult<()> {
        if self.extent != Some(extent) {
            self.entries.clear();
            self.extent = Some(extent);
        }
        self.errors.clear();
        self.active.clear();
        self.entries.retain(|node, _| {
            plan.steps
                .iter()
                .any(|s| s.node == *node && s.operation.prepared.is_some())
        });
        for (index, step) in plan.steps.iter().enumerate() {
            let Some(source) = &step.operation.prepared else {
                continue;
            };
            let reusable = self
                .entries
                .get(&step.node)
                .is_some_and(|e| e.source == *source);
            if !reusable {
                let result = factory
                    .ok_or_else(|| "Generated image adapter unavailable".to_string())
                    .and_then(|f| f.load(source, extent));
                let rgba = match result {
                    Ok(v) => v,
                    Err(e) => {
                        if step.program_required {
                            return Err(e);
                        }
                        self.errors.insert(step.node, e);
                        continue;
                    }
                };
                if rgba.len() != (extent.width * extent.height * 4) as usize {
                    return Err("Generated image has an invalid size".into());
                }
                let texture = gpu.0.create_texture(&wgpu::TextureDescriptor {
                    label: Some("prepared image"),
                    size: wgpu::Extent3d {
                        width: extent.width,
                        height: extent.height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                gpu.1.write_texture(
                    texture.as_image_copy(),
                    &rgba,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(extent.width * 4),
                        rows_per_image: Some(extent.height),
                    },
                    texture.size(),
                );
                self.entries.insert(
                    step.node,
                    Entry {
                        source: source.clone(),
                        texture,
                        revision: step.prepared_revision,
                    },
                );
            }
            self.active
                .push((index, step.node, self.entries[&step.node].clone()));
            views[index] = Some(
                self.entries[&step.node]
                    .texture
                    .create_view(&Default::default()),
            );
        }
        Ok(())
    }
    pub fn update(
        &mut self,
        queue: &wgpu::Queue,
        plan: &RenderPlan,
        factory: Option<&dyn PreparedImageFactory>,
    ) -> PortResult<()> {
        let Some(extent) = self.extent else {
            return Ok(());
        };
        for (step, node, entry) in &mut self.active {
            let step = &plan.steps[*step];
            if step.prepared_revision == entry.revision {
                continue;
            }
            entry.revision = step.prepared_revision;
            let source = step
                .operation
                .prepared
                .as_ref()
                .expect("compiled prepared input");
            if source == &entry.source {
                continue;
            }
            let result = factory
                .ok_or_else(|| "Generated image adapter unavailable".to_string())
                .and_then(|f| f.load(source, extent));
            match result {
                Ok(rgba) if rgba.len() == (extent.width * extent.height * 4) as usize => {
                    queue.write_texture(
                        entry.texture.as_image_copy(),
                        &rgba,
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(extent.width * 4),
                            rows_per_image: Some(extent.height),
                        },
                        entry.texture.size(),
                    );
                    entry.source = source.clone();
                    self.errors.remove(node);
                }
                Ok(_) => return Err("Generated image has an invalid size".into()),
                Err(error) => {
                    self.errors.insert(*node, error);
                }
            }
        }
        Ok(())
    }
}
