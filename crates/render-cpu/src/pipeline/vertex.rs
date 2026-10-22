use glam::{Mat4, Vec3, Vec4};

use crate::clip::ClipVertex;
use crate::error::RasterError;
use crate::geometry::RasterVertex;

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
        if !lighting.environment.camera_position.is_finite()
            || !lighting.environment.ambient.is_finite()
        {
            return Err(RasterError::NonFiniteVertex);
        }
        out.clear();
        out.reserve(vertices.len());
        for (vertex, normal) in vertices.iter().zip(lighting.normals) {
            let mut transformed = self.transform(vertex)?;
            let position = lighting.model.transform_point3(vertex.position);
            let normal =
                render_core::normalize_direction(lighting.normal_matrix.transform_vector3(*normal));
            transformed.color = render_core::shade_surface(
                lighting.environment,
                lighting.material,
                vertex.color,
                position,
                normal,
            );
            out.push(transformed);
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
