use glam::{Vec3, Vec4};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RasterVertex {
    pub position: Vec3,
    pub color: Vec4,
}

impl RasterVertex {
    pub const fn new(position: Vec3, color: Vec4) -> Self {
        Self { position, color }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RasterMesh {
    pub vertices: Vec<RasterVertex>,
    pub indices: Vec<u32>,
}

impl RasterMesh {
    pub fn new(vertices: Vec<RasterVertex>, indices: Vec<u32>) -> Self {
        Self { vertices, indices }
    }

    pub fn triangle(a: RasterVertex, b: RasterVertex, c: RasterVertex) -> Self {
        Self {
            vertices: vec![a, b, c],
            indices: vec![0, 1, 2],
        }
    }

    pub fn cube(center: Vec3, size: f32, color: Vec4) -> Self {
        let h = size * 0.5;
        let faces: [(Vec3, [Vec3; 4]); 6] = [
            (
                Vec3::Z,
                [
                    Vec3::new(-h, -h, h),
                    Vec3::new(h, -h, h),
                    Vec3::new(h, h, h),
                    Vec3::new(-h, h, h),
                ],
            ),
            (
                Vec3::NEG_Z,
                [
                    Vec3::new(h, -h, -h),
                    Vec3::new(-h, -h, -h),
                    Vec3::new(-h, h, -h),
                    Vec3::new(h, h, -h),
                ],
            ),
            (
                Vec3::X,
                [
                    Vec3::new(h, -h, h),
                    Vec3::new(h, -h, -h),
                    Vec3::new(h, h, -h),
                    Vec3::new(h, h, h),
                ],
            ),
            (
                Vec3::NEG_X,
                [
                    Vec3::new(-h, -h, -h),
                    Vec3::new(-h, -h, h),
                    Vec3::new(-h, h, h),
                    Vec3::new(-h, h, -h),
                ],
            ),
            (
                Vec3::Y,
                [
                    Vec3::new(-h, h, h),
                    Vec3::new(h, h, h),
                    Vec3::new(h, h, -h),
                    Vec3::new(-h, h, -h),
                ],
            ),
            (
                Vec3::NEG_Y,
                [
                    Vec3::new(-h, -h, -h),
                    Vec3::new(h, -h, -h),
                    Vec3::new(h, -h, h),
                    Vec3::new(-h, -h, h),
                ],
            ),
        ];
        let mut vertices = Vec::with_capacity(24);
        let mut indices = Vec::with_capacity(36);
        for (_normal, positions) in faces {
            let base = vertices.len() as u32;
            for p in positions {
                vertices.push(RasterVertex::new(p + center, color));
            }
            indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        Self { vertices, indices }
    }

    pub fn triangle_indices(&self) -> impl Iterator<Item = (u32, u32, u32)> + '_ {
        self.indices
            .chunks_exact(3)
            .filter_map(|c| Some((c[0], c[1], c[2])))
    }
}
