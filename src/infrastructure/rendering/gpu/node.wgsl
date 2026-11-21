struct Uniforms {
    viewport_time: vec4<f32>,
    parameters: vec4<f32>,
};
@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var image_a: texture_2d<f32>;
@group(0) @binding(2) var image_b: texture_2d<f32>;

@vertex
fn vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let x = f32((index << 1u) & 2u);
    let y = f32(index & 2u);
    return vec4<f32>(x * 2.0 - 1.0, y * 2.0 - 1.0, 0.0, 1.0);
}

fn hue_rgb(hue: f32, saturation: f32) -> vec3<f32> {
    let p = abs(fract(vec3<f32>(hue) + vec3<f32>(0.0, 2.0 / 3.0, 1.0 / 3.0)) * 6.0 - 3.0);
    return mix(vec3<f32>(1.0), clamp(p - 1.0, vec3<f32>(0.0), vec3<f32>(1.0)), saturation);
}

fn hash(p: vec2<f32>) -> f32 { return fract(sin(dot(p,vec2<f32>(127.1,311.7)))*43758.5453); }
fn noise(uv:vec2<f32>) -> f32 {
    let p=uv*u.parameters.x+vec2<f32>(u.viewport_time.z*u.parameters.y,0.0);
    let cell=floor(p); let f=fract(p); let s=f*f*(3.0-2.0*f);
    return mix(mix(hash(cell),hash(cell+vec2<f32>(1.0,0.0)),s.x),mix(hash(cell+vec2<f32>(0.0,1.0)),hash(cell+vec2<f32>(1.0)),s.x),s.y);
}

fn read_a(uv: vec2<f32>) -> vec4<f32> {
    let dimensions = vec2<i32>(textureDimensions(image_a));
    let pixel = clamp(vec2<i32>(uv * vec2<f32>(dimensions)), vec2<i32>(0), dimensions - 1);
    return textureLoad(image_a, pixel, 0);
}

fn transform(uv: vec2<f32>) -> vec4<f32> {
    let aspect = vec2<f32>(u.viewport_time.x / u.viewport_time.y, 1.0);
    let p = (uv - 0.5 - u.parameters.xy) * aspect;
    let angle = -radians(u.parameters.w);
    let rotated = vec2<f32>(cos(angle) * p.x - sin(angle) * p.y, sin(angle) * p.x + cos(angle) * p.y);
    let source = rotated / (aspect * u.parameters.z) + 0.5;
    if any(source < vec2<f32>(0.0)) || any(source > vec2<f32>(1.0)) { return vec4<f32>(0.0); }
    return read_a(source);
}

fn grade(a: vec4<f32>) -> vec4<f32> {
    var color = (a.rgb - 0.5) * u.parameters.y + 0.5 + u.parameters.x;
    let luma = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
    color = mix(vec3<f32>(luma), color, u.parameters.z);
    let angle = radians(u.parameters.w);
    let axis = normalize(vec3<f32>(1.0));
    color = color * cos(angle) + cross(axis, color) * sin(angle) + axis * dot(axis, color) * (1.0 - cos(angle));
    return vec4<f32>(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), a.a);
}

fn kaleidoscope(uv: vec2<f32>) -> vec4<f32> {
    if round(u.parameters.x) <= 1.0 { return read_a(uv); }
    let aspect = vec2<f32>(u.viewport_time.x / u.viewport_time.y, 1.0);
    let p = (uv - 0.5) * aspect;
    let sector = 6.2831853 / round(u.parameters.x);
    let angle = atan2(p.y, p.x) + radians(u.parameters.y);
    let folded = abs(fract(angle / sector) * sector - sector * 0.5);
    return read_a(vec2<f32>(cos(folded), sin(folded)) * length(p) / aspect + 0.5);
}

// Fixed nine-tap kernel: cost is independent of radius.
fn blur(uv: vec2<f32>) -> vec4<f32> {
    let d = vec2<f32>(u.parameters.x) / u.viewport_time.xy;
    var color = read_a(uv) * 4.0;
    color += (read_a(uv + vec2<f32>(d.x,0.0)) + read_a(uv - vec2<f32>(d.x,0.0)) + read_a(uv + vec2<f32>(0.0,d.y)) + read_a(uv - vec2<f32>(0.0,d.y))) * 2.0;
    color += read_a(uv+d) + read_a(uv-d) + read_a(uv+vec2<f32>(d.x,-d.y)) + read_a(uv+vec2<f32>(-d.x,d.y));
    return color / 16.0;
}

// Straight-alpha layer compositing; alpha is preserved through effect nodes.
fn blend(a: vec4<f32>, b: vec4<f32>) -> vec4<f32> {
    var mixed = b.rgb;
    let mode = u32(round(u.parameters.y));
    if mode == 1u { mixed = min(a.rgb + b.rgb, vec3<f32>(1.0)); }
    if mode == 2u { mixed = a.rgb * b.rgb; }
    if mode == 3u { mixed = vec3<f32>(1.0) - (vec3<f32>(1.0)-a.rgb)*(vec3<f32>(1.0)-b.rgb); }
    let alpha = b.a * u.parameters.x;
    let out_alpha = alpha + a.a * (1.0-alpha);
    let rgb = (mixed * alpha + a.rgb * a.a * (1.0-alpha)) / max(out_alpha, 0.00001);
    return vec4<f32>(rgb, out_alpha);
}

@fragment
fn fragment(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let mode = u32(u.viewport_time.w);
    let uv = position.xy / u.viewport_time.xy;
    if mode == 0u {
        var uv = position.xy / u.viewport_time.xy - 0.5;
        uv.x *= u.viewport_time.x / u.viewport_time.y;
        let angle = radians(u.parameters.z);
        let axis = dot(uv, vec2<f32>(cos(angle), sin(angle)));
        let wave = 0.5 + 0.5 * sin(6.2831853 * (axis * u.parameters.x + u.viewport_time.z * u.parameters.y));
        return vec4<f32>(vec3<f32>(wave), 1.0);
    }
    if mode == 4u { return vec4<f32>(u.parameters.xyz, 1.0); }
    if mode == 12u { return vec4<f32>(vec3<f32>(noise(uv)),1.0); }
    if mode == 13u { let tile=floor(uv*u.parameters.x); return vec4<f32>(vec3<f32>(fract((tile.x+tile.y)*0.5)*2.0),1.0); }
    if mode == 14u { let angle=radians(u.parameters.x);let value=clamp(dot(uv-0.5,vec2<f32>(cos(angle),sin(angle)))+0.5,0.0,1.0); return vec4<f32>(vec3<f32>(value),1.0); }
    if mode == 5u { return transform(uv); }
    if mode == 7u { return kaleidoscope(uv); }
    if mode == 10u { return blur(uv); }
    if mode == 17u { let cell=floor(position.xy/u.parameters.x); return read_a((cell+0.5)*u.parameters.x/u.viewport_time.xy); }
    if mode == 18u {
        let d=1.0/u.viewport_time.xy;
        let gx=read_a(uv+vec2<f32>(d.x,0.0)).rgb-read_a(uv-vec2<f32>(d.x,0.0)).rgb;
        let gy=read_a(uv+vec2<f32>(0.0,d.y)).rgb-read_a(uv-vec2<f32>(0.0,d.y)).rgb;
        let amount=u.parameters.x;
        return mix(read_a(uv),vec4<f32>(sqrt(gx*gx+gy*gy)*max(amount,1.0),read_a(uv).a),min(amount,1.0));
    }
    let pixel = vec2<i32>(position.xy);
    let a = textureLoad(image_a, pixel, 0);
    if mode == 15u { return vec4<f32>(mix(a.rgb,vec3<f32>(1.0)-a.rgb,u.parameters.x),a.a); }
    if mode == 16u { let levels=max(2.0,round(u.parameters.x))-1.0;return vec4<f32>(round(a.rgb*levels)/levels,a.a); }
    if mode == 19u { return vec4<f32>(vec3<f32>(a[u32(u.parameters.x)]),1.0); }
    if mode == 1u {
        let luminance = dot(a.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
        let color = hue_rgb(u.parameters.x + luminance * 0.3, u.parameters.y);
        return vec4<f32>(color * luminance * u.parameters.z, a.a);
    }
    if mode == 3u { return a; }
    if mode == 21u { return vec4(a.rgb*u.parameters.rgb,a.a*u.parameters.w); }
    if mode == 6u { return grade(a); }
    if mode == 8u {
        if u.parameters.x <= 0.0 { return a; }
        let luminance = dot(a.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
        return vec4<f32>(a.rgb, a.a * smoothstep(u.parameters.x, u.parameters.x + u.parameters.y, luminance));
    }
    if mode == 9u {
        let gate = select(0.0, 1.0, u.parameters.x == 0.0 || fract(u.viewport_time.z * u.parameters.x) < u.parameters.y);
        return vec4<f32>(a.rgb * gate, a.a);
    }
    let b = textureLoad(image_b, pixel, 0);
    if mode == 11u { return blend(a,b); }
    return mix(a,b,u.parameters.x);
}

