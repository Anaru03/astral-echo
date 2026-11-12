use crate::{Mat4, Vec3};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RigidSkinVertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub bone: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CpuPose {
    pub skin_matrices: Vec<Mat4>,
}

impl CpuPose {
    pub fn new(skin_matrices: Vec<Mat4>) -> Self {
        Self { skin_matrices }
    }

    pub fn skin(&self, vertex: RigidSkinVertex) -> Option<(Vec3, Vec3)> {
        let matrix = self.skin_matrices.get(vertex.bone)?;
        let position = matrix.transform_point3(vertex.position);
        let normal = crate::normalize_direction(matrix.transform_vector3(vertex.normal));
        position.is_finite().then_some((position, normal))
    }
}

pub fn skin_vertices(vertices: &[RigidSkinVertex], pose: &CpuPose) -> Vec<(Vec3, Vec3)> {
    vertices
        .iter()
        .filter_map(|vertex| pose.skin(*vertex))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rigid_pose_moves_vertices_and_normalizes_normals() {
        let vertices = [RigidSkinVertex {
            position: Vec3::ZERO,
            normal: Vec3::X,
            bone: 0,
        }];
        let pose = CpuPose::new(vec![Mat4::from_translation(Vec3::new(0.0, 0.5, 0.0))]);
        let bind = CpuPose::new(vec![Mat4::IDENTITY])
            .skin(vertices[0])
            .unwrap();
        let posed = pose.skin(vertices[0]).unwrap();
        assert_ne!(bind.0, posed.0);
        assert!((posed.1.length() - 1.0).abs() < 1e-5);
    }
}
