use glam::{Vec3, Vec4};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RasterVertex {
    pub position: Vec3,
    pub color: Vec4,
    pub normal: Vec3,
}

impl RasterVertex {
    pub const fn new(position: Vec3, color: Vec4) -> Self {
        Self {
            position,
            color,
            normal: Vec3::ZERO,
        }
    }

    pub fn with_normal(mut self, normal: Vec3) -> Self {
        self.normal = normal.normalize_or_zero();
        self
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
        for (normal, positions) in faces {
            let base = vertices.len() as u32;
            for p in positions {
                vertices.push(RasterVertex::new(p + center, color).with_normal(normal));
            }
            indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        Self { vertices, indices }
    }

    pub fn resolved_normals(&self) -> Vec<Vec3> {
        let mut normals = vec![Vec3::ZERO; self.vertices.len()];
        for triangle in self.indices.chunks_exact(3) {
            let [a, b, c] = [
                triangle[0] as usize,
                triangle[1] as usize,
                triangle[2] as usize,
            ];
            if a >= normals.len() || b >= normals.len() || c >= normals.len() {
                continue;
            }
            let n = (self.vertices[b].position - self.vertices[a].position)
                .cross(self.vertices[c].position - self.vertices[a].position);
            if n.is_finite() {
                for index in [a, b, c] {
                    normals[index] += n;
                }
            }
        }
        for (normal, vertex) in normals.iter_mut().zip(&self.vertices) {
            *normal = if vertex.normal.is_finite() && vertex.normal.length_squared() > 1e-12 {
                vertex.normal.normalize()
            } else {
                normal.normalize_or_zero()
            };
        }
        normals
    }

    pub fn triangle_indices(&self) -> impl Iterator<Item = (u32, u32, u32)> + '_ {
        self.indices.chunks_exact(3).map(|c| (c[0], c[1], c[2]))
    }
}
