use vjay_application::{compiler::RenderPlan, ports::Extent};

pub(super) struct Resources {
    pub textures: Vec<wgpu::Texture>,
    pub forwarded: Vec<bool>,
    pub views: Vec<wgpu::TextureView>,
    pub groups: Vec<wgpu::BindGroup>,
    pub uniforms: wgpu::Buffer,
    pub staging: Vec<u8>,
    pub stride: usize,
    pub extent: Extent,
    pub output_slot: usize,
    _fallback: wgpu::Texture,
}

/// Forward only a capture-compatible image with exactly the graph's extent.
pub(super) fn can_forward(texture: &wgpu::Texture, extent: Extent) -> bool {
    texture.usage().contains(wgpu::TextureUsages::COPY_SRC)
        && texture.width() == extent.width
        && texture.height() == extent.height
        && texture.depth_or_array_layers() == 1
        && texture.sample_count() == 1
        && texture.dimension() == wgpu::TextureDimension::D2
        && texture.format() == wgpu::TextureFormat::Rgba8Unorm
}

pub(super) fn create_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let texture = |binding| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    };
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("node bindings"),
        entries: &{
            let mut entries = vec![
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: wgpu::BufferSize::new(272),
                    },
                    count: None,
                },
                texture(1),
                texture(2),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ];
            entries.extend((4..10).map(texture));
            entries
        },
    })
}

pub(super) fn create_pipeline_for(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    fragment: &wgpu::ShaderModule,
    entry: &str,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("node shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("fullscreen.wgsl").into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("node pipeline layout"),
        bind_group_layouts: &[Some(layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("node pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vertex"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: fragment,
            entry_point: Some(entry),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8Unorm,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    })
}

impl Resources {
    pub fn new(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        plan: &RenderPlan,
        extent: Extent,
        video: &[Option<wgpu::Texture>],
    ) -> Self {
        let stride = 272_usize
            .div_ceil(device.limits().min_uniform_buffer_offset_alignment as usize)
            * device.limits().min_uniform_buffer_offset_alignment as usize;
        let staging = vec![0; stride * plan.steps.len().max(1)];
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("node uniforms"),
            size: staging.len() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let make_texture = |width, height| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some("pooled node texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            })
        };
        let mut forwarded = vec![false; plan.steps.len()];
        let mut aliases = vec![None; plan.texture_count];
        for (i, step) in plan.steps.iter().enumerate() {
            if step.external_source()
                && let Some(texture) = &video[i]
                && can_forward(texture, extent)
            {
                aliases[step.target] = Some(texture.clone());
                forwarded[i] = true;
            }
        }
        let textures: Vec<_> = aliases
            .into_iter()
            .map(|source| source.unwrap_or_else(|| make_texture(extent.width, extent.height)))
            .collect();
        let views: Vec<_> = textures
            .iter()
            .map(|t| t.create_view(&Default::default()))
            .collect();
        let sources = video
            .iter()
            .map(|t| t.as_ref().map(|t| t.create_view(&Default::default())))
            .collect::<Vec<_>>();
        let fallback = make_texture(1, 1);
        let fallback_view = fallback.create_view(&Default::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let groups = plan
            .steps
            .iter()
            .enumerate()
            .map(|(index, step)| {
                let input = |port: usize| {
                    if port == 0
                        && let Some(view) = &sources[index]
                    {
                        return view;
                    }
                    step.inputs[port].map_or(&fallback_view, |i| &views[i])
                };
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("node inputs"),
                    layout,
                    entries: &{
                        let mut entries = vec![
                            wgpu::BindGroupEntry {
                                binding: 3,
                                resource: wgpu::BindingResource::Sampler(&sampler),
                            },
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                    buffer: &uniforms,
                                    offset: 0,
                                    size: wgpu::BufferSize::new(272),
                                }),
                            },
                        ];
                        for port in 0..8 {
                            entries.push(wgpu::BindGroupEntry {
                                binding: if port < 2 {
                                    port as u32 + 1
                                } else {
                                    port as u32 + 2
                                },
                                resource: wgpu::BindingResource::TextureView(input(port)),
                            });
                        }
                        entries
                    },
                })
            })
            .collect();
        Self {
            forwarded,
            textures,
            views,
            groups,
            uniforms,
            staging,
            stride,
            extent,
            output_slot: plan.output_slot,
            _fallback: fallback,
        }
    }
}
