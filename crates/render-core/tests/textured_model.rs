use std::path::Path;

use render_core::load_textured_obj;
use render_core::{register_textured_model, SnapshotScene, TextureAddressMode};

#[test]
fn loads_obj_with_multiple_materials_and_textures() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/textured_model/example.obj");

    let model = load_textured_obj(&path).expect("No se pudo cargar el OBJ con materiales");

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
    assert!(model
        .meshes
        .iter()
        .any(|mesh| { mesh.material_index == Some(wings) }));

    assert!(model
        .meshes
        .iter()
        .any(|mesh| { mesh.material_index == Some(core) }));

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
        mesh.mesh
            .vertices
            .iter()
            .all(|vertex| vertex.uv.is_finite())
    }));

    println!("OBJ cargado correctamente");
    println!("Mallas: {}", model.meshes.len());
    println!("Materiales: {}", model.materials.len());

    for mesh in &model.meshes {
        println!("Malla: {} | Material: {:?}", mesh.name, mesh.material_index);
    }

    for material in &model.materials {
        println!(
            "Material: {} | Textura: {:?}",
            material.name, material.diffuse_texture
        );
    }
}

#[test]
fn registers_confirmed_habitat_wall_with_seamless_repeat() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/models/echo_7/montajes");
    let obj = root.join("ECHO7_Completa_DosCubiertas.obj");
    let model = load_textured_obj(&obj).expect("No se pudo cargar el montaje ECHO-7");
    let wall = model
        .meshes
        .iter()
        .find(|part| part.name == "ECHO7_30_Exterior_Unificado__Drum_doble_altura_08")
        .expect("No se encontró la pared seleccionada");
    let material_index = wall.material_index.expect("La pared no tiene material");
    assert_eq!(model.materials[material_index].name, "Casco_Marfil");

    let mut scene = SnapshotScene::default();
    let parts = register_textured_model(&mut scene, &obj, 100_000, 100_000)
        .expect("No se pudo registrar el montaje ECHO-7");
    let registered = parts
        .iter()
        .find(|part| part.name == wall.name)
        .expect("La pared no fue registrada");
    assert!(scene
        .texture_path(registered.material)
        .is_some_and(|path| path.ends_with("base_marfil_seamless.png")));
    assert_eq!(
        scene.texture_address_mode(registered.material),
        TextureAddressMode::Repeat
    );
    let image = scene
        .texture_image(registered.material)
        .expect("PNG no decodificado");
    assert!(image.width > 0 && image.height > 0);
}
