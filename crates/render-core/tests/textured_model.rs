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

#[test]
fn habitat_wall_selection_excludes_habitat_furniture_and_glass() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/models/echo_7/montajes/ECHO7_Completa_DosCubiertas.obj");
    let model = load_textured_obj(path).expect("No se pudo cargar el montaje ECHO-7");
    for segment in ["01", "07", "08", "09", "88"] {
        let name = format!("ECHO7_30_Exterior_Unificado__Drum_doble_altura_{segment}");
        let mesh = model.meshes.iter().find(|mesh| mesh.name == name).unwrap();
        let material = mesh.material_index.unwrap();
        assert_eq!(model.materials[material].name, "Casco_Marfil");
    }
    for excluded in [
        "ECHO7_09_Habitat_Central__Jardinera_01",
        "ECHO7_09_Habitat_Central__Estante_hidroponico_01",
        "ECHO7_09_Habitat_Central__Cristal_cupula_01",
    ] {
        let mesh = model
            .meshes
            .iter()
            .find(|mesh| mesh.name == excluded)
            .unwrap();
        assert!(!mesh
            .name
            .starts_with("ECHO7_30_Exterior_Unificado__Drum_doble_altura_"));
    }
}

#[test]
fn lower_corridor_wall_selection_excludes_upper_corridor_and_fixtures() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/models/echo_7/montajes/ECHO7_Completa_DosCubiertas.obj");
    let model = load_textured_obj(path).expect("No se pudo cargar el montaje ECHO-7");
    for segment in ["01", "10", "20"] {
        let name = format!("ECHO7_02_Pasillos__Pared_pasillo_{segment}");
        let mesh = model.meshes.iter().find(|mesh| mesh.name == name).unwrap();
        let material = mesh.material_index.unwrap();
        assert_eq!(model.materials[material].name, "Casco_Marfil");
    }
    for excluded in [
        "ECHO7_26_Pasillos_Superiores__Pared_pasillo_01",
        "ECHO7_02_Pasillos__Suelo_pasillo_01",
        "ECHO7_02_Pasillos__Techo_pasillo_01",
    ] {
        assert!(model.meshes.iter().any(|mesh| mesh.name == excluded));
        assert!(!excluded.starts_with("ECHO7_02_Pasillos__Pared_pasillo_"));
    }
}

#[test]
fn upper_corridor_wall_selection_excludes_lower_corridor_and_fixtures() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/models/echo_7/montajes/ECHO7_Completa_DosCubiertas.obj");
    let model = load_textured_obj(path).expect("No se pudo cargar el montaje ECHO-7");
    for segment in ["01", "10", "20"] {
        let name = format!("ECHO7_26_Pasillos_Superiores__Pared_pasillo_{segment}");
        let mesh = model.meshes.iter().find(|mesh| mesh.name == name).unwrap();
        let material = mesh.material_index.unwrap();
        assert_eq!(model.materials[material].name, "Casco_Marfil");
    }
    for excluded in [
        "ECHO7_02_Pasillos__Pared_pasillo_01",
        "ECHO7_26_Pasillos_Superiores__Suelo_pasillo_01",
        "ECHO7_26_Pasillos_Superiores__Techo_pasillo_01",
    ] {
        assert!(model.meshes.iter().any(|mesh| mesh.name == excluded));
        assert!(!excluded.starts_with("ECHO7_26_Pasillos_Superiores__Pared_pasillo_"));
    }
}

#[test]
fn cloning_wall_selection_excludes_doors_roof_and_equipment() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/models/echo_7/montajes/ECHO7_Completa_DosCubiertas.obj");
    let model = load_textured_obj(path).expect("No se pudo cargar el montaje ECHO-7");
    for name in [
        "ECHO7_03_Clonacion__Pared_presion_01",
        "ECHO7_03_Clonacion__Pared_presion_27",
        "ECHO7_03_Clonacion__Pared_frente_01",
        "ECHO7_03_Clonacion__Pared_frente_02",
    ] {
        let mesh = model.meshes.iter().find(|mesh| mesh.name == name).unwrap();
        let material = mesh.material_index.unwrap();
        assert_eq!(model.materials[material].name, "Casco_Marfil");
    }
    for excluded in [
        "ECHO7_03_Clonacion__Jamba_puerta_01",
        "ECHO7_03_Clonacion__Suelo_modulo_01",
        "ECHO7_03_Clonacion__Costilla_techo_01",
        "ECHO7_03_Clonacion__Conducto_superior_01",
        "ECHO7_03_Clonacion__Tanque_base_01",
    ] {
        assert!(model.meshes.iter().any(|mesh| mesh.name == excluded));
        assert!(!excluded.contains("__Pared_presion_") && !excluded.contains("__Pared_frente_"));
    }
}

#[test]
fn cloning_ceiling_selection_includes_only_clear_envelope() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/models/echo_7/montajes/ECHO7_Completa_DosCubiertas.obj");
    let model = load_textured_obj(path).expect("No se pudo cargar el montaje ECHO-7");
    let ceiling = model
        .meshes
        .iter()
        .find(|mesh| mesh.name == "ECHO7_03_Clonacion__Dintel_envolvente_01")
        .unwrap();
    let material = ceiling.material_index.unwrap();
    assert_eq!(model.materials[material].name, "Casco_Marfil");
    for excluded in [
        "ECHO7_03_Clonacion__Costilla_techo_01",
        "ECHO7_03_Clonacion__Luminaria_techo_01",
        "ECHO7_03_Clonacion__Conducto_superior_01",
        "ECHO7_03_Clonacion__Pilar_costilla_01",
        "ECHO7_03_Clonacion__Panel_acceso_01",
        "ECHO7_03_Clonacion__Dintel_puerta_01",
        "ECHO7_03_Clonacion__Tanque_cabezal_01",
    ] {
        assert!(model.meshes.iter().any(|mesh| mesh.name == excluded));
        assert_ne!(excluded, "ECHO7_03_Clonacion__Dintel_envolvente_01");
    }
}

#[test]
fn habitat_slab_splits_only_the_downward_cloning_face() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/models/echo_7/montajes/ECHO7_Completa_DosCubiertas.obj");
    let model = load_textured_obj(path).expect("No se pudo cargar el montaje ECHO-7");
    let upper = model
        .meshes
        .iter()
        .find(|mesh| mesh.name == "ECHO7_30_Exterior_Unificado__Losa_entre_cubiertas_01")
        .unwrap();
    let lower = model
        .meshes
        .iter()
        .find(|mesh| mesh.name.ends_with("__Cara_inferior_clonacion"))
        .unwrap();
    assert!(!upper.mesh.indices.is_empty());
    assert!(!lower.mesh.indices.is_empty());
    for triangle in lower.mesh.indices.chunks_exact(3) {
        let normal = (lower.mesh.vertices[triangle[0] as usize].normal
            + lower.mesh.vertices[triangle[1] as usize].normal
            + lower.mesh.vertices[triangle[2] as usize].normal)
            .normalize_or_zero();
        assert!(normal.z < -0.5);
    }
    assert!(!upper.name.ends_with("__Cara_inferior_clonacion"));
}

#[test]
fn awakening_roof_uses_only_the_downward_face_of_shared_slab() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/models/echo_7/montajes/ECHO7_Completa_DosCubiertas.obj");
    let model = load_textured_obj(path).expect("No se pudo cargar el montaje ECHO-7");
    let upper = model
        .meshes
        .iter()
        .find(|mesh| mesh.name == "ECHO7_30_Exterior_Unificado__Losa_entre_cubiertas_02")
        .unwrap();
    let lower = model
        .meshes
        .iter()
        .find(|mesh| {
            mesh.name
                == "ECHO7_30_Exterior_Unificado__Losa_entre_cubiertas_02__Cara_inferior_despertar"
        })
        .unwrap();
    assert!(!upper.mesh.indices.is_empty());
    assert!(!lower.mesh.indices.is_empty());
    for triangle in lower.mesh.indices.chunks_exact(3) {
        let normal = (lower.mesh.vertices[triangle[0] as usize].normal
            + lower.mesh.vertices[triangle[1] as usize].normal
            + lower.mesh.vertices[triangle[2] as usize].normal)
            .normalize_or_zero();
        assert!(normal.z < -0.5);
    }
    let rib = model
        .meshes
        .iter()
        .find(|mesh| mesh.name == "ECHO7_04_Despertar__Costilla_techo_03")
        .unwrap();
    assert_eq!(
        model.materials[rib.material_index.unwrap()].name,
        "Metal_Gris"
    );
}

#[test]
fn cloning_cabinet_keeps_green_panels_and_metal_accessories_separate() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/models/echo_7/montajes/ECHO7_Completa_DosCubiertas.obj");
    let model = load_textured_obj(path).expect("No se pudo cargar el montaje ECHO-7");
    let marfil = model
        .meshes
        .iter()
        .find(|mesh| mesh.name == "ECHO7_16_Equipo_Clonacion__Banco_genetico_armario_01")
        .unwrap();
    let material = marfil.material_index.unwrap();
    assert_eq!(model.materials[material].name, "Casco_Marfil");
    for (name, expected) in [
        (
            "ECHO7_16_Equipo_Clonacion__Cajon_genetico_01",
            "Panel_Verde",
        ),
        (
            "ECHO7_16_Equipo_Clonacion__Panel_R04_BANCO_GENETICO_01",
            "Panel_Verde",
        ),
        (
            "ECHO7_16_Equipo_Clonacion__Indicador_banco_01",
            "Metal_Gris",
        ),
    ] {
        let mesh = model.meshes.iter().find(|mesh| mesh.name == name).unwrap();
        let material = mesh.material_index.unwrap();
        assert_eq!(model.materials[material].name, expected);
    }
}

#[test]
fn cloning_front_panels_select_only_the_green_panel_variant() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/models/echo_7/montajes/ECHO7_Completa_DosCubiertas.obj");
    let model = load_textured_obj(path).expect("No se pudo cargar el montaje ECHO-7");
    for name in [
        "ECHO7_16_Equipo_Clonacion__Cajon_genetico_01",
        "ECHO7_16_Equipo_Clonacion__Cajon_genetico_06",
        "ECHO7_16_Equipo_Clonacion__Panel_R04_BANCO_GENETICO_01",
    ] {
        let mesh = model.meshes.iter().find(|mesh| mesh.name == name).unwrap();
        let material = mesh.material_index.unwrap();
        assert_eq!(model.materials[material].name, "Panel_Verde");
        assert_eq!(
            model.materials[material]
                .diffuse_texture
                .as_ref()
                .unwrap()
                .file_name()
                .unwrap(),
            "panel_verde_original.png"
        );
    }
    for excluded in [
        "ECHO7_16_Equipo_Clonacion__Banco_genetico_armario_01",
        "ECHO7_16_Equipo_Clonacion__Tirador_cajon_01",
        "ECHO7_16_Equipo_Clonacion__Indicador_banco_01",
    ] {
        assert!(model.meshes.iter().any(|mesh| mesh.name == excluded));
    }
}

#[test]
fn awakening_walls_and_ceiling_exclude_capsules_and_fixtures() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/models/echo_7/montajes/ECHO7_Completa_DosCubiertas.obj");
    let model = load_textured_obj(path).expect("No se pudo cargar el montaje ECHO-7");
    for name in [
        "ECHO7_04_Despertar__Pared_presion_01",
        "ECHO7_04_Despertar__Pared_presion_27",
        "ECHO7_04_Despertar__Pared_frente_01",
        "ECHO7_04_Despertar__Pared_frente_02",
        "ECHO7_04_Despertar__Dintel_envolvente_01",
    ] {
        let mesh = model.meshes.iter().find(|mesh| mesh.name == name).unwrap();
        let material = mesh.material_index.unwrap();
        assert_eq!(model.materials[material].name, "Casco_Marfil");
    }
    for excluded in [
        "ECHO7_04_Despertar__Cristal_tapa_capsula_01",
        "ECHO7_04_Despertar__Costilla_techo_01",
        "ECHO7_04_Despertar__Conducto_superior_01",
        "ECHO7_04_Despertar__Mesa_tablero_01",
        "ECHO7_04_Despertar__Capsula_base_01",
    ] {
        assert!(model.meshes.iter().any(|mesh| mesh.name == excluded));
        assert!(!excluded.contains("__Pared_"));
    }
}
