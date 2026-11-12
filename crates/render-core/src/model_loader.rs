
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::path::Path;

use glam::{Vec2, Vec3, Vec4};

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

/// Loads a large OBJ as a bounded preview mesh by keeping one triangle every
/// `face_stride` faces. This is intended for cinematic/planet assets whose
/// source geometry is much denser than the runtime camera can use.
pub fn load_obj_decimated(
    path: impl AsRef<Path>,
    face_stride: usize,
) -> Result<RasterMesh, ObjLoadError> {
    let stride = face_stride.max(1);
    let path = path.as_ref();
    let mut faces = Vec::new();
    let mut needed = HashSet::new();
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut face_number = 0usize;
    for line in reader.lines() {
        let line = line?;
        let mut tokens = line.split_whitespace();
        if tokens.next() != Some("f") {
            continue;
        }
        let indices = tokens
            .filter_map(|token| token.split('/').next()?.parse::<isize>().ok())
            .collect::<Vec<_>>();
        if indices.len() < 3 || face_number % stride != 0 {
            face_number = face_number.saturating_add(1);
            continue;
        }
        for triangle in 1..indices.len() - 1 {
            let raw = [indices[0], indices[triangle], indices[triangle + 1]];
            if raw.iter().any(|index| *index <= 0) {
                continue;
            }
            let triangle = [raw[0] as usize, raw[1] as usize, raw[2] as usize];
            needed.extend(triangle);
            faces.push(triangle);
        }
        face_number = face_number.saturating_add(1);
    }
    if faces.is_empty() {
        return Err(ObjLoadError::Empty);
    }

    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut positions = HashMap::with_capacity(needed.len());
    let mut vertex_index = 0usize;
    for line in reader.lines() {
        let line = line?;
        let mut tokens = line.split_whitespace();
        if tokens.next() != Some("v") {
            continue;
        }
        vertex_index = vertex_index.saturating_add(1);
        if !needed.contains(&vertex_index) {
            continue;
        }
        let values = tokens
            .take(3)
            .filter_map(|value| value.parse::<f32>().ok())
            .collect::<Vec<_>>();
        if values.len() != 3 || values.iter().any(|value| !value.is_finite()) {
            return Err(ObjLoadError::Invalid("invalid decimated vertex".into()));
        }
        positions.insert(
            vertex_index,
            crate::Vec3::new(values[0], values[1], values[2]),
        );
    }

    let mut vertices = Vec::with_capacity(faces.len() * 3);
    let mut indices = Vec::with_capacity(faces.len() * 3);
    for face in faces {
        let [a, b, c] = face;
        let (Some(a), Some(b), Some(c)) =
            (positions.get(&a), positions.get(&b), positions.get(&c))
        else {
            continue;
        };
        let normal = (*b - *a).cross(*c - *a).normalize_or_zero();
        let base = vertices.len() as u32;
        vertices.extend([
            RasterVertex::new(*a, crate::Vec4::ONE).with_normal(normal),
            RasterVertex::new(*b, crate::Vec4::ONE).with_normal(normal),
            RasterVertex::new(*c, crate::Vec4::ONE).with_normal(normal),
        ]);
        indices.extend([base, base + 1, base + 2]);
    }
    if indices.is_empty() {
        return Err(ObjLoadError::Empty);
    }
    Ok(RasterMesh::new(vertices, indices))
}

pub fn load_obj_reader<R: BufRead>(
    reader: &mut R,
) -> Result<RasterMesh, ObjLoadError> {
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

pub(crate) fn build_mesh(
    meshes: impl IntoIterator<Item = tobj::Mesh>,
) -> Result<RasterMesh, ObjLoadError> {
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

        if !mesh.normals.is_empty()
            && mesh.normals.len() != vertex_count * 3
        {
            return Err(ObjLoadError::Invalid(
                "single-index normals do not match positions".into(),
            ));
        }

        // Validar las coordenadas UV del modelo OBJ.
        if !mesh.texcoords.is_empty()
            && mesh.texcoords.len() != vertex_count * 2
        {
            return Err(ObjLoadError::Invalid(
                "texture coordinates do not match positions".into(),
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
                return Err(ObjLoadError::Invalid(
                    "non-finite position".into(),
                ));
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
                return Err(ObjLoadError::Invalid(
                    "non-finite normal".into(),
                ));
            }

            // Leer UVs. Los modelos sin UVs usan (0, 0).
            let uv = if mesh.texcoords.is_empty() {
                Vec2::ZERO
            } else {
                Vec2::new(
                    mesh.texcoords[index * 2],
                    mesh.texcoords[index * 2 + 1],
                )
            };

            if !uv.is_finite() {
                return Err(ObjLoadError::Invalid(
                    "non-finite texture coordinates".into(),
                ));
            }

            vertices.push(
                RasterVertex::new(position, color)
                    .with_normal(normal)
                    .with_uv(uv),
            );
        }

        for index in mesh.indices {
            if index as usize >= vertex_count {
                return Err(ObjLoadError::Invalid(
                    "index outside vertex data".into(),
                ));
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
        return Err(ObjLoadError::Invalid(
            "non-finite vertex data".into(),
        ));
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
        let mesh = load(
            "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n",
        )
        .unwrap();

        assert_eq!(mesh.vertices.len(), 3);
        assert_eq!(mesh.indices, vec![0, 1, 2]);

        assert!(mesh
            .vertices
            .iter()
            .all(|vertex| vertex.color == Vec4::ONE));

        assert!(mesh
            .vertices
            .iter()
            .all(|vertex| vertex.normal.is_finite()));

        assert!(mesh
            .vertices
            .iter()
            .all(|vertex| vertex.uv == Vec2::ZERO));
    }

    #[test]
    fn triangulates_polygon() {
        let mesh = load(
            "v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nf 1 2 3 4\n",
        )
        .unwrap();

        assert_eq!(mesh.indices.len(), 6);
    }

    #[test]
    fn loads_explicit_normals() {
        let mesh = load(
            "v 0 0 0\nv 1 0 0\nv 0 1 0\nvn 0 0 1\nf 1//1 2//1 3//1\n",
        )
        .unwrap();

        assert!(mesh
            .vertices
            .iter()
            .all(|vertex| vertex.normal == Vec3::Z));
    }

    #[test]
    fn loads_texture_coordinates() {
        let source = concat!(
            "v 0 0 0\n",
            "v 1 0 0\n",
            "v 0 1 0\n",
            "vt 0 0\n",
            "vt 1 0\n",
            "vt 0 1\n",
            "f 1/1 2/2 3/3\n",
        );

        let mesh = load(source).unwrap();

        assert_eq!(mesh.vertices.len(), 3);
        assert_eq!(mesh.indices, vec![0, 1, 2]);

        assert_eq!(mesh.vertices[0].uv, Vec2::new(0.0, 0.0));
        assert_eq!(mesh.vertices[1].uv, Vec2::new(1.0, 0.0));
        assert_eq!(mesh.vertices[2].uv, Vec2::new(0.0, 1.0));
    }

    #[test]
    fn rejects_non_finite_texture_coordinates() {
        let source = concat!(
            "v 0 0 0\n",
            "v 1 0 0\n",
            "v 0 1 0\n",
            "vt 0 0\n",
            "vt 1e39 0\n",
            "vt 0 1\n",
            "f 1/1 2/2 3/3\n",
        );

        assert!(load(source).is_err());
    }

    #[test]
    fn resolves_negative_indices() {
        let mesh = load(
            "v 0 0 0\nv 1 0 0\nv 0 1 0\nf -3 -2 -1\n",
        )
        .unwrap();

        assert_eq!(mesh.indices, vec![0, 1, 2]);
    }

    #[test]
    fn flattens_multiple_objects() {
        let mesh = load(
            "o first\n\
             v 0 0 0\n\
             v 1 0 0\n\
             v 0 1 0\n\
             f 1 2 3\n\
             o second\n\
             v 2 0 0\n\
             v 3 0 0\n\
             v 2 1 0\n\
             f 4 5 6\n",
        )
        .unwrap();

        assert_eq!(mesh.vertices.len(), 6);
        assert_eq!(mesh.indices.len(), 6);
    }

    #[test]
    fn rejects_empty_and_invalid_geometry() {
        assert!(load("# empty\n").is_err());

        assert!(
            load("v 0 0 0\nv 1 0 0\nf 1 2 3\n").is_err()
        );
    }

    #[test]
    fn registers_and_resolves_loaded_mesh() {
        let mesh = load(
            "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n",
        )
        .unwrap();

        let mut assets = VisualAssets::default();

        assets.register_mesh(
            MeshHandle::SHIP_PLACEHOLDER,
            VisualAssets::SHIP,
            mesh,
        );

        assert_eq!(
            assets
                .mesh(
                    MeshHandle::SHIP_PLACEHOLDER,
                    VisualAssets::SHIP,
                )
                .unwrap()
                .indices
                .len(),
            3
        );
    }

    #[test]
    fn loads_repository_shooter_obj() {
        use std::collections::HashSet;

        let path = std::path::Path::new(
            env!("CARGO_MANIFEST_DIR"),
        )
        .join("../../assets/models/shooter/shooter.obj");

        let mesh = load_obj(path)
            .expect("repository Shooter OBJ should load");

        assert!(!mesh.vertices.is_empty());
        assert!(!mesh.indices.is_empty());
        assert_eq!(mesh.indices.len() % 3, 0);

        assert!(mesh.vertices.iter().all(|vertex| {
            vertex.position.is_finite()
                && vertex.normal.is_finite()
                && vertex.uv.is_finite()
        }));

        let unique_positions: HashSet<_> = mesh
            .vertices
            .iter()
            .map(|vertex| {
                vertex.position.to_array().map(f32::to_bits)
            })
            .collect();

        let degenerate_triangles = mesh
            .indices
            .chunks_exact(3)
            .filter(|triangle| {
                let a = mesh.vertices[triangle[0] as usize].position;
                let b = mesh.vertices[triangle[1] as usize].position;
                let c = mesh.vertices[triangle[2] as usize].position;

                (b - a)
                    .cross(c - a)
                    .length_squared()
                    <= 1e-12
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
        for relative in [
            "charger/charger.obj",
            "shielded/shielded.obj",
        ] {
            let path = std::path::Path::new(
                env!("CARGO_MANIFEST_DIR"),
            )
            .join("../../assets/models")
            .join(relative);

            let mesh = load_obj(path)
                .expect("repository enemy model should load");

            assert!(!mesh.vertices.is_empty());

            assert!(mesh
                .vertices
                .iter()
                .all(|vertex| {
                    vertex.normal.is_finite()
                        && vertex.uv.is_finite()
                }));

            assert!(mesh
                .resolved_normals()
                .iter()
                .all(|normal| normal.is_finite()));
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
