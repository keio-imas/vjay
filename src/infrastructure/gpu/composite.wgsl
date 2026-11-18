struct Frame { viewport:vec4<f32>, parameters:vec4<f32>, extra:array<vec4<f32>,15> }
@group(0) @binding(0) var<uniform> frame:Frame;
@group(0) @binding(1) var a:texture_2d<f32>;
@group(0) @binding(2) var b:texture_2d<f32>;
@group(0) @binding(3) var s:sampler;
@group(0) @binding(4) var c:texture_2d<f32>;
@group(0) @binding(5) var d:texture_2d<f32>;
@group(0) @binding(6) var e:texture_2d<f32>;
@group(0) @binding(7) var f:texture_2d<f32>;
@group(0) @binding(8) var g:texture_2d<f32>;
@group(0) @binding(9) var h:texture_2d<f32>;
fn image(index:u32,uv:vec2<f32>)->vec4<f32> {
    switch index {
        case 0u:{return textureSampleLevel(a,s,uv,0.0);} case 1u:{return textureSampleLevel(b,s,uv,0.0);}
        case 2u:{return textureSampleLevel(c,s,uv,0.0);} case 3u:{return textureSampleLevel(d,s,uv,0.0);}
        case 4u:{return textureSampleLevel(e,s,uv,0.0);} case 5u:{return textureSampleLevel(f,s,uv,0.0);}
        case 6u:{return textureSampleLevel(g,s,uv,0.0);} case 7u:{return textureSampleLevel(h,s,uv,0.0);}
        default:{return vec4(0.0);}
    }
}
fn luma(rgb:vec3<f32>)->f32 {return dot(rgb,vec3(0.2126,0.7152,0.0722));}
fn over(bg:vec4<f32>,fg:vec4<f32>)->vec4<f32> {
    let alpha=fg.a+bg.a*(1.0-fg.a);
    return vec4((fg.rgb*fg.a+bg.rgb*bg.a*(1.0-fg.a))/max(alpha,0.00001),alpha);
}
@fragment fn fragment(@builtin(position) position:vec4<f32>)->@location(0) vec4<f32> {
    let uv=position.xy/frame.viewport.xy;let p=frame.parameters;let extra=frame.extra[0];let mode=u32(frame.viewport.w+0.5);
    let base=image(0u,uv);
    switch mode {
        case 0u:{
            let angle=-radians(extra.x);let co=cos(angle);let si=sin(angle);
            let aspect=vec2(frame.viewport.x/frame.viewport.y,1.0);
            let shifted=(uv-vec2(0.5)-p.xy)*aspect;let rotated=vec2(co*shifted.x-si*shifted.y,si*shifted.x+co*shifted.y);
            let local=rotated/(p.zw*aspect)+vec2(0.5);var fg=image(1u,local);
            if any(local<vec2(0.0)) || any(local>vec2(1.0)){fg=vec4(0.0);}
            fg.a*=extra.y;return over(base,fg);
        }
        case 1u:{return image(u32(clamp(round(p.x),0.0,7.0)),uv);}
        case 2u,12u:{
            let dimensions=max(round(p.xy),vec2(1.0));let cell=floor(uv*dimensions);let local=fract(uv*dimensions);
            if any(local<vec2(p.z*0.5)) || any(local>vec2(1.0-p.z*0.5)){return vec4(0.0);}
            var fitted=(local-vec2(p.z*0.5))/(1.0-p.z);
            if p.w>=0.5 {let fit=vec2(min(1.0,dimensions.x/dimensions.y),min(1.0,dimensions.y/dimensions.x));fitted=(fitted-vec2(0.5))/fit+vec2(0.5);}
            if any(fitted<vec2(0.0)) || any(fitted>vec2(1.0)){return vec4(0.0);}
            return image(select(u32(cell.y*dimensions.x+cell.x),0u,mode==12u),fitted);
        }
        case 3u:{
            if p.w<=0.0 {return base;}
            let key=p.xyz;let key_chroma=vec2(key.b-luma(key),key.r-luma(key));let chroma=vec2(base.b-luma(base.rgb),base.r-luma(base.rgb));
            let alpha=smoothstep(p.w,p.w+extra.x,distance(chroma,key_chroma));
            var rgb=base.rgb;let amount=extra.y*(1.0-alpha);
            if key.g>=key.r && key.g>=key.b {rgb.g-=max(rgb.g-max(rgb.r,rgb.b),0.0)*amount;}
            else if key.b>=key.r {rgb.b-=max(rgb.b-max(rgb.r,rgb.g),0.0)*amount;}
            else {rgb.r-=max(rgb.r-max(rgb.g,rgb.b),0.0)*amount;}
            return vec4(rgb,base.a*alpha);
        }
        case 4u:{let mask=image(1u,uv);var alpha=mix(luma(mask.rgb),mask.a,step(0.5,p.x));alpha=mix(alpha,1.0-alpha,step(0.5,p.y));return vec4(base.rgb,base.a*alpha);}
        case 5u:{if p.x<=0.0{return base;}if p.x>=1.0{return image(1u,uv);}let direction=vec2(cos(radians(p.y)),sin(radians(p.y)));let axis=dot(uv-vec2(0.5),direction)+0.5;let progress=mix(-p.z,1.0+p.z,p.x);return mix(base,image(1u,uv),1.0-smoothstep(progress-p.z,progress+p.z,axis));}
        case 6u:{if uv.x<p.x || uv.x>p.y || uv.y<p.z || uv.y>p.w {return vec4(0.0);}return base;}
        case 7u:{return image(0u,vec2(select(uv.x,1.0-uv.x,p.x>=0.5),select(uv.y,1.0-uv.y,p.y>=0.5)));}
        case 8u:{let displacement=image(1u,uv).rg-vec2(0.5);return image(0u,fract(uv+displacement*p.xy));}
        case 9u:{return vec4(pow(clamp((base.rgb-vec3(p.x))/max(p.y-p.x,0.0001),vec3(0.0),vec3(1.0)),vec3(1.0/p.z)),base.a);}
        case 10u:{return vec4(vec3(smoothstep(p.x-p.y,p.x+p.y,luma(base.rgb))),base.a);}
        case 11u:{return vec4(base.rgb*(1.0-smoothstep(p.x,p.x+p.y,distance(uv,vec2(0.5)))*p.z),base.a);}
        default:{return base;}
    }
}
