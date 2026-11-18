struct Frame { viewport:vec4<f32>, p:vec4<f32>, extra:array<vec4<f32>,15> }
@group(0) @binding(0) var<uniform> u:Frame;
@group(0) @binding(1) var image:texture_2d<f32>;
@group(0) @binding(3) var samp:sampler;
fn read(uv:vec2<f32>)->vec4<f32> {
    if any(uv<vec2(0.0)) || any(uv>vec2(1.0)) {return vec4(0.0);}
    return textureSampleLevel(image,samp,uv,0.0);
}
fn inverse_rotate(p:vec3<f32>, degrees:vec3<f32>)->vec3<f32> {
    let a=-radians(degrees);let c=cos(a);let s=sin(a);
    let z=vec3(c.z*p.x-s.z*p.y,s.z*p.x+c.z*p.y,p.z);
    let y=vec3(c.y*z.x+s.y*z.z,z.y,-s.y*z.x+c.y*z.z);
    return vec3(y.x,c.x*y.y-s.x*y.z,s.x*y.y+c.x*y.z);
}
fn sdf(p:vec3<f32>,shape:u32)->f32 {
    switch shape {
        case 1u:{let q=abs(p)-vec3(0.65);return length(max(q,vec3(0.0)))+min(max(q.x,max(q.y,q.z)),0.0);}
        case 2u:{let q=vec2(length(p.xz)-0.62,p.y);return length(q)-0.23;}
        case 3u:{let q=abs(vec2(length(p.xz),p.y))-vec2(0.55,0.75);return min(max(q.x,q.y),0.0)+length(max(q,vec2(0.0)));}
        default:{return length(p)-0.8;}
    }
}
@fragment fn fragment(@builtin(position) position:vec4<f32>)->@location(0) vec4<f32> {
    let uv=position.xy/u.viewport.xy;let aspect=u.viewport.x/u.viewport.y;let mode=u32(u.viewport.w+0.5);
    let p=u.p;let e=u.extra[0];
    if mode==0u {
        let camera=1.0/tan(radians(e.w)*0.5);
        let origin=inverse_rotate(vec3(-p.x*2.0,p.y*2.0,camera-p.z),vec3(p.w,e.xy));
        let direction=inverse_rotate(normalize(vec3((uv-vec2(0.5))*vec2(2.0*aspect,-2.0),-camera)),vec3(p.w,e.xy));
        if abs(direction.z)<0.00001 {return vec4(0.0);}
        let distance=-origin.z/direction.z;
        if distance<=0.0 {return vec4(0.0);}
        let q=origin+direction*distance;
        return read(q.xy/(vec2(2.0*aspect,-2.0)*e.z)+vec2(0.5));
    }
    if mode==2u {
        if e.x<=0.0 {return read(uv);}
        let q=(uv-vec2(0.5))*vec2(aspect,1.0);
        let angle=atan2(q.y,q.x)/6.2831853+p.z/360.0;
        let depth=p.x/max(length(q),0.008);
        return mix(read(uv),read(fract(vec2(angle,depth*p.y+u.viewport.z*p.w))),e.x);
    }
    let angles=p.yzw;let shape=u32(round(p.x));let scale=e.x;
    let origin=inverse_rotate(vec3(0.0,0.0,e.y)/scale,angles);
    let maximum_travel=length(origin)+2.0;
    let direction=inverse_rotate(normalize(vec3((uv-vec2(0.5))*vec2(2.0*aspect,-2.0),-2.0)),angles);
    var travel=0.0;var q=origin;var hit=false;
    // A fixed iteration cap bounds per-pixel cost; no CPU mesh or readback.
    for(var step=0u;step<64u;step+=1u){
        q=origin+direction*travel;let distance=sdf(q,shape);
        if distance<0.001 {hit=true;break;}
        travel+=distance;
        if travel>maximum_travel {break;}
    }
    if !hit {return vec4(0.0);}
    let epsilon=0.002;
    let normal=normalize(vec3(
        sdf(q+vec3(epsilon,0.0,0.0),shape)-sdf(q-vec3(epsilon,0.0,0.0),shape),
        sdf(q+vec3(0.0,epsilon,0.0),shape)-sdf(q-vec3(0.0,epsilon,0.0),shape),
        sdf(q+vec3(0.0,0.0,epsilon),shape)-sdf(q-vec3(0.0,0.0,epsilon),shape)));
    let light=0.2+0.8*max(dot(normal,normalize(vec3(-0.4,0.7,1.0))),0.0)*e.w;
    let texture_uv=vec2(atan2(q.z,q.x)/6.2831853+0.5,acos(clamp(q.y/max(length(q),0.001),-1.0,1.0))/3.14159265);
    let color=u.extra[1];let tex=read(texture_uv);
    return vec4(mix(color.rgb,tex.rgb,e.z)*light,color.a*mix(1.0,tex.a,e.z));
}
