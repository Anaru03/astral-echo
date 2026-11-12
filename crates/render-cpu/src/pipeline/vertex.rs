use glam::{Mat4, Vec3, Vec4};
use std::time::Instant;

use crate::clip::ClipVertex;
use crate::error::RasterError;
use crate::geometry::RasterVertex;
use crate::pipeline::stages::StageTimings;

pub(super) struct VertexLighting<'a> {
    pub model: Mat4,
    pub normal_matrix: Mat4,
    pub normals: &'a [Vec3],
    pub material: render_core::MaterialInfo,
    pub environment: &'a render_core::LightingEnvironment,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VertexStage {
    mvp: Mat4,
}

impl VertexStage {
    pub fn new(mvp: Mat4) -> Self {
        Self { mvp }
    }

    pub fn mvp(&self) -> Mat4 {
        self.mvp
    }

    pub fn transform(&self, vertex: &RasterVertex) -> Result<ClipVertex, RasterError> {
        if !vertex.position.is_finite() {
            return Err(RasterError::NonFiniteVertex);
        }
        if !vertex.color.is_finite() {
            return Err(RasterError::NonFiniteVertex);
        }
        let clip =
            self.mvp * Vec4::new(vertex.position.x, vertex.position.y, vertex.position.z, 1.0);
        if !clip.is_finite() {
            return Err(RasterError::NonFiniteVertex);
        }
        Ok(ClipVertex::new(clip, vertex.color))
    }

    pub(super) fn transform_lit_into(
        &self,
        vertices: &[RasterVertex],
        lighting: &VertexLighting<'_>,
        out: &mut Vec<ClipVertex>,
    ) -> Result<(), RasterError> {
        self.transform_lit_into_profiled(vertices, lighting, out, None)
    }

    pub(super) fn transform_lit_into_profiled(
        &self,
        vertices: &[RasterVertex],
        lighting: &VertexLighting<'_>,
        out: &mut Vec<ClipVertex>,
        mut timings: Option<&mut StageTimings>,
    ) -> Result<(), RasterError> {
        if !lighting.environment.camera_position.is_finite()
            || !lighting.environment.ambient.is_finite()
        {
            return Err(RasterError::NonFiniteVertex);
        }
        out.clear();
        out.reserve(vertices.len());
        for (vertex, normal) in vertices.iter().zip(lighting.normals) {
            let started = timings.as_ref().map(|_| Instant::now());
            let mut transformed = self.transform(vertex)?;
            if let (Some(timing), Some(started)) = (timings.as_deref_mut(), started) {
                timing.vertex_clip_ns += started.elapsed().as_nanos() as u64;
            }
            let started = timings.as_ref().map(|_| Instant::now());
            let position = lighting.model.transform_point3(vertex.position);
            if let (Some(timing), Some(started)) = (timings.as_deref_mut(), started) {
                timing.vertex_world_ns += started.elapsed().as_nanos() as u64;
            }
            let started = timings.as_ref().map(|_| Instant::now());
            let normal =
                render_core::normalize_direction(lighting.normal_matrix.transform_vector3(*normal));
            if let (Some(timing), Some(started)) = (timings.as_deref_mut(), started) {
                timing.vertex_normal_ns += started.elapsed().as_nanos() as u64;
            }
            let started = timings.as_ref().map(|_| Instant::now());
            if let Some(timing) = timings.as_deref_mut() {
                let (shaded, shade_timings) =
                    render_core::shade_surface_profiled_sanitized_normalized(
                        lighting.environment,
                        lighting.material,
                        vertex.color,
                        position,
                        normal,
                    );
                transformed.color = shaded;
                timing.shade.merge(shade_timings);
            } else {
                transformed.color = render_core::shade_surface_sanitized_normalized(
                    lighting.environment,
                    lighting.material,
                    vertex.color,
                    position,
                    normal,
                );
            }
            if let (Some(timing), Some(started)) = (timings.as_deref_mut(), started) {
                timing.vertex_shading_ns += started.elapsed().as_nanos() as u64;
            }
            let started = timings.as_ref().map(|_| Instant::now());
            out.push(transformed);
            if let (Some(timing), Some(started)) = (timings.as_deref_mut(), started) {
                timing.vertex_write_ns += started.elapsed().as_nanos() as u64;
            }
        }
        Ok(())
    }

    pub fn transform_into(
        &self,
        vertices: &[RasterVertex],
        out: &mut Vec<ClipVertex>,
    ) -> Result<(), RasterError> {
        out.clear();
        out.reserve(vertices.len());
        for vertex in vertices {
            out.push(self.transform(vertex)?);
        }
        Ok(())
    }

    pub(super) fn transform_into_profiled(
        &self,
        vertices: &[RasterVertex],
        out: &mut Vec<ClipVertex>,
        timing: &mut StageTimings,
    ) -> Result<(), RasterError> {
        out.clear();
        out.reserve(vertices.len());
        for vertex in vertices {
            let started = Instant::now();
            let transformed = self.transform(vertex)?;
            timing.vertex_clip_ns += started.elapsed().as_nanos() as u64;
            let started = Instant::now();
            out.push(transformed);
            timing.vertex_write_ns += started.elapsed().as_nanos() as u64;
        }
        Ok(())
    }
}

pub fn model_to_world(position: Vec3, model: &Mat4) -> Vec3 {
    model.transform_point3(position)
}

pub fn world_to_view(position: Vec3, view: &Mat4) -> Vec3 {
    view.transform_point3(position)
}

pub fn model_to_clip(position: Vec3, mvp: &Mat4) -> Vec4 {
    mvp * Vec4::new(position.x, position.y, position.z, 1.0)
}
