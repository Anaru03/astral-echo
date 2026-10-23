use crate::math::{Mat4, Vec3};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AabbError {
    NonFinite,
    Inverted,
}

impl Aabb {
    pub fn new(min: Vec3, max: Vec3) -> Result<Self, AabbError> {
        if !min.is_finite() || !max.is_finite() {
            return Err(AabbError::NonFinite);
        }
        if min.x > max.x || min.y > max.y || min.z > max.z {
            return Err(AabbError::Inverted);
        }
        Ok(Self { min, max })
    }

    pub fn unit() -> Self {
        Self {
            min: Vec3::splat(-0.5),
            max: Vec3::splat(0.5),
        }
    }

    pub fn center(self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    pub fn extents(self) -> Vec3 {
        (self.max - self.min) * 0.5
    }

    pub fn contains_point(self, p: Vec3) -> bool {
        p.x >= self.min.x
            && p.x <= self.max.x
            && p.y >= self.min.y
            && p.y <= self.max.y
            && p.z >= self.min.z
            && p.z <= self.max.z
    }

    pub fn intersects(self, other: Aabb) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.y <= other.max.y
            && self.max.y >= other.min.y
            && self.min.z <= other.max.z
            && self.max.z >= other.min.z
    }

    pub fn corners(self) -> [Vec3; 8] {
        let (min, max) = (self.min, self.max);
        [
            Vec3::new(min.x, min.y, min.z),
            Vec3::new(max.x, min.y, min.z),
            Vec3::new(min.x, max.y, min.z),
            Vec3::new(max.x, max.y, min.z),
            Vec3::new(min.x, min.y, max.z),
            Vec3::new(max.x, min.y, max.z),
            Vec3::new(min.x, max.y, max.z),
            Vec3::new(max.x, max.y, max.z),
        ]
    }

    pub fn transformed(self, m: Mat4) -> Self {
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for c in self.corners() {
            let p = m.transform_point3(c);
            min = min.min(p);
            max = max.max(p);
        }
        Self { min, max }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plane {
    pub normal: Vec3,
    pub d: f32,
}

impl Plane {
    fn from_vec4(v: crate::math::Vec4) -> Self {
        let len = Vec3::new(v.x, v.y, v.z).length();
        if len > f32::EPSILON {
            Self {
                normal: Vec3::new(v.x / len, v.y / len, v.z / len),
                d: v.w / len,
            }
        } else {
            Self {
                normal: Vec3::X,
                d: 0.0,
            }
        }
    }

    fn distance(self, p: Vec3) -> f32 {
        self.normal.dot(p) + self.d
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frustum {
    planes: [Plane; 6],
}

impl Frustum {
    pub fn from_view_projection(vp: Mat4) -> Self {
        let rows = [vp.row(0), vp.row(1), vp.row(2), vp.row(3)];
        let planes = [
            Plane::from_vec4(rows[3] + rows[0]),
            Plane::from_vec4(rows[3] - rows[0]),
            Plane::from_vec4(rows[3] + rows[1]),
            Plane::from_vec4(rows[3] - rows[1]),
            Plane::from_vec4(rows[3] + rows[2]),
            Plane::from_vec4(rows[3] - rows[2]),
        ];
        Self { planes }
    }

    pub fn intersects_aabb(self, aabb: Aabb) -> bool {
        for plane in self.planes {
            let p = Vec3::new(
                if plane.normal.x >= 0.0 {
                    aabb.max.x
                } else {
                    aabb.min.x
                },
                if plane.normal.y >= 0.0 {
                    aabb.max.y
                } else {
                    aabb.min.y
                },
                if plane.normal.z >= 0.0 {
                    aabb.max.z
                } else {
                    aabb.min.z
                },
            );
            if plane.distance(p) < 0.0 {
                return false;
            }
        }
        true
    }

    pub fn contains_point(self, p: Vec3) -> bool {
        self.planes.iter().all(|pl| pl.distance(p) >= 0.0)
    }
}
