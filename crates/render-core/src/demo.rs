use crate::{Camera, CameraError, Projection, Transform};
use glam::{Vec3, Vec4};

use crate::geometry::{RasterMesh, RasterVertex};

/// Número de divisiones de la malla base de la escena de prueba.
pub const GRID_SUBDIVISIONS: u32 = 8;

/// Radio del terreno de la escena de prueba.
pub const TERRAIN_EXTENT: f32 = 22.0;

/// Altura del domo del terreno en el centro.
pub const TERRAIN_HEIGHT: f32 = 3.0;

/// Altura de la superficie del terreno en `(x, z)`.
pub fn terrain_height(x: f32, z: f32) -> f32 {
    let falloff = 1.0 - (x * x + z * z) / (TERRAIN_EXTENT * TERRAIN_EXTENT);
    TERRAIN_HEIGHT * falloff.max(0.0)
}

/// Color de un vértice en función de su altura normalizada.
fn height_color(t: f32) -> Vec4 {
    let t = t.clamp(0.0, 1.0);
    Vec4::new(
        0.15 + 0.75 * t,
        0.30 + 0.45 * (1.0 - t),
        0.85 - 0.55 * t,
        1.0,
    )
}

/// Crea un terreno en cuadrícula con colores por altura.
pub fn grid_mesh(subdivisions: u32, extent: f32, height: f32) -> RasterMesh {
    let subdivisions = subdivisions.max(1);
    let step = extent * 2.0 / subdivisions as f32;
    let mut vertices = Vec::with_capacity(((subdivisions + 1) * (subdivisions + 1)) as usize);
    let mut indices = Vec::with_capacity((subdivisions * subdivisions * 6) as usize);
    for iy in 0..=subdivisions {
        for ix in 0..=subdivisions {
            let x = -extent + step * ix as f32;
            let z = -extent + step * iy as f32;
            let h = height * (1.0 - (x * x + z * z) / (extent * extent));
            vertices.push(RasterVertex::new(
                Vec3::new(x, h, z),
                height_color((h / height.max(f32::EPSILON) + 1.0) * 0.5),
            ));
        }
    }
    let stride = subdivisions + 1;
    for iy in 0..subdivisions {
        for ix in 0..subdivisions {
            let base = iy * stride + ix;
            indices.extend([
                base,
                base + stride,
                base + 1,
                base + 1,
                base + stride,
                base + stride + 1,
            ]);
        }
    }
    RasterMesh::new(vertices, indices)
}

/// Construye la escena de referencia: terreno, cubos y triángulos grandes.
pub fn demo_scene() -> Vec<(RasterMesh, Transform)> {
    let mut scene = Vec::new();
    scene.push((
        grid_mesh(GRID_SUBDIVISIONS, TERRAIN_EXTENT, TERRAIN_HEIGHT),
        Transform::from_translation(Vec3::new(0.0, -1.0, 0.0)),
    ));
    for i in 0..12 {
        let angle = i as f32 * 0.52;
        let radius = 6.0 + (i % 4) as f32 * 2.5;
        let x = angle.cos() * radius;
        let z = angle.sin() * radius;
        // Los cubos se apoyan sobre la superficie del domo.
        let y = terrain_height(x, z) - 1.0 + 0.8;
        let position = Vec3::new(x, y, z);
        scene.push((
            RasterMesh::cube(Vec3::ZERO, 1.6, Vec4::new(0.35, 0.72, 1.0, 1.0)),
            Transform::new(
                position,
                glam::Quat::from_rotation_y(angle * 1.3),
                Vec3::ONE,
            ),
        ));
    }
    for i in 0..6 {
        let angle = i as f32 * 1.05;
        let position = Vec3::new(angle.cos() * 14.0, 6.0, angle.sin() * 14.0);
        scene.push((
            RasterMesh::triangle(
                RasterVertex::new(Vec3::new(-2.5, -2.0, 0.0), Vec4::new(1.0, 0.3, 0.2, 1.0)),
                RasterVertex::new(Vec3::new(2.5, -2.0, 0.0), Vec4::new(0.2, 1.0, 0.4, 1.0)),
                RasterVertex::new(Vec3::new(0.0, 2.5, 0.0), Vec4::new(0.4, 0.5, 1.0, 1.0)),
            ),
            Transform::from_translation(position),
        ));
    }
    scene
}

/// Cámara de la escena de referencia.
///
/// Encuadra el domo de terreno y los objetos colocados sobre él desde una
/// posición elevada, siguiendo la propuesta de `docs/graphics/01-performance-baseline.md`.
pub fn demo_camera(aspect: f32) -> Result<Camera, CameraError> {
    Camera::new(
        Vec3::new(0.0, 26.0, 46.0),
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::Y,
        Projection::Perspective {
            fov_y_rad: 60.0_f32.to_radians(),
            aspect,
            near: 0.1,
            far: 500.0,
        },
    )
}
