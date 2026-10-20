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
}

impl Default for MaterialInfo {
    fn default() -> Self {
        Self::new(Vec4::ONE, 0.8, 0.0)
    }
}
