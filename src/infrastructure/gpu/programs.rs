use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use vjay_application::{compiler::RenderPlan, ports::PortResult};
use vjay_domain::execution::ShaderSource;

#[derive(Clone, Default)]
pub struct ShaderPrograms {
    definitions: HashMap<String, (String, String)>,
}
impl ShaderPrograms {
    pub fn register(&mut self, id: &str, source: &str, entry: &str) -> PortResult<()> {
        if self.definitions.contains_key(id) {
            return Err(format!("Shader program already registered: {id}"));
        }
        self.definitions
            .insert(id.into(), (source.into(), entry.into()));
        Ok(())
    }
}
/// Selected explicitly at the composition root. The renderer accepts any bundle.
pub fn builtin_programs() -> Arc<ShaderPrograms> {
    let mut programs = ShaderPrograms::default();
    programs
        .register("vjay.images.v1", include_str!("node.wgsl"), "fragment")
        .expect("unique built-in shader");
    programs
        .register(
            "vjay.composite.v1",
            include_str!("composite.wgsl"),
            "fragment",
        )
        .expect("unique compositor shader");
    programs
        .register(
            "vjay.spatial3d.v1",
            include_str!("spatial3d.wgsl"),
            "fragment",
        )
        .expect("unique 3D shader");
    Arc::new(programs)
}
#[derive(Default)]
pub(super) struct ShaderCache {
    pipelines: HashMap<ShaderSource, wgpu::RenderPipeline>,
}
pub(super) fn pipeline(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    source: &str,
    entry: &str,
) -> PortResult<wgpu::RenderPipeline> {
    let errors = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("registered fragment program"),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let pipeline = super::resources::create_pipeline_for(device, layout, &module, entry);
    if let Some(error) = pollster::block_on(errors.pop()) {
        return Err(error.to_string());
    }
    Ok(pipeline)
}
impl ShaderCache {
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        plan: &RenderPlan,
        programs: &ShaderPrograms,
    ) -> PortResult<Vec<wgpu::RenderPipeline>> {
        let live: HashSet<_> = plan
            .steps
            .iter()
            .map(|step| &step.operation.shader)
            .collect();
        self.pipelines.retain(|key, _| live.contains(key));
        let mut result = Vec::with_capacity(plan.steps.len());
        for step in &plan.steps {
            let key = &step.operation.shader;
            if !self.pipelines.contains_key(key) {
                let compiled;
                let (source, entry) = match key {
                    ShaderSource::Registered(id) => {
                        let (source, entry) = programs
                            .definitions
                            .get(*id)
                            .ok_or_else(|| format!("Shader program is not registered: {id}"))?;
                        (source.as_str(), entry.as_str())
                    }
                    ShaderSource::Glsl(source) => {
                        compiled = super::glsl::compile(source)?;
                        (compiled.as_str(), "main")
                    }
                    ShaderSource::Wgsl { source, entry } => (source.as_str(), entry.as_str()),
                };
                self.pipelines
                    .insert(key.clone(), pipeline(device, layout, source, entry)?);
            }
            result.push(self.pipelines[key].clone());
        }
        Ok(result)
    }
}
