use crate::math::{Vec3, Vec4};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MaterialHandle(pub u32);

impl MaterialHandle {
    pub const INVALID: Self = Self(u32::MAX);

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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MaterialInfo {
    pub base_color: Vec4,
    pub emissive: Vec3,
    pub roughness: f32,
    pub metallic: f32,
}

impl MaterialInfo {
    pub fn albedo(self) -> Vec4 {
        self.base_color
    }

    pub fn sanitized(self) -> Self {
        Self {
            base_color: if self.base_color.is_finite() {
                self.base_color.clamp(Vec4::ZERO, Vec4::ONE)
            } else {
                Vec4::ONE
            },
            emissive: if self.emissive.is_finite() {
                self.emissive.max(Vec3::ZERO)
            } else {
                Vec3::ZERO
            },
            metallic: if self.metallic.is_finite() {
                self.metallic.clamp(0.0, 1.0)
            } else {
                0.0
            },
            roughness: if self.roughness.is_finite() {
                self.roughness.clamp(0.045, 1.0)
            } else {
                1.0
            },
        }
    }

    pub fn fixture_metal() -> Self {
        Self::new(Vec4::new(1.0, 0.71, 0.29, 1.0), 0.15, 1.0)
    }
    pub fn fixture_rough() -> Self {
        Self::new(Vec4::new(0.2, 0.5, 0.8, 1.0), 0.95, 0.0)
    }
    pub fn fixture_emissive() -> Self {
        Self::new(Vec4::ONE, 0.8, 0.0).with_emissive(Vec3::new(0.4, 0.1, 0.02))
    }

    pub fn new(base_color: Vec4, roughness: f32, metallic: f32) -> Self {
        Self {
            base_color,
            emissive: Vec3::ZERO,
            roughness: roughness.clamp(0.0, 1.0),
            metallic: metallic.clamp(0.0, 1.0),
        }
    }

    pub fn with_emissive(mut self, emissive: Vec3) -> Self {
        self.emissive = emissive;
        self
    }

    pub fn ship() -> Self {
        Self::new(Vec4::new(0.03, 0.5, 1.0, 1.0), 0.4, 0.1).with_emissive(Vec3::new(0.0, 0.2, 0.5))
    }

    pub fn enemy() -> Self {
        Self::new(Vec4::new(0.9, 0.03, 0.1, 1.0), 0.5, 0.0)
    }

    pub fn destination() -> Self {
        Self::new(Vec4::new(0.07, 0.85, 0.35, 1.0), 0.7, 0.0)
    }

    pub fn sun() -> Self {
        Self::new(Vec4::new(1.0, 0.55, 0.08, 1.0), 0.5, 0.0)
            .with_emissive(Vec3::new(1.0, 0.24, 0.02))
    }

    pub fn planet() -> Self {
        Self::new(Vec4::ONE, 0.85, 0.0)
    }

    pub fn station() -> Self {
        Self::new(Vec4::new(0.2, 0.55, 0.85, 1.0), 0.55, 0.65)
            .with_emissive(Vec3::new(0.02, 0.08, 0.14))
    }

    pub fn derelict() -> Self {
        Self::new(Vec4::new(0.28, 0.22, 0.18, 1.0), 0.92, 0.05)
    }
}

impl Default for MaterialInfo {
    fn default() -> Self {
        Self::new(Vec4::ONE, 0.8, 0.0)
    }
}