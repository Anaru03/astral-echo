use render_core::{Camera, RasterMesh, Transform};
use render_cpu::{mvp, Color, CpuPipeline, Framebuffer, PipelineConfig, Topology, Viewport};
use render_gpu::{GpuImage, CLEAR_RGBA};

pub fn cpu_image(
    scene: &[(RasterMesh, Transform)],
    camera: &Camera,
    width: u32,
    height: u32,
) -> GpuImage {
    let mut fb = Framebuffer::new(width, height).unwrap();
    draw_cpu(&mut fb, scene, camera);
    let rgba = fb.color_bytes().to_vec();
    let depth = fb.depth_slice_mut().to_vec();
    GpuImage {
        width,
        height,
        rgba,
        depth,
    }
}

pub fn draw_cpu(fb: &mut Framebuffer, scene: &[(RasterMesh, Transform)], camera: &Camera) {
    let (width, height) = (fb.width(), fb.height());
    let mut pipeline = CpuPipeline::new(
        fb,
        Viewport::from_size(width, height).unwrap(),
        PipelineConfig::default(),
    );
    pipeline.clear(Color::new(
        CLEAR_RGBA[0],
        CLEAR_RGBA[1],
        CLEAR_RGBA[2],
        CLEAR_RGBA[3],
    ));
    for (mesh, transform) in scene {
        pipeline
            .draw_mesh(mesh, mvp(camera, *transform), Topology::TriangleList)
            .unwrap();
    }
}

#[derive(Debug)]
pub struct Comparison {
    pub mean_channel_error: f64,
    pub color_outliers: usize,
    pub depth_outliers: usize,
    pub max_depth_error: f32,
    pub pixels: usize,
}

impl Comparison {
    pub fn passes(&self) -> bool {
        self.mean_channel_error <= 0.25
            && self.color_outliers as f64 / self.pixels as f64 <= 0.005
            && self.depth_outliers as f64 / self.pixels as f64 <= 0.005
    }
}

pub fn compare(cpu: &GpuImage, gpu: &GpuImage) -> Comparison {
    assert_eq!((cpu.width, cpu.height), (gpu.width, gpu.height));
    assert_eq!(cpu.rgba.len(), gpu.rgba.len());
    assert_eq!(cpu.depth.len(), gpu.depth.len());
    let mut result = Comparison {
        mean_channel_error: 0.0,
        color_outliers: 0,
        depth_outliers: 0,
        max_depth_error: 0.0,
        pixels: cpu.depth.len(),
    };
    for (a, b) in cpu.rgba.chunks_exact(4).zip(gpu.rgba.chunks_exact(4)) {
        let mut outlier = false;
        for (a, b) in a.iter().zip(b) {
            let difference = a.abs_diff(*b);
            result.mean_channel_error += f64::from(difference);
            outlier |= difference > 2;
        }
        result.color_outliers += usize::from(outlier);
    }
    result.mean_channel_error /= cpu.rgba.len() as f64;
    for (a, b) in cpu.depth.iter().zip(&gpu.depth) {
        assert!(a.is_finite() && b.is_finite());
        let error = (a - b).abs();
        result.max_depth_error = result.max_depth_error.max(error);
        result.depth_outliers += usize::from(error > 1e-5);
    }
    result
}
