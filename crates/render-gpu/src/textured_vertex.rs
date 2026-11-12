use bytemuck::{Pod, Zeroable};
use render_core::RasterVertex;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct TexturedVertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
    pub uv: [f32; 2],
}

impl From<&RasterVertex> for TexturedVertex {
    fn from(vertex: &RasterVertex) -> Self {
        Self {
            position: vertex.position.to_array(),
            color: vertex.color.to_array(),
            uv: vertex.uv.to_array(),
        }
    }
}

impl TexturedVertex {
    pub const ATTRIBUTES: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![
        0 => Float32x3,
        1 => Float32x4,
        2 => Float32x2
    ];

    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use render_core::{Vec2, Vec3, Vec4};

    #[test]
    fn textured_vertex_preserves_position_color_and_uv() {
        let source =
            RasterVertex::new(Vec3::new(1.0, 2.0, 3.0), Vec4::ONE).with_uv(Vec2::new(0.25, 0.75));

        let result = TexturedVertex::from(&source);

        assert_eq!(result.position, [1.0, 2.0, 3.0]);
        assert_eq!(result.color, [1.0; 4]);
        assert_eq!(result.uv, [0.25, 0.75]);
    }

    #[test]
    fn textured_vertex_layout_has_correct_attributes() {
        let layout = TexturedVertex::layout();

        assert_eq!(layout.array_stride, 36);
        assert_eq!(layout.attributes.len(), 3);

        assert_eq!(layout.attributes[0].shader_location, 0);
        assert_eq!(layout.attributes[1].shader_location, 1);
        assert_eq!(layout.attributes[2].shader_location, 2);

        assert_eq!(layout.attributes[0].offset, 0);
        assert_eq!(layout.attributes[1].offset, 12);
        assert_eq!(layout.attributes[2].offset, 28);
    }
}
