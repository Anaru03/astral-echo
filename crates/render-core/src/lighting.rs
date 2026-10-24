use crate::{MaterialInfo, RenderLight, RenderLightKind, RenderSnapshot, Vec3, Vec4};

pub const MAX_POINT_LIGHTS: usize = 4;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u32)]
pub enum LightingMode {
    #[default]
    Unlit = 0,
    Lambert = 1,
    BlinnPhong = 2,
    Pbr = 3,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LightingEnvironment {
    pub mode: LightingMode,
    pub shadows: crate::ShadowConfig,
    pub camera_position: Vec3,
    pub ambient: Vec3,
    lights: Vec<RenderLight>,
}

impl Default for LightingEnvironment {
    fn default() -> Self {
        Self {
            mode: LightingMode::Unlit,
            shadows: crate::ShadowConfig::default(),
            camera_position: Vec3::ZERO,
            ambient: Vec3::splat(0.03),
            lights: Vec::new(),
        }
    }
}

impl LightingEnvironment {
    pub fn from_snapshot(snapshot: &RenderSnapshot, mode: LightingMode) -> Self {
        let mut environment = Self {
            mode,
            camera_position: snapshot.camera_position,
            ..Self::default()
        };
        environment.set_lights(snapshot.lights.iter().map(|light| light.light));
        environment
    }

    pub fn lights(&self) -> &[RenderLight] {
        &self.lights
    }

    pub fn set_lights(&mut self, lights: impl IntoIterator<Item = RenderLight>) {
        self.lights.clear();
        let mut directional = false;
        let mut points = 0;
        for mut light in lights {
            if !light.color.is_finite() || !light.intensity.is_finite() || light.intensity < 0.0 {
                continue;
            }
            light.color = light.color.max(Vec3::ZERO);
            match &mut light.kind {
                RenderLightKind::Directional { direction } => {
                    if directional || !direction.is_finite() || direction.abs().max_element() == 0.0
                    {
                        continue;
                    }
                    *direction = crate::normalize_direction(*direction);
                    directional = true;
                }
                RenderLightKind::Point { position, range } => {
                    if points >= MAX_POINT_LIGHTS
                        || !position.is_finite()
                        || !range.is_finite()
                        || *range <= 0.0
                    {
                        continue;
                    }
                    points += 1;
                }
            }
            self.lights.push(light);
        }
    }
}

pub fn shade_surface(
    environment: &LightingEnvironment,
    material: MaterialInfo,
    color: Vec4,
    position: Vec3,
    normal: Vec3,
) -> Vec4 {
    if environment.mode == LightingMode::Unlit {
        return color;
    }
    let material = material.sanitized();
    let base = color * material.base_color;
    let albedo = base.truncate().max(Vec3::ZERO);
    let normal = crate::normalize_direction(normal);
    let view = crate::normalize_direction(environment.camera_position - position);
    let mut result = albedo * environment.ambient.max(Vec3::ZERO) + material.emissive;
    for light in environment.lights() {
        let (direction, attenuation) = match light.kind {
            RenderLightKind::Directional { direction } => (-direction, 1.0),
            RenderLightKind::Point {
                position: light_position,
                range,
            } => {
                let delta = light_position - position;
                let distance_squared = delta.length_squared();
                let falloff = (1.0 - distance_squared / (range * range)).clamp(0.0, 1.0);
                (
                    crate::normalize_direction(delta),
                    falloff * falloff / (1.0 + distance_squared),
                )
            }
        };
        let nl = normal.dot(direction).max(0.0);
        if nl <= 0.0 {
            continue;
        }
        let radiance = light.color * light.intensity * attenuation;
        let half = crate::normalize_direction(view + direction);
        let nh = normal.dot(half).max(0.0);
        let f0 = Vec3::splat(0.04).lerp(albedo, material.metallic);
        let reflected = match environment.mode {
            LightingMode::Pbr => {
                let nv = normal.dot(view).max(0.0001);
                let vh = view.dot(half).max(0.0);
                let a2 = material.roughness.powi(4);
                let denominator = nh * nh * (a2 - 1.0) + 1.0;
                let distribution =
                    a2 / (std::f32::consts::PI * denominator * denominator).max(1e-12);
                let k = (material.roughness + 1.0).powi(2) / 8.0;
                let visibility = (nv / (nv * (1.0 - k) + k)) * (nl / (nl * (1.0 - k) + k));
                let fresnel = f0 + (Vec3::ONE - f0) * (1.0 - vh).powi(5);
                let diffuse = (Vec3::ONE - fresnel) * (1.0 - material.metallic) * albedo
                    / std::f32::consts::PI;
                diffuse + fresnel * (distribution * visibility / (4.0 * nv * nl).max(0.0001))
            }
            LightingMode::BlinnPhong => {
                let power =
                    (2.0 / (material.roughness * material.roughness) - 2.0).clamp(1.0, 512.0);
                albedo * (1.0 - material.metallic) + f0 * nh.powf(power) * ((power + 2.0) / 8.0)
            }
            _ => albedo,
        };
        result += reflected * radiance * nl;
    }
    result
        .clamp(Vec3::ZERO, Vec3::ONE)
        .extend(base.w.clamp(0.0, 1.0))
}

pub fn material_fixture() -> crate::PreparedScene {
    use crate::{
        LodLevel, MaterialHandle, MeshHandle, PreparedDraw, PreparedScene, RasterMesh,
        SnapshotNode, Transform, VisibilityClass,
    };
    use std::sync::Arc;
    let mesh = Arc::new(RasterMesh::cube(Vec3::ZERO, 1.0, Vec4::ONE));
    let mut draws: Vec<_> = (0..3)
        .map(|index| PreparedDraw {
            node: SnapshotNode::destination(
                index,
                Transform::from_translation(Vec3::new((index as f32 - 1.0) * 1.4, 0.0, -5.0)),
                MeshHandle::CUBE,
                MaterialHandle::new(200 + index),
                true,
            ),
            mesh: mesh.clone(),
            lod: LodLevel::High,
            visibility: VisibilityClass::Inside,
            projected_size_px: 64.0,
        })
        .collect();
    for (index, draw) in draws.iter_mut().enumerate() {
        draw.node.transform.rotation = if index == 0 {
            crate::Quat::from_rotation_y(0.32) * crate::Quat::from_rotation_x(-0.21)
        } else {
            crate::Quat::from_rotation_y(-0.45) * crate::Quat::from_rotation_x(0.25)
        };
    }
    let mut scene = PreparedScene::from_draws(draws);
    scene.lighting.mode = LightingMode::BlinnPhong;
    scene.lighting.set_lights([
        RenderLight {
            color: Vec3::ONE,
            intensity: 0.8,
            kind: RenderLightKind::Directional {
                direction: Vec3::new(-0.4, -0.5, -1.0),
            },
            casts_shadow: false,
        },
        RenderLight {
            color: Vec3::new(1.0, 0.8, 0.6),
            intensity: 12.0,
            kind: RenderLightKind::Point {
                position: Vec3::new(-2.0, 2.0, -2.0),
                range: 12.0,
            },
            casts_shadow: false,
        },
    ]);
    for (index, material) in [
        MaterialInfo::fixture_metal(),
        MaterialInfo::fixture_rough(),
        MaterialInfo::fixture_emissive(),
    ]
    .into_iter()
    .enumerate()
    {
        scene.set_material(MaterialHandle::new(200 + index as u32), material);
    }
    scene
}
