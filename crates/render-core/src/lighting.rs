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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ShadeTimings {
    pub calls: u64,
    pub light_calls: u64,
    pub directional_lights: u64,
    pub point_lights: u64,
    pub point_distance_calculations: u64,
    pub point_normalizations: u64,
    pub inputs_ns: u64,
    pub normal_ns: u64,
    pub view_ns: u64,
    pub light_direction_ns: u64,
    pub attenuation_ns: u64,
    pub diffuse_ns: u64,
    pub specular_ns: u64,
    pub ambient_ns: u64,
    pub composition_ns: u64,
}

impl ShadeTimings {
    pub fn merge(&mut self, other: Self) {
        self.calls += other.calls;
        self.light_calls += other.light_calls;
        self.directional_lights += other.directional_lights;
        self.point_lights += other.point_lights;
        self.point_distance_calculations += other.point_distance_calculations;
        self.point_normalizations += other.point_normalizations;
        self.inputs_ns += other.inputs_ns;
        self.normal_ns += other.normal_ns;
        self.view_ns += other.view_ns;
        self.light_direction_ns += other.light_direction_ns;
        self.attenuation_ns += other.attenuation_ns;
        self.diffuse_ns += other.diffuse_ns;
        self.specular_ns += other.specular_ns;
        self.ambient_ns += other.ambient_ns;
        self.composition_ns += other.composition_ns;
    }
}

pub fn shade_surface(
    environment: &LightingEnvironment,
    material: MaterialInfo,
    color: Vec4,
    position: Vec3,
    normal: Vec3,
) -> Vec4 {
    shade_surface_sanitized(environment, material.sanitized(), color, position, normal)
}

pub fn shade_surface_sanitized(
    environment: &LightingEnvironment,
    material: MaterialInfo,
    color: Vec4,
    position: Vec3,
    normal: Vec3,
) -> Vec4 {
    shade_surface_impl(environment, material, color, position, normal, None, false)
}

/// Internal fast path for callers that have already normalized `normal`.
/// The public `shade_surface` path remains defensive and normalizes its input.
pub fn shade_surface_sanitized_normalized(
    environment: &LightingEnvironment,
    material: MaterialInfo,
    color: Vec4,
    position: Vec3,
    normal: Vec3,
) -> Vec4 {
    shade_surface_impl(
        environment,
        material,
        color,
        position,
        normal,
        None,
        true,
    )
}

pub fn shade_surface_profiled(
    environment: &LightingEnvironment,
    material: MaterialInfo,
    color: Vec4,
    position: Vec3,
    normal: Vec3,
) -> (Vec4, ShadeTimings) {
    shade_surface_profiled_sanitized(
        environment,
        material.sanitized(),
        color,
        position,
        normal,
    )
}

pub fn shade_surface_profiled_sanitized(
    environment: &LightingEnvironment,
    material: MaterialInfo,
    color: Vec4,
    position: Vec3,
    normal: Vec3,
) -> (Vec4, ShadeTimings) {
    let mut timings = ShadeTimings {
        calls: 1,
        ..Default::default()
    };
    let result = shade_surface_impl(
        environment,
        material,
        color,
        position,
        normal,
        Some(&mut timings),
        false,
    );
    (result, timings)
}

pub fn shade_surface_profiled_sanitized_normalized(
    environment: &LightingEnvironment,
    material: MaterialInfo,
    color: Vec4,
    position: Vec3,
    normal: Vec3,
) -> (Vec4, ShadeTimings) {
    let mut timings = ShadeTimings {
        calls: 1,
        ..Default::default()
    };
    let result = shade_surface_impl(
        environment,
        material,
        color,
        position,
        normal,
        Some(&mut timings),
        true,
    );
    (result, timings)
}

fn shade_surface_impl(
    environment: &LightingEnvironment,
    material: MaterialInfo,
    color: Vec4,
    position: Vec3,
    normal: Vec3,
    mut timings: Option<&mut ShadeTimings>,
    normal_is_normalized: bool,
) -> Vec4 {
    let started = timings.as_ref().map(|_| std::time::Instant::now());
    if environment.mode == LightingMode::Unlit {
        return color;
    }
    let base = color * material.base_color;
    let albedo = base.truncate().max(Vec3::ZERO);
    if let (Some(timing), Some(started)) = (timings.as_deref_mut(), started) {
        timing.inputs_ns += started.elapsed().as_nanos() as u64;
    }
    let started = timings.as_ref().map(|_| std::time::Instant::now());
    let normal = if normal_is_normalized {
        normal
    } else {
        crate::normalize_direction(normal)
    };
    if let (Some(timing), Some(started)) = (timings.as_deref_mut(), started) {
        timing.normal_ns += started.elapsed().as_nanos() as u64;
    }
    let view = match environment.mode {
        LightingMode::BlinnPhong | LightingMode::Pbr => {
            let started = timings.as_ref().map(|_| std::time::Instant::now());
            let view = crate::normalize_direction(environment.camera_position - position);
            if let (Some(timing), Some(started)) = (timings.as_deref_mut(), started) {
                timing.view_ns += started.elapsed().as_nanos() as u64;
            }
            view
        }
        LightingMode::Unlit | LightingMode::Lambert => Vec3::ZERO,
    };
    let started = timings.as_ref().map(|_| std::time::Instant::now());
    let mut result = albedo * environment.ambient.max(Vec3::ZERO) + material.emissive;
    if let (Some(timing), Some(started)) = (timings.as_deref_mut(), started) {
        timing.ambient_ns += started.elapsed().as_nanos() as u64;
    }
    for light in environment.lights() {
        if let Some(timing) = timings.as_deref_mut() {
            timing.light_calls += 1;
        }
        let started = timings.as_ref().map(|_| std::time::Instant::now());
        let (direction, attenuation) = match light.kind {
            RenderLightKind::Directional { direction } => {
                if let Some(timing) = timings.as_deref_mut() {
                    timing.directional_lights += 1;
                }
                (-direction, 1.0)
            }
            RenderLightKind::Point {
                position: light_position,
                range,
            } => {
                if let Some(timing) = timings.as_deref_mut() {
                    timing.point_lights += 1;
                    timing.point_distance_calculations += 1;
                    timing.point_normalizations += 1;
                }
                let delta = light_position - position;
                let distance_squared = delta.length_squared();
                let falloff = (1.0 - distance_squared / (range * range)).clamp(0.0, 1.0);
                (
                    crate::normalize_direction(delta),
                    falloff * falloff / (1.0 + distance_squared),
                )
            }
        };
        if let (Some(timing), Some(started)) = (timings.as_deref_mut(), started) {
            timing.light_direction_ns += started.elapsed().as_nanos() as u64;
        }
        let started = timings.as_ref().map(|_| std::time::Instant::now());
        let attenuation = attenuation;
        if let (Some(timing), Some(started)) = (timings.as_deref_mut(), started) {
            timing.attenuation_ns += started.elapsed().as_nanos() as u64;
        }
        let started = timings.as_ref().map(|_| std::time::Instant::now());
        let nl = normal.dot(direction).max(0.0);
        if nl <= 0.0 {
            continue;
        }
        if let (Some(timing), Some(started)) = (timings.as_deref_mut(), started) {
            timing.diffuse_ns += started.elapsed().as_nanos() as u64;
        }
        let radiance = light.color * light.intensity * attenuation;
        let started = timings.as_ref().map(|_| std::time::Instant::now());
        let reflected = match environment.mode {
            LightingMode::Pbr => {
                let half = crate::normalize_direction(view + direction);
                let nh = normal.dot(half).max(0.0);
                let f0 = Vec3::splat(0.04).lerp(albedo, material.metallic);
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
                let half = crate::normalize_direction(view + direction);
                let nh = normal.dot(half).max(0.0);
                let f0 = Vec3::splat(0.04).lerp(albedo, material.metallic);
                let power =
                    (2.0 / (material.roughness * material.roughness) - 2.0).clamp(1.0, 512.0);
                albedo * (1.0 - material.metallic) + f0 * nh.powf(power) * ((power + 2.0) / 8.0)
            }
            _ => albedo,
        };
        if let (Some(timing), Some(started)) = (timings.as_deref_mut(), started) {
            timing.specular_ns += started.elapsed().as_nanos() as u64;
        }
        let started = timings.as_ref().map(|_| std::time::Instant::now());
        result += reflected * radiance * nl;
        if let (Some(timing), Some(started)) = (timings.as_deref_mut(), started) {
            timing.composition_ns += started.elapsed().as_nanos() as u64;
        }
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