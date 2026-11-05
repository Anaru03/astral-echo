use std::fmt;
use std::io::BufRead;
use std::path::Path;
use glam::{Vec3, Vec4};
use crate::{RasterMesh, RasterVertex};

#[derive(Debug)]
pub enum ObjLoadError {
    Io(std::io::Error),
    Parse(tobj::LoadError),
    Empty,
    Invalid(String),
}

impl fmt::Display for ObjLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "failed to read OBJ: {error}"),
            Self::Parse(error) => write!(f, "failed to parse OBJ: {error:?}"),
            Self::Empty => write!(f, "OBJ contains no renderable geometry"),
            Self::Invalid(message) => write!(f, "invalid OBJ geometry: {message}"),
        }
    }
}

impl std::error::Error for ObjLoadError {}

impl From<std::io::Error> for ObjLoadError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

pub fn load_obj(path: impl AsRef<Path>) -> Result<RasterMesh, ObjLoadError> {
    let (models, _materials) = tobj::load_obj(
        path.as_ref(),
        &tobj::LoadOptions {
            triangulate: true,
            single_index: true,
            ignore_points: true,
            ignore_lines: true,
            ..Default::default()
        },
    )
    .map_err(ObjLoadError::Parse)?;
    build_mesh(models.into_iter().map(|model| model.mesh))
}

pub fn load_obj_reader<R: BufRead>(reader: &mut R) -> Result<RasterMesh, ObjLoadError> {
    let (models, _materials) = tobj::load_obj_buf(
        reader,
        &tobj::LoadOptions {
            triangulate: true,
            single_index: true,
            ignore_points: true,
            ignore_lines: true,
            ..Default::default()
        },
        |_| Ok((Vec::new(), std::collections::HashMap::new())),
    )
    .map_err(ObjLoadError::Parse)?;
    build_mesh(models.into_iter().map(|model| model.mesh))
}

fn build_mesh(meshes: impl IntoIterator<Item = tobj::Mesh>) -> Result<RasterMesh, ObjLoadError> {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let color = Vec4::ONE;

    for mesh in meshes {
        if mesh.positions.len() % 3 != 0 {
            return Err(ObjLoadError::Invalid(
                "positions are not groups of three".into(),
            ));
        }
        if mesh.indices.is_empty() || mesh.indices.len() % 3 != 0 {
            return Err(ObjLoadError::Invalid(
                "indices are not a non-empty triangle list".into(),
            ));
        }
        let vertex_count = mesh.positions.len() / 3;
        if mesh.normals.is_empty() && !mesh.normal_indices.is_empty() {
            return Err(ObjLoadError::Invalid(
                "normal indices exist without normal data".into(),
            ));
        }
        if !mesh.normals.is_empty() && mesh.normals.len() != vertex_count * 3 {
            return Err(ObjLoadError::Invalid(
                "single-index normals do not match positions".into(),
            ));
        }

        let base = vertices.len() as u32;
        for index in 0..vertex_count {
            let position = Vec3::new(
                mesh.positions[index * 3],
                mesh.positions[index * 3 + 1],
                mesh.positions[index * 3 + 2],
            );
            if !position.is_finite() {
                return Err(ObjLoadError::Invalid("non-finite position".into()));
            }
            let normal = if mesh.normals.is_empty() {
                Vec3::ZERO
            } else {
                Vec3::new(
                    mesh.normals[index * 3],
                    mesh.normals[index * 3 + 1],
                    mesh.normals[index * 3 + 2],
                )
            };
            if !normal.is_finite() {
                return Err(ObjLoadError::Invalid("non-finite normal".into()));
            }
            vertices.push(RasterVertex::new(position, color).with_normal(normal));
        }
        for index in mesh.indices {
            if index as usize >= vertex_count {
                return Err(ObjLoadError::Invalid("index outside vertex data".into()));
            }
            indices.push(base + index);
        }
    }

    if vertices.is_empty() || indices.is_empty() {
        return Err(ObjLoadError::Empty);
    }
    let mut result = RasterMesh::new(vertices, indices);
    if result
        .vertices
        .iter()
        .any(|vertex| !vertex.position.is_finite())
    {
        return Err(ObjLoadError::Invalid("non-finite vertex data".into()));
    }
    if result
        .vertices
        .iter()
        .all(|vertex| vertex.normal == Vec3::ZERO)
    {
        let normals = result.resolved_normals();
        for (vertex, normal) in result.vertices.iter_mut().zip(normals) {
            vertex.normal = normal;
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use crate::{MeshHandle, VisualAssets};

    fn load(source: &str) -> Result<RasterMesh, ObjLoadError> {
        load_obj_reader(&mut Cursor::new(source.as_bytes()))
    }

    #[test]
    fn loads_triangle_and_generates_default_color() {
        let mesh = load("v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n").unwrap();
        assert_eq!(mesh.vertices.len(), 3);
        assert_eq!(mesh.indices, vec![0, 1, 2]);
        assert!(mesh.vertices.iter().all(|vertex| vertex.color == Vec4::ONE));
        assert!(mesh.vertices.iter().all(|vertex| vertex.normal.is_finite()));
    }

    #[test]
    fn triangulates_polygon() {
        let mesh = load("v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nf 1 2 3 4\n").unwrap();
        assert_eq!(mesh.indices.len(), 6);
    }

    #[test]
    fn loads_explicit_normals() {
        let mesh = load("v 0 0 0\nv 1 0 0\nv 0 1 0\nvn 0 0 1\nf 1//1 2//1 3//1\n").unwrap();
        assert!(mesh.vertices.iter().all(|vertex| vertex.normal == Vec3::Z));
    }

    #[test]
    fn resolves_negative_indices() {
        let mesh = load("v 0 0 0\nv 1 0 0\nv 0 1 0\nf -3 -2 -1\n").unwrap();
        assert_eq!(mesh.indices, vec![0, 1, 2]);
    }

    #[test]
    fn flattens_multiple_objects() {
        let mesh = load(
            "o first\nv 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\no second\nv 2 0 0\nv 3 0 0\nv 2 1 0\nf 4 5 6\n",
        )
        .unwrap();
        assert_eq!(mesh.vertices.len(), 6);
        assert_eq!(mesh.indices.len(), 6);
    }

    #[test]
    fn rejects_empty_and_invalid_geometry() {
        assert!(load("# empty\n").is_err());
        assert!(load("v 0 0 0\nv 1 0 0\nf 1 2 3\n").is_err());
    }

    #[test]
    fn registers_and_resolves_loaded_mesh() {
        let mesh = load("v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n").unwrap();
        let mut assets = VisualAssets::default();
        assets.register_mesh(MeshHandle::SHIP_PLACEHOLDER, VisualAssets::SHIP, mesh);
        assert_eq!(
            assets
                .mesh(MeshHandle::SHIP_PLACEHOLDER, VisualAssets::SHIP)
                .unwrap()
                .indices
                .len(),
            3
        );
    }

    #[test]
    fn loads_repository_shooter_obj() {
        use std::collections::HashSet;

        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/models/shooter/shooter.obj");
        let mesh = load_obj(path).expect("repository Shooter OBJ should load");
        assert!(!mesh.vertices.is_empty());
        assert!(!mesh.indices.is_empty());
        assert_eq!(mesh.indices.len() % 3, 0);
        assert!(mesh
            .vertices
            .iter()
            .all(|vertex| vertex.position.is_finite() && vertex.normal.is_finite()));
        let unique_positions: HashSet<_> = mesh
            .vertices
            .iter()
            .map(|vertex| vertex.position.to_array().map(f32::to_bits))
            .collect();
        let degenerate_triangles = mesh
            .indices
            .chunks_exact(3)
            .filter(|triangle| {
                let a = mesh.vertices[triangle[0] as usize].position;
                let b = mesh.vertices[triangle[1] as usize].position;
                let c = mesh.vertices[triangle[2] as usize].position;
                (b - a).cross(c - a).length_squared() <= 1e-12
            })
            .count();
        println!(
            "Shooter RasterMesh vertices={} indices={} triangles={} unique_positions={} duplicate_position_vertices={} degenerate_triangles={} approx_bytes={}",
            mesh.vertices.len(),
            mesh.indices.len(),
            mesh.indices.len() / 3,
            unique_positions.len(),
            mesh.vertices.len().saturating_sub(unique_positions.len()),
            degenerate_triangles,
            mesh.vertices.len() * std::mem::size_of::<RasterVertex>()
                + mesh.indices.len() * std::mem::size_of::<u32>()
        );
    }

    #[test]
    fn loads_repository_enemy_archetype_models_with_normals() {
        for relative in ["charger/charger.obj", "shielded/shielded.obj"] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../assets/models")
                .join(relative);
            let mesh = load_obj(path).expect("repository enemy model should load");
            assert!(!mesh.vertices.is_empty());
            assert!(mesh.vertices.iter().all(|vertex| vertex.normal.is_finite()));
            assert!(mesh.resolved_normals().iter().all(|normal| normal.is_finite()));
        }
    }

    #[test]
    fn loads_repository_building_planet_obj() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/models/building/building_planet.obj");
        let mesh = load_obj(path).expect("repository building planet OBJ should load");
        assert!(!mesh.vertices.is_empty());
        assert!(!mesh.indices.is_empty());
        assert_eq!(mesh.indices.len() % 3, 0);
        assert!(mesh
            .vertices
            .iter()
            .all(|vertex| vertex.position.is_finite() && vertex.normal.is_finite()));
        assert!(mesh
            .indices
            .iter()
            .all(|index| (*index as usize) < mesh.vertices.len()));
    }

    #[test]
    fn loads_repository_tessara_obj_with_valid_geometry() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/models/clock/clock_planet.obj");
        let mesh = load_obj(path).expect("repository Tessara OBJ should load");
        assert!(!mesh.vertices.is_empty());
        assert!(!mesh.indices.is_empty());
        assert_eq!(mesh.indices.len() % 3, 0);
        assert!(mesh
            .vertices
            .iter()
            .all(|vertex| vertex.position.is_finite() && vertex.normal.is_finite()));
        assert!(mesh
            .indices
            .iter()
            .all(|index| (*index as usize) < mesh.vertices.len()));
    }

    #[test]
    fn loads_repository_sun_obj_with_valid_geometry() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/models/sun/sun.obj");
        let mesh = load_obj(path).expect("repository sun OBJ should load");
        assert!(!mesh.vertices.is_empty());
        assert!(!mesh.indices.is_empty());
        assert_eq!(mesh.indices.len() % 3, 0);
        assert!(mesh
            .vertices
            .iter()
            .all(|vertex| vertex.position.is_finite() && vertex.normal.is_finite()));
        assert!(mesh
            .indices
            .iter()
            .all(|index| (*index as usize) < mesh.vertices.len()));
    }
}