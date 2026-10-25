struct Light { vector: vec4<f32>, color: vec4<f32>, range: vec4<f32> }
struct Environment { camera: vec4<f32>, ambient: vec4<f32>, counts: vec4<f32>, lights: array<Light,5> }
@group(0) @binding(0) var<uniform> environment: Environment;

fn safe_normalize(v: vec3<f32>) -> vec3<f32> {
    let scale = max(max(abs(v.x),abs(v.y)),abs(v.z));
    if scale > 0.0 { let scaled = v/scale; return scaled*inverseSqrt(dot(scaled,scaled)); }
    return vec3<f32>(0.0);
}

fn shade_visibility(color: vec4<f32>, albedo: vec4<f32>, params: vec4<f32>, emissive: vec3<f32>, position: vec3<f32>, normal_input: vec3<f32>, mode: f32, shadow_visibility: f32) -> vec4<f32> {
    let base = color*albedo;
    let normal = safe_normalize(normal_input);
    let view = safe_normalize(environment.camera.xyz-position);
    var result = base.rgb*max(environment.ambient.rgb,vec3<f32>(0.0))+emissive;
    for (var i=0u; i<u32(environment.counts.x); i+=1u) {
        let light = environment.lights[i];
        var direction = -light.vector.xyz;
        var attenuation = 1.0;
        if light.vector.w > 0.5 {
            let delta = light.vector.xyz-position;
            let distance_squared = dot(delta,delta);
            direction = safe_normalize(delta);
            let falloff = clamp(1.0-distance_squared/(light.range.x*light.range.x),0.0,1.0);
            attenuation = falloff*falloff/(1.0+distance_squared);
        }
        if light.vector.w < 0.5 { attenuation *= shadow_visibility; }
        let nl = max(dot(normal,direction),0.0);
        if nl <= 0.0 { continue; }
        let half_vector = safe_normalize(view+direction);
        let nh = max(dot(normal,half_vector),0.0);
        let f0 = mix(vec3<f32>(0.04),base.rgb,params.x);
        var reflected = base.rgb;
        if mode > 2.5 {
            let nv = max(dot(normal,view),0.0001);
            let vh = max(dot(view,half_vector),0.0);
            let a2 = params.y*params.y*params.y*params.y;
            let denominator = nh*nh*(a2-1.0)+1.0;
            let distribution = a2/max(3.14159265359*denominator*denominator,1e-12);
            let k = (params.y+1.0)*(params.y+1.0)/8.0;
            let visibility = (nv/(nv*(1.0-k)+k))*(nl/(nl*(1.0-k)+k));
            let fresnel = f0+(vec3<f32>(1.0)-f0)*pow(1.0-vh,5.0);
            let diffuse = (vec3<f32>(1.0)-fresnel)*(1.0-params.x)*base.rgb/3.14159265359;
            reflected = diffuse+fresnel*(distribution*visibility/max(4.0*nv*nl,0.0001));
        } else if mode > 1.5 {
            let power = clamp(2.0/(params.y*params.y)-2.0,1.0,512.0);
            reflected = base.rgb*(1.0-params.x)+f0*pow(nh,power)*((power+2.0)/8.0);
        }
        result += reflected*light.color.rgb*light.color.w*attenuation*nl;
    }
    var output = max(result,vec3<f32>(0.0));
    if environment.counts.y < 0.5 {
        output = clamp(output,vec3<f32>(0.0),vec3<f32>(1.0));
    }
    return vec4<f32>(output,clamp(base.a,0.0,1.0));
}

fn shade(color: vec4<f32>, albedo: vec4<f32>, params: vec4<f32>, emissive: vec3<f32>, position: vec3<f32>, normal: vec3<f32>, mode: f32) -> vec4<f32> {
    return shade_visibility(color,albedo,params,emissive,position,normal,mode,1.0);
}

struct LitVertex {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) c0: vec4<f32>,
    @location(4) c1: vec4<f32>,
    @location(5) c2: vec4<f32>,
    @location(6) c3: vec4<f32>,
    @location(7) m0: vec4<f32>,
    @location(8) m1: vec4<f32>,
    @location(9) m2: vec4<f32>,
    @location(10) n0: vec4<f32>,
    @location(11) n1: vec4<f32>,
    @location(12) n2: vec4<f32>,
    @location(13) albedo: vec4<f32>,
    @location(14) params: vec4<f32>,
    @location(15) emissive: vec4<f32>,
}

struct LitOutput {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) world: vec3<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) @interpolate(flat) albedo: vec4<f32>,
    @location(4) @interpolate(flat) params: vec4<f32>,
    @location(5) @interpolate(flat) emissive: vec3<f32>,
}

fn lighting_vertex(input: LitVertex) -> LitOutput {
    var result: LitOutput;
    let p = vec4<f32>(input.position,1.0);
    result.clip = mat4x4<f32>(input.c0,input.c1,input.c2,input.c3)*p;
    result.world = vec3<f32>(dot(input.m0,p),dot(input.m1,p),dot(input.m2,p));
    result.normal = safe_normalize(vec3<f32>(dot(input.n0.xyz,input.normal),dot(input.n1.xyz,input.normal),dot(input.n2.xyz,input.normal)));
    result.albedo=input.albedo;result.params=input.params;result.emissive=input.emissive.rgb;
    result.color=input.color;
    if environment.camera.w < 2.5 {
        result.color=shade(input.color,input.albedo,input.params,input.emissive.rgb,result.world,result.normal,environment.camera.w);
    }
    return result;
}

@fragment
fn fs_lighting(input: LitOutput) -> @location(0) vec4<f32> {
    var color=input.color;
    if environment.camera.w > 2.5 {
        color=shade(input.color,input.albedo,input.params,input.emissive,input.world,input.normal,environment.camera.w);
    }
    if environment.ambient.w > 0.5 {
        let c=max(color.rgb,vec3<f32>(0.0));
        color=vec4<f32>(select(pow((c+0.055)/1.055,vec3<f32>(2.4)),c/12.92,c<=vec3<f32>(0.04045)),color.a);
    }
    return color;
}

@vertex fn vs_lighting(input: LitVertex) -> LitOutput { return lighting_vertex(input); }