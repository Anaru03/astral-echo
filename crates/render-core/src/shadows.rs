use crate::{LightingEnvironment, LightingMode, Mat4, RenderLightKind, Vec3};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u32)]
pub enum ShadowResolution {
    R512 = 512,
    #[default]
    R1024 = 1024,
    R2048 = 2048,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShadowConfig {
    pub enabled: bool,
    pub resolution: ShadowResolution,
    pub pcf: bool,
    pub depth_bias: f32,
    pub slope_bias: f32,
    pub normal_bias: f32,
    pub center: Vec3,
    pub half_extent: f32,
    pub depth: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShadowConfigError;

impl Default for ShadowConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            resolution: ShadowResolution::R1024,
            pcf: false,
            depth_bias: 0.001,
            slope_bias: 0.002,
            normal_bias: 0.0,
            center: Vec3::ZERO,
            half_extent: 10.0,
            depth: 40.0,
        }
    }
}

impl ShadowConfig {
    pub fn low() -> Self {
        Self {
            resolution: ShadowResolution::R512,
            ..Self::default()
        }
    }

    pub fn validate(self) -> Result<(), ShadowConfigError> {
        if !self.center.is_finite()
            || !self.half_extent.is_finite()
            || self.half_extent <= 0.0
            || !self.depth.is_finite()
            || self.depth <= 0.1
            || [self.depth_bias, self.slope_bias, self.normal_bias]
                .iter()
                .any(|v| !v.is_finite() || *v < 0.0)
        {
            return Err(ShadowConfigError);
        }
        Ok(())
    }

    pub fn view_projection(self, direction: Vec3) -> Result<Mat4, ShadowConfigError> {
        self.validate()?;
        let direction = crate::normalize_direction(direction);
        if direction == Vec3::ZERO {
            return Err(ShadowConfigError);
        }
        let up = if direction.dot(Vec3::Y).abs() > 0.99 {
            Vec3::Z
        } else {
            Vec3::Y
        };
        let view = Mat4::look_at_rh(self.center - direction * self.depth * 0.5, self.center, up);
        let projection = Mat4::orthographic_rh(
            -self.half_extent,
            self.half_extent,
            -self.half_extent,
            self.half_extent,
            0.1,
            self.depth,
        );
        let matrix = projection * view;
        if !matrix.is_finite() {
            return Err(ShadowConfigError);
        }
        Ok(matrix)
    }
}

impl LightingEnvironment {
    pub fn shadow_view_projection(&self) -> Result<Option<Mat4>, ShadowConfigError> {
        if !self.shadows.enabled || self.mode == LightingMode::Unlit {
            return Ok(None);
        }
        self.shadows.validate()?;
        for light in self.lights() {
            if let RenderLightKind::Directional { direction } = light.kind {
                if light.casts_shadow {
                    return self.shadows.view_projection(direction).map(Some);
                }
            }
        }
        Ok(None)
    }
}
pub fn shadow_fixture() -> crate::PreparedScene {
    use crate::{
        LodLevel, MaterialHandle, MeshHandle, PreparedDraw, PreparedScene, RasterMesh,
        RasterVertex, RenderLight, SnapshotNode, Transform, Vec4, VisibilityClass,
    };
    use std::sync::Arc;
    let floor = Arc::new(RasterMesh {
        vertices: [
            Vec3::new(-4.0, 0.0, -4.0),
            Vec3::new(4.0, 0.0, -4.0),
            Vec3::new(4.0, 0.0, 4.0),
            Vec3::new(-4.0, 0.0, 4.0),
        ]
        .into_iter()
        .map(|position| RasterVertex {
            position,
            color: Vec4::ONE,
            normal: Vec3::Y,
        })
        .collect(),
        indices: vec![0, 2, 1, 0, 3, 2],
    });
    let cube = Arc::new(RasterMesh::cube(Vec3::ZERO, 1.0, Vec4::ONE));
    let draws = [
        (floor, MeshHandle::QUAD, Vec3::ZERO),
        (cube, MeshHandle::CUBE, Vec3::new(0.0, 0.5, 0.0)),
    ]
    .into_iter()
    .enumerate()
    .map(|(id, (mesh, handle, position))| PreparedDraw {
        node: SnapshotNode::destination(
            id as u32,
            Transform::from_translation(position),
            handle,
            MaterialHandle::new(300),
            true,
        ),
        mesh,
        lod: LodLevel::High,
        visibility: VisibilityClass::Inside,
        projected_size_px: 64.0,
    })
    .collect();
    let mut scene = PreparedScene::from_draws(draws);
    scene.lighting.mode = LightingMode::Lambert;
    scene.lighting.ambient = Vec3::splat(0.12);
    scene.lighting.camera_position = Vec3::new(5.0, 6.0, 8.0);
    scene.lighting.set_lights([RenderLight {
        color: Vec3::ONE,
        intensity: 0.8,
        kind: RenderLightKind::Directional {
            direction: Vec3::new(0.8, -1.0, 0.4),
        },
        casts_shadow: true,
    }]);
    scene.lighting.shadows = ShadowConfig {
        half_extent: 6.0,
        depth: 24.0,
        ..Default::default()
    };
    scene
}
