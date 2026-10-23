struct ShadowEnvironment { vp: mat4x4<f32>, bias: vec4<f32>, direction: vec4<f32> }
@group(1) @binding(0) var shadow_depth: texture_depth_2d;
@group(1) @binding(1) var shadow_sampler: sampler_comparison;
@group(1) @binding(2) var<uniform> shadow: ShadowEnvironment;

fn shadow_visibility(position: vec3<f32>, normal_input: vec3<f32>) -> f32 {
    let normal = safe_normalize(normal_input);
    let clip = shadow.vp*vec4<f32>(position+normal*shadow.bias.z,1.0);
    let ndc = clip.xyz/clip.w;
    let uv = vec2<f32>(ndc.x*0.5+0.5,0.5-ndc.y*0.5);
    if any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) || ndc.z <= 0.0 || ndc.z >= 1.0 { return 1.0; }
    let slope = 1.0-max(dot(normal,-shadow.direction.xyz),0.0);
    let reference = ndc.z-shadow.bias.x-shadow.bias.y*slope;
    if shadow.direction.w < 0.5 { return textureSampleCompareLevel(shadow_depth,shadow_sampler,uv,reference); }
    var result = 0.0;
    for(var y=-1; y<=1; y+=1) {
        for(var x=-1; x<=1; x+=1) {
            result += textureSampleCompareLevel(shadow_depth,shadow_sampler,uv+vec2<f32>(f32(x),f32(y))*shadow.bias.w,reference);
        }
    }
    return result/9.0;
}

@vertex fn vs_shadow_lighting(input: LitVertex) -> LitOutput {
    var result = lighting_vertex(input);
    result.color = input.color;
    return result;
}

@fragment fn fs_shadow_lighting(input: LitOutput) -> @location(0) vec4<f32> {
    var color = shade_visibility(input.color,input.albedo,input.params,input.emissive,input.world,input.normal,environment.camera.w,shadow_visibility(input.world,input.normal));
    if environment.ambient.w > 0.5 {
        let c=max(color.rgb,vec3<f32>(0.0));
        color=vec4<f32>(select(pow((c+0.055)/1.055,vec3<f32>(2.4)),c/12.92,c<=vec3<f32>(0.04045)),color.a);
    }
    return color;
}
