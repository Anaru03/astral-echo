struct CameraUniform { mvp: mat4x4<f32> }
@group(0) @binding(0) var<uniform> camera: CameraUniform;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(@location(0) position: vec3<f32>, @location(1) color: vec4<f32>) -> VertexOutput {
    var result: VertexOutput;
    result.position = camera.mvp * vec4<f32>(position, 1.0);
    result.color = color;
    return result;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> { return input.color; }

@fragment
fn fs_srgb(input: VertexOutput) -> @location(0) vec4<f32> {
    let c = clamp(input.color.rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    let linear = select(pow((c + 0.055) / 1.055, vec3<f32>(2.4)), c / 12.92, c <= vec3<f32>(0.04045));
    return vec4<f32>(linear, input.color.a);
}

@vertex
fn vs_instanced(
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) c0: vec4<f32>,
    @location(3) c1: vec4<f32>,
    @location(4) c2: vec4<f32>,
    @location(5) c3: vec4<f32>,
) -> VertexOutput {
    var result: VertexOutput;
    result.position = mat4x4<f32>(c0, c1, c2, c3) * vec4<f32>(position, 1.0);
    result.color = color;
    return result;
}
