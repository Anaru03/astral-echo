@vertex fn vs_depth(@location(0) position: vec3<f32>, @location(3) c0: vec4<f32>, @location(4) c1: vec4<f32>, @location(5) c2: vec4<f32>, @location(6) c3: vec4<f32>) -> @builtin(position) vec4<f32> {
    return mat4x4<f32>(c0,c1,c2,c3)*vec4<f32>(position,1.0);
}
