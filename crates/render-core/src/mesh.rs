use crate::math::{Vec2, Vec3};

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct Vertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub uv: Vec2,
}

impl Vertex {
    pub const fn new(position: Vec3, normal: Vec3, uv: Vec2) -> Self {
        Self {
            position,
            normal,
            uv,
        }
    }

    pub fn quad() -> ([Vertex; 4], [u32; 6]) {
        let n = Vec3::Z;
        let verts = [
            Vertex::new(Vec3::new(-0.5, -0.5, 0.0), n, Vec2::new(0.0, 0.0)),
            Vertex::new(Vec3::new(0.5, -0.5, 0.0), n, Vec2::new(1.0, 0.0)),
            Vertex::new(Vec3::new(0.5, 0.5, 0.0), n, Vec2::new(1.0, 1.0)),
            Vertex::new(Vec3::new(-0.5, 0.5, 0.0), n, Vec2::new(0.0, 1.0)),
        ];
        (verts, [0, 1, 2, 0, 2, 3])
    }

    pub fn cube() -> (Vec<Vertex>, Vec<u32>) {
        let faces: [(Vec3, [Vec3; 4], [Vec2; 4]); 6] = [
            (
                Vec3::Z,
                [
                    Vec3::new(-0.5, -0.5, 0.5),
                    Vec3::new(0.5, -0.5, 0.5),
                    Vec3::new(0.5, 0.5, 0.5),
                    Vec3::new(-0.5, 0.5, 0.5),
                ],
                [
                    Vec2::new(0.0, 0.0),
                    Vec2::new(1.0, 0.0),
                    Vec2::new(1.0, 1.0),
                    Vec2::new(0.0, 1.0),
                ],
            ),
            (
                Vec3::NEG_Z,
                [
                    Vec3::new(0.5, -0.5, -0.5),
                    Vec3::new(-0.5, -0.5, -0.5),
                    Vec3::new(-0.5, 0.5, -0.5),
                    Vec3::new(0.5, 0.5, -0.5),
                ],
                [
                    Vec2::new(0.0, 0.0),
                    Vec2::new(1.0, 0.0),
                    Vec2::new(1.0, 1.0),
                    Vec2::new(0.0, 1.0),
                ],
            ),
            (
                Vec3::X,
                [
                    Vec3::new(0.5, -0.5, 0.5),
                    Vec3::new(0.5, -0.5, -0.5),
                    Vec3::new(0.5, 0.5, -0.5),
                    Vec3::new(0.5, 0.5, 0.5),
                ],
                [
                    Vec2::new(0.0, 0.0),
                    Vec2::new(1.0, 0.0),
                    Vec2::new(1.0, 1.0),
                    Vec2::new(0.0, 1.0),
                ],
            ),
            (
                Vec3::NEG_X,
                [
                    Vec3::new(-0.5, -0.5, -0.5),
                    Vec3::new(-0.5, -0.5, 0.5),
                    Vec3::new(-0.5, 0.5, 0.5),
                    Vec3::new(-0.5, 0.5, -0.5),
                ],
                [
                    Vec2::new(0.0, 0.0),
                    Vec2::new(1.0, 0.0),
                    Vec2::new(1.0, 1.0),
                    Vec2::new(0.0, 1.0),
                ],
            ),
            (
                Vec3::Y,
                [
                    Vec3::new(-0.5, 0.5, 0.5),
                    Vec3::new(0.5, 0.5, 0.5),
                    Vec3::new(0.5, 0.5, -0.5),
                    Vec3::new(-0.5, 0.5, -0.5),
                ],
                [
                    Vec2::new(0.0, 0.0),
                    Vec2::new(1.0, 0.0),
                    Vec2::new(1.0, 1.0),
                    Vec2::new(0.0, 1.0),
                ],
            ),
            (
                Vec3::NEG_Y,
                [
                    Vec3::new(-0.5, -0.5, -0.5),
                    Vec3::new(0.5, -0.5, -0.5),
                    Vec3::new(0.5, -0.5, 0.5),
                    Vec3::new(-0.5, -0.5, 0.5),
                ],
                [
                    Vec2::new(0.0, 0.0),
                    Vec2::new(1.0, 0.0),
                    Vec2::new(1.0, 1.0),
                    Vec2::new(0.0, 1.0),
                ],
            ),
        ];
        let mut verts = Vec::with_capacity(24);
        let mut idx = Vec::with_capacity(36);
        for (normal, pos, uvs) in faces {
            let base = verts.len() as u32;
            for i in 0..4 {
                verts.push(Vertex::new(pos[i], normal, uvs[i]));
            }
            idx.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        (verts, idx)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MeshHandle(pub u32);

impl MeshHandle {
    pub const INVALID: Self = Self(u32::MAX);
    pub const QUAD: Self = Self(0);
    pub const CUBE: Self = Self(1);
    pub const SHIP_PLACEHOLDER: Self = Self(2);
    pub const ENEMY_PLACEHOLDER: Self = Self(3);
    pub const DESTINATION_PLACEHOLDER: Self = Self(4);
    pub const COLLECTIBLE_PLACEHOLDER: Self = Self(5);
    pub const SUN_SPHERE: Self = Self(6);
    pub const PLANET_SPHERE: Self = Self(7);

    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u32 {
        self.0
    }

    pub const fn is_valid(self) -> bool {
        self.0 != u32::MAX
    }
}