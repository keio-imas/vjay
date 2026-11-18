use std::{collections::HashMap, path::PathBuf};
use vjay_application::{
    compiler::RenderPlan,
    media::ImageLoader,
    ports::{Extent, PortResult},
};
use vjay_domain::node::AssetId;
#[derive(Default)]
pub(super) struct StillInputs {
    textures: HashMap<AssetId, (PathBuf, wgpu::Texture)>,
    extent: Option<Extent>,
    pub errors: HashMap<vjay_domain::graph::NodeId, String>,
}
impl StillInputs {
    pub fn prepare(
        &mut self,
        gpu: (&wgpu::Device, &wgpu::Queue),
        plan: &RenderPlan,
        extent: Extent,
        assets: &HashMap<AssetId, PathBuf>,
        loader: Option<&dyn ImageLoader>,
        views: &mut [Option<wgpu::TextureView>],
    ) -> PortResult<()> {
        let (device, queue) = gpu;
        self.errors.clear();
        if self.extent != Some(extent) {
            self.textures.clear();
            self.extent = Some(extent);
        }
        self.textures.retain(|id, (path, _)| {
            if assets.get(id) != Some(path) {
                return false;
            }
            plan.steps
                .iter()
                .any(|s| s.still_image && s.asset == Some(*id))
        });
        for (index, step) in plan.steps.iter().enumerate().filter(|(_, s)| s.still_image) {
            let id = step.asset.ok_or("Choose an image file")?;
            if let std::collections::hash_map::Entry::Vacant(entry) = self.textures.entry(id) {
                let loader = loader.ok_or("Image loader adapter is unavailable")?;
                let path = assets.get(&id).ok_or("Image asset is missing")?;
                let rgba = match loader.load(path, extent) {
                    Ok(rgba) => rgba,
                    Err(error) => {
                        if step.program_required {
                            return Err(error);
                        }
                        self.errors.insert(step.node, error);
                        continue;
                    }
                };
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("static image"),
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
                queue.write_texture(
                    texture.as_image_copy(),
                    &rgba,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(extent.width * 4),
                        rows_per_image: Some(extent.height),
                    },
                    wgpu::Extent3d {
                        width: extent.width,
                        height: extent.height,
                        depth_or_array_layers: 1,
                    },
                );
                entry.insert((path.clone(), texture));
            }
            views[index] = Some(self.textures[&id].1.create_view(&Default::default()));
        }
        Ok(())
    }
}
