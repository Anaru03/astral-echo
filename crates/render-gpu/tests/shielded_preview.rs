use std::{path::PathBuf, sync::Arc};
use render_core::{
    expand_model_parts, load_textured_obj, Camera, LightingMode, Projection, RegisteredModelPart,
    RenderSnapshot, SnapshotNode, SnapshotScene, Transform, Vec3,
};
use render_gpu::{GpuContext, GpuError, GpuRenderer, OffscreenTarget};

fn draw(with_textures: bool, gpu: Arc<GpuContext>) -> Vec<u8> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/models/shielded/shielded.obj");
    let mut scene = SnapshotScene::default();
    scene.set_lighting_mode(LightingMode::Pbr);
    let model = load_textured_obj(&root).expect("OBJ/MTL deben poder cargarse");
    println!("===== MATERIALES SHIELDED =====");
    println!("Materiales encontrados: {}", model.materials.len());

    for material in &model.materials {
        println!(
            "Material: {} | Color: {:?} | Emision: {:?}",
            material.name,
            material.info.base_color,
            material.info.emissive
        );
    }

    let mut parts = Vec::new();
    for (index, part) in model.meshes.into_iter().enumerate() {
        let mesh = render_core::MeshHandle::new(900_000 + index as u32);
        let material = render_core::MaterialHandle::new(900_000 + index as u32);
        if with_textures {
            if let Some(material_index) = part.material_index {
                if let Some(path) = &model.materials[material_index].diffuse_texture {
                    scene.register_material_texture(material, path.clone());
                }
            }
        }
        if let Some(index) = part.material_index {
            scene.set_material(material, model.materials[index].info);
        } else {
            scene.set_material(material, render_core::MaterialInfo::ship());
        }
        scene.register_mesh(mesh, material, part.mesh);
        parts.push(RegisteredModelPart { name: part.name, mesh, material });
    }
    assert_eq!(parts.len(), 349);
    let camera = Camera::new(
        Vec3::new(6.0, 5.0, 11.0),
        Vec3::ZERO,
        Vec3::Y,
        Projection::Perspective {
            fov_y_rad: 50.0_f32.to_radians(),
            aspect: 2.0,
            near: 0.1,
            far: 100.0,
        },
    ).unwrap();
    let mut snapshot = RenderSnapshot::minimal_demo();
    snapshot.nodes.clear();
    snapshot.lights = vec![
        render_core::SnapshotLight::new(
            render_core::RenderLight {
                color: Vec3::ONE,
                intensity: 3.0,
                kind: render_core::RenderLightKind::Directional {
                    direction: Vec3::new(-0.5, -1.0, -0.8).normalize(),
                },
                casts_shadow: false,
            },
        ),
        render_core::SnapshotLight::new(
            render_core::RenderLight {
                color: Vec3::new(0.85, 0.92, 1.0),
                intensity: 2.0,
                kind: render_core::RenderLightKind::Directional {
                    direction: Vec3::new(0.6, -0.7, 0.5).normalize(),
                },
                casts_shadow: false,
            },
        ),
    ];
    snapshot.camera_position = camera.position();
    snapshot.view = camera.view_matrix();
    snapshot.projection = camera.projection_matrix();
    let parent = SnapshotNode::destination(
        100, Transform::IDENTITY, parts[0].mesh, parts[0].material, true,
    );
    snapshot.nodes = expand_model_parts(&parent, &parts, 100);
    scene.update(&snapshot).unwrap();

    // Iluminacion ambiental para apreciar el casco y blindaje.

    let mut prepared = scene.prepare(render_core::CullingConfig {
        width: 640, height: 320, ..Default::default()
    });

    // Iluminacion ambiental exclusiva de la vista previa de Shielded.
    prepared.lighting.ambient = Vec3::splat(0.35);
    println!("Submallas visibles: {}", prepared.stats().visible);
    let mut renderer = GpuRenderer::from_prepared(gpu.clone(), &prepared).unwrap();
    let target = OffscreenTarget::new(gpu, 640, 320).unwrap();
    target.render_prepared(
        &mut renderer,
        &prepared,
        scene.view_projection(),
    ).unwrap();
    let image = target.readback().unwrap();

    let output = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/shielded-preview");

    std::fs::create_dir_all(&output).unwrap();

    let filename = if with_textures {
        "shielded.png"
    } else {
        "shielded-untextured.png"
    };

    image.write_png(output.join(filename)).unwrap();

    println!("Captura guardada: {}", output.join(filename).display());

    let colored = image.rgba.chunks_exact(4)
        .filter(|pixel| pixel[0] > 20 || pixel[1] > 20 || pixel[2] > 20)
        .count();

    let depth_pixels = image.depth.iter()
        .filter(|depth| **depth < 1.0)
        .count();

    println!(
        "DIAGNOSTICO GPU: texturas={}, pixeles_color={}, pixeles_depth={}",
        with_textures, colored, depth_pixels
    );

    assert!(
        depth_pixels > 100,
        "Shooter no aparece en el buffer de profundidad"
    );

    assert!(
        colored > 100,
        "Shooter se renderiza negro"
    );

    image.rgba
}

#[test]
fn shielded_material_preview() {
    let gpu = match GpuContext::new(Default::default()) {
        Ok(gpu) => Arc::new(gpu),
        Err(GpuError::Unavailable(reason)) => {
            eprintln!("SKIP GPU: {reason}");
            return;
        }
        Err(error) => panic!("{error}"),
    };

    draw(true, gpu.clone());
    draw(false, gpu);
}
