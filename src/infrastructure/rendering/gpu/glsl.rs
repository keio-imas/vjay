use vjay_application::ports::PortResult;

pub fn compile(body: &str) -> PortResult<String> {
    if body.len() > 256 * 1024 {
        return Err("GLSL source exceeds 256 KiB".into());
    }
    let body = body
        .lines()
        .filter(|line| !line.trim_start().starts_with("#version"))
        .collect::<Vec<_>>()
        .join("\n");
    let wrapper = if body.contains("mainImage") {
        "\nlayout(location=0) out vec4 vjayColor;\nvoid main(){mainImage(vjayColor,vec2(gl_FragCoord.x,frame.viewport.y-gl_FragCoord.y));}\n"
    } else {
        ""
    };
    let source = format!(
        "#version 450\nlayout(std140,set=0,binding=0) uniform Frame {{vec4 viewport;vec4 parameters;vec4 extra[15];}} frame;\nlayout(set=0,binding=1) uniform texture2D inputA;\nlayout(set=0,binding=2) uniform texture2D inputB;\nlayout(set=0,binding=3) uniform sampler inputSampler;\n#define iTime frame.viewport.z\n#define iResolution vec3(frame.viewport.xy,1.0)\n#define iChannel0 sampler2D(inputA,inputSampler)\n#define iChannel1 sampler2D(inputB,inputSampler)\n#define iParameters frame.parameters\n{body}\n{wrapper}"
    );
    let module = naga::front::glsl::Frontend::default()
        .parse(
            &naga::front::glsl::Options {
                stage: naga::ShaderStage::Fragment,
                defines: Default::default(),
            },
            &source,
        )
        .map_err(|errors| errors.emit_to_string(&source))?;
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .map_err(|e| e.to_string())?;
    naga::back::wgsl::write_string(
        &module,
        &info,
        naga::back::wgsl::WriterFlags::EXPLICIT_TYPES,
    )
    .map_err(|e| e.to_string())
}
pub(super) fn pipeline(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    body: &str,
) -> PortResult<wgpu::RenderPipeline> {
    super::programs::pipeline(device, layout, &compile(body)?, "main")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shadertoy_uniforms_and_texture_inputs_compile_and_bad_source_has_diagnostics() {
        let source=compile("void mainImage(out vec4 c,in vec2 p){c=texture(iChannel0,p/iResolution.xy)+vec4(iParameters.xyz,sin(iTime));}").unwrap();
        assert!(source.contains("@fragment"));
        assert!(compile("void mainImage(out vec4 c,in vec2 p){ c=unknown_function(p); }").is_err());
        assert!(compile(&"x".repeat(256 * 1024 + 1)).is_err());
    }
}
