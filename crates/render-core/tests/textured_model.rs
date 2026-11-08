
use std::path::Path;

use render_core::load_textured_obj;

#[test]
fn loads_obj_with_multiple_materials_and_textures() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/textured_model/example.obj");

    let model = load_textured_obj(&path)
        .expect("No se pudo cargar el OBJ con materiales");

    // Comprobar que existen dos materiales y dos mallas.
    assert_eq!(model.materials.len(), 2);
    assert_eq!(model.meshes.len(), 2);

    // Buscar los materiales por nombre.
    let wings = model
        .materials
        .iter()
        .position(|material| material.name == "Wings")
        .expect("No se encontro el material Wings");

    let core = model
        .materials
        .iter()
        .position(|material| material.name == "Core")
        .expect("No se encontro el material Core");

    assert_ne!(wings, core);

    // Comprobar que cada malla tiene su material.
    assert!(model.meshes.iter().any(|mesh| {
        mesh.material_index == Some(wings)
    }));

    assert!(model.meshes.iter().any(|mesh| {
        mesh.material_index == Some(core)
    }));

    // Comprobar las rutas de las texturas PNG.
    assert!(model.materials[wings]
        .diffuse_texture
        .as_ref()
        .expect("Wings no tiene textura")
        .ends_with("textures/wings.png"));

    assert!(model.materials[core]
        .diffuse_texture
        .as_ref()
        .expect("Core no tiene textura")
        .ends_with("textures/core.png"));

    // Comprobar que las coordenadas UV son validas.
    assert!(model.meshes.iter().all(|mesh| {
        mesh.mesh.vertices.iter().all(|vertex| {
            vertex.uv.is_finite()
        })
    }));

    println!("OBJ cargado correctamente");
    println!("Mallas: {}", model.meshes.len());
    println!("Materiales: {}", model.materials.len());

    for mesh in &model.meshes {
        println!(
            "Malla: {} | Material: {:?}",
            mesh.name,
            mesh.material_index
        );
    }

    for material in &model.materials {
        println!(
            "Material: {} | Textura: {:?}",
            material.name,
            material.diffuse_texture
        );
    }
}
