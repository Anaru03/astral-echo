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

    pub fn sphere(center: Vec3, radius: f32, color: Vec4) -> Self {
        const SEGMENTS: u32 = 16;
        const RINGS: u32 = 8;
        let mut vertices = Vec::with_capacity(((RINGS + 1) * (SEGMENTS + 1)) as usize);
        let mut indices = Vec::with_capacity((RINGS * SEGMENTS * 6) as usize);
        for ring in 0..=RINGS {
            let v = ring as f32 / RINGS as f32;
            let latitude = v * core::f32::consts::PI;
            let sin_latitude = latitude.sin();
            let cos_latitude = latitude.cos();
            for segment in 0..=SEGMENTS {
                let u = segment as f32 / SEGMENTS as f32;
                let longitude = u * core::f32::consts::TAU;
                let normal = Vec3::new(
                    sin_latitude * longitude.cos(),
                    cos_latitude,
                    sin_latitude * longitude.sin(),
                );
                vertices.push(
                    RasterVertex::new(center + normal * radius, color).with_normal(normal),
                );
            }
        }
        let row = SEGMENTS + 1;
        for ring in 0..RINGS {
            for segment in 0..SEGMENTS {
                let a = ring * row + segment;
                let b = a + 1;
                let c = a + row;
                let d = c + 1;
                indices.extend([a, c, b, b, c, d]);
            }
        }
        Self { vertices, indices }
    }

    pub fn planet_surface(center: Vec3, radius: f32) -> Self {
        const SEGMENTS: u32 = 24;
        const RINGS: u32 = 12;
        let mut vertices = Vec::with_capacity(((RINGS + 1) * (SEGMENTS + 1)) as usize);
        let mut indices = Vec::with_capacity((RINGS * SEGMENTS * 6) as usize);
        for ring in 0..=RINGS {
            let v = ring as f32 / RINGS as f32;
            let latitude = v * core::f32::consts::PI;
            let sin_latitude = latitude.sin();
            let cos_latitude = latitude.cos();
            for segment in 0..=SEGMENTS {
                let u = segment as f32 / SEGMENTS as f32;
                let longitude = u * core::f32::consts::TAU;
                let normal = Vec3::new(
                    sin_latitude * longitude.cos(),
                    cos_latitude,
                    sin_latitude * longitude.sin(),
                );
                let continental_noise = (normal.x * 3.7 + normal.z * 2.1).sin()
                    + (normal.x * 8.3 - normal.z * 5.2 + normal.y * 2.0).cos() * 0.45
                    + normal.y * 0.25;
                let color = if continental_noise > 0.72 {
                    if normal.y > 0.72 {
                        Vec4::new(0.78, 0.82, 0.86, 1.0)
                    } else {
                        Vec4::new(0.22, 0.48, 0.12, 1.0)
                    }
                } else {
                    Vec4::new(0.04, 0.18, 0.62, 1.0)
                };
                vertices.push(
                    RasterVertex::new(center + normal * radius, color).with_normal(normal),
                );
            }
        }
        let row = SEGMENTS + 1;
        for ring in 0..RINGS {
            for segment in 0..SEGMENTS {
                let a = ring * row + segment;
                let b = a + 1;
                let c = a + row;
                let d = c + 1;
                indices.extend([a, c, b, b, c, d]);
            }
        }
        Self { vertices, indices }
    }

    pub fn station(color: Vec4) -> Self {
        let mut result = Self::default();
        append_station_part(
            &mut result,
            Self::cube(Vec3::ZERO, 1.0, color),
            Vec3::new(1.2, 0.6, 1.2),
            Vec3::ZERO,
        );
        for (scale, translation) in [
            (Vec3::new(1.8, 0.22, 0.35), Vec3::new(1.8, 0.0, 0.0)),
            (Vec3::new(1.8, 0.22, 0.35), Vec3::new(-1.8, 0.0, 0.0)),
            (Vec3::new(0.35, 0.22, 1.8), Vec3::new(0.0, 0.0, 1.8)),
            (Vec3::new(0.35, 0.22, 1.8), Vec3::new(0.0, 0.0, -1.8)),
        ] {
            append_station_part(
                &mut result,
                Self::cube(Vec3::ZERO, 1.0, color),
                scale,
                translation,
            );
        }
        append_station_part(
            &mut result,
            Self::sphere(Vec3::new(0.0, 0.7, 0.0), 0.45, color),
            Vec3::ONE,
            Vec3::ZERO,
        );
        result
    }

    pub fn derelict(color: Vec4) -> Self {
        let mut result = Self::default();
        append_station_part(
            &mut result,
            Self::cube(Vec3::ZERO, 1.0, color),
            Vec3::new(2.4, 0.55, 0.8),
            Vec3::ZERO,
        );
        append_station_part(
            &mut result,
            Self::cube(Vec3::ZERO, 1.0, color),
            Vec3::new(0.65, 0.35, 1.4),
            Vec3::new(0.9, 0.0, 0.0),
        );
        append_station_part(
            &mut result,
            Self::sphere(Vec3::new(-1.0, 0.0, 0.0), 0.5, color),
            Vec3::ONE,
            Vec3::ZERO,
        );
        result
    }

    pub fn asteroid_field(color: Vec4, radius: f32, seed: u32) -> Self {
        let field_radius = radius.max(0.1);
        let mut result = Self::default();
        let mut state = u64::from(seed) | 1;
        let mut next = || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            ((state >> 40) as f32) / ((1u32 << 24) - 1) as f32
        };
        for index in 0..8 {
            let latitude = next() * 2.0 - 1.0;
            let longitude = index as f32 * core::f32::consts::TAU / 8.0 + next() * 0.45;
            let ring = (1.0 - latitude * latitude).max(0.0).sqrt();
            let direction = Vec3::new(
                ring * longitude.cos(),
                latitude,
                ring * longitude.sin(),
            );
            let center = direction * field_radius * (0.35 + next() * 0.45);
            let size = field_radius * (0.06 + next() * 0.06);
            let scale = Vec3::new(
                size * (0.75 + next() * 0.7),
                size * (0.75 + next() * 0.7),
                size * (0.75 + next() * 0.7),
            );
            append_station_part(
                &mut result,
                Self::sphere(Vec3::ZERO, 1.0, color),
                scale,
                center,
            );
        }
        result
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

fn append_station_part(target: &mut RasterMesh, mut part: RasterMesh, scale: Vec3, translation: Vec3) {
    let base = target.vertices.len() as u32;
    for vertex in &mut part.vertices {
        vertex.position = vertex.position * scale + translation;
    }
    target.vertices.extend(part.vertices);
    target
        .indices
        .extend(part.indices.into_iter().map(|index| index + base));
}