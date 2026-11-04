use glam::Vec4;

use crate::error::RasterError;

pub const NEAR_EPS: f32 = 1e-6;
pub const PLANE_EPS: f32 = 1e-6;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ClipMetrics {
    /// Triángulos recibidos por la etapa de clipping.
    pub triangles_received: u32,
    /// Triángulos completamente fuera de al menos un plano.
    pub trivial_rejects: u32,
    /// Triángulos aceptados sin ejecutar clipping completo.
    pub trivial_accepts: u32,
    /// Llamadas efectivas a clipping completo desde la ruta rápida.
    pub full_clip_calls: u32,
    /// Evaluaciones reales de distancia vértice-plano, incluidas las repetidas.
    pub plane_distance_evaluations: u32,
    /// Intersecciones arista-plano calculadas durante clipping.
    pub plane_intersections: u32,
    /// Vértices creados mediante interpolación durante clipping.
    pub interpolated_vertices: u32,
    /// Suma de vértices de los polígonos resultantes antes de triangularlos.
    pub final_polygon_vertices: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClipVertex {
    pub clip: Vec4,
    pub color: Vec4,
}

impl ClipVertex {
    pub const fn new(clip: Vec4, color: Vec4) -> Self {
        Self { clip, color }
    }

    pub fn is_finite(&self) -> bool {
        self.clip.is_finite() && self.color.is_finite()
    }

    pub fn is_in_front(&self) -> bool {
        self.clip.w > NEAR_EPS
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClipPlane {
    Left,
    Right,
    Bottom,
    Top,
    Near,
    Far,
}

impl ClipPlane {
    pub const ALL: [ClipPlane; 6] = [
        ClipPlane::Left,
        ClipPlane::Right,
        ClipPlane::Bottom,
        ClipPlane::Top,
        ClipPlane::Near,
        ClipPlane::Far,
    ];

    /// Planos laterales del frustum (sin near ni far).
    pub const LATERAL: [ClipPlane; 4] = [
        ClipPlane::Left,
        ClipPlane::Right,
        ClipPlane::Bottom,
        ClipPlane::Top,
    ];

    pub fn distance(self, clip: &Vec4) -> f32 {
        match self {
            ClipPlane::Left => clip.x + clip.w,
            ClipPlane::Right => clip.w - clip.x,
            ClipPlane::Bottom => clip.y + clip.w,
            ClipPlane::Top => clip.w - clip.y,
            ClipPlane::Near => clip.z,
            ClipPlane::Far => clip.w - clip.z,
        }
    }
}

fn interpolate(a: &ClipVertex, b: &ClipVertex, t: f32) -> ClipVertex {
    ClipVertex::new(
        a.clip + (b.clip - a.clip) * t,
        a.color + (b.color - a.color) * t,
    )
}

fn clip_against_plane(
    polygon: &[ClipVertex],
    plane: ClipPlane,
    out: &mut Vec<ClipVertex>,
    mut metrics: Option<&mut ClipMetrics>,
) {
    out.clear();
    if polygon.is_empty() {
        return;
    }
    let len = polygon.len();
    for i in 0..len {
        let current = polygon[i];
        let next = polygon[(i + 1) % len];
        let d_current = plane.distance(&current.clip);
        let d_next = plane.distance(&next.clip);
        if let Some(metrics) = metrics.as_deref_mut() {
            metrics.plane_distance_evaluations += 2;
        }
        let current_inside = d_current >= 0.0;
        let next_inside = d_next >= 0.0;
        if current_inside {
            out.push(current);
        }
        if current_inside != next_inside {
            let denom = d_current - d_next;
            if denom.abs() > f32::EPSILON {
                let t = (d_current / denom).clamp(0.0, 1.0);
                if let Some(metrics) = metrics.as_deref_mut() {
                    metrics.plane_intersections += 1;
                    metrics.interpolated_vertices += 1;
                }
                out.push(interpolate(&current, &next, t));
            }
        }
    }
}

pub fn clip_polygon(polygon: &[ClipVertex]) -> Result<Vec<ClipVertex>, RasterError> {
    clip_polygon_with_metrics(polygon, None)
}

fn clip_polygon_with_metrics(
    polygon: &[ClipVertex],
    mut metrics: Option<&mut ClipMetrics>,
) -> Result<Vec<ClipVertex>, RasterError> {
    if polygon.len() < 3 {
        return Ok(Vec::new());
    }
    for v in polygon {
        if !v.is_finite() {
            return Err(RasterError::InvalidClip);
        }
    }
    let mut current: Vec<ClipVertex> = polygon.to_vec();
    let mut scratch: Vec<ClipVertex> = Vec::with_capacity(polygon.len() + 4);
    for plane in ClipPlane::ALL {
        clip_against_plane(&current, plane, &mut scratch, metrics.as_deref_mut());
        std::mem::swap(&mut current, &mut scratch);
        if current.len() < 3 {
            return Ok(Vec::new());
        }
    }
    for v in &current {
        if !v.is_finite() {
            return Err(RasterError::InvalidClip);
        }
    }
    if let Some(metrics) = metrics.as_deref_mut() {
        metrics.final_polygon_vertices += current.len() as u32;
    }
    Ok(current)
}

pub fn clip_polygon_near(polygon: &[ClipVertex]) -> Result<Vec<ClipVertex>, RasterError> {
    if polygon.len() < 3 {
        return Ok(Vec::new());
    }
    for v in polygon {
        if !v.is_finite() {
            return Err(RasterError::InvalidClip);
        }
    }
    let mut output: Vec<ClipVertex> = Vec::with_capacity(polygon.len() + 1);
    for i in 0..polygon.len() {
        let current = polygon[i];
        let next = polygon[(i + 1) % polygon.len()];
        let current_in = current.is_in_front();
        let next_in = next.is_in_front();
        if current_in {
            output.push(current);
        }
        if current_in != next_in {
            let dc = current.clip.w - NEAR_EPS;
            let dn = next.clip.w - NEAR_EPS;
            let denom = dc - dn;
            if denom.abs() > f32::EPSILON {
                let t = dc / denom;
                let t = t.clamp(0.0, 1.0);
                output.push(interpolate(&current, &next, t));
            }
        }
    }
    Ok(output)
}

pub fn clip_triangle(
    a: ClipVertex,
    b: ClipVertex,
    c: ClipVertex,
) -> Result<Vec<ClipVertex>, RasterError> {
    let polygon = [a, b, c];
    clip_polygon(&polygon)
}

pub(crate) fn clip_triangle_profiled(
    a: ClipVertex,
    b: ClipVertex,
    c: ClipVertex,
    metrics: &mut ClipMetrics,
) -> Result<Vec<ClipVertex>, RasterError> {
    clip_polygon_with_metrics(&[a, b, c], Some(metrics))
}

pub fn clip_triangle_fast(
    a: ClipVertex,
    b: ClipVertex,
    c: ClipVertex,
) -> Result<Vec<ClipVertex>, RasterError> {
    clip_triangle_fast_impl(a, b, c, None)
}

pub(crate) fn clip_triangle_fast_profiled(
    a: ClipVertex,
    b: ClipVertex,
    c: ClipVertex,
    metrics: &mut ClipMetrics,
) -> Result<Vec<ClipVertex>, RasterError> {
    clip_triangle_fast_impl(a, b, c, Some(metrics))
}

fn clip_triangle_fast_impl(
    a: ClipVertex,
    b: ClipVertex,
    c: ClipVertex,
    mut metrics: Option<&mut ClipMetrics>,
) -> Result<Vec<ClipVertex>, RasterError> {
    if let Some(metrics) = metrics.as_deref_mut() {
        metrics.triangles_received += 1;
    }
    if !a.is_finite() || !b.is_finite() || !c.is_finite() {
        return Err(RasterError::InvalidClip);
    }
    let mut distances_by_plane = [[0.0_f32; 3]; ClipPlane::ALL.len()];
    for (plane_index, plane) in ClipPlane::ALL.into_iter().enumerate() {
        let distances = [
            plane.distance(&a.clip),
            plane.distance(&b.clip),
            plane.distance(&c.clip),
        ];
        distances_by_plane[plane_index] = distances;
        if let Some(metrics) = metrics.as_deref_mut() {
            metrics.plane_distance_evaluations += 3;
        }
        if distances.iter().all(|distance| *distance < 0.0) {
            if let Some(metrics) = metrics.as_deref_mut() {
                metrics.trivial_rejects += 1;
            }
            return Ok(Vec::new());
        }
        if distances.iter().any(|distance| !distance.is_finite()) {
            return Err(RasterError::InvalidClip);
        }
    }
    let mut trivially_inside = true;
    for distances in distances_by_plane {
        for distance in distances {
            if distance < 0.0 {
                trivially_inside = false;
                break;
            }
        }
        if !trivially_inside {
            break;
        }
    }
    if trivially_inside {
        if let Some(metrics) = metrics.as_deref_mut() {
            metrics.trivial_accepts += 1;
            metrics.final_polygon_vertices += 3;
        }
        return Ok(vec![a, b, c]);
    }
    if let Some(metrics) = metrics.as_deref_mut() {
        metrics.full_clip_calls += 1;
    }
    clip_polygon_with_metrics(&[a, b, c], metrics)
}

pub fn clip_triangle_near(
    a: ClipVertex,
    b: ClipVertex,
    c: ClipVertex,
) -> Result<Vec<ClipVertex>, RasterError> {
    let polygon = [a, b, c];
    clip_polygon_near(&polygon)
}