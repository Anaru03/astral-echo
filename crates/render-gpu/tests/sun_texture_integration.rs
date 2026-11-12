use render_core::{
    expand_model_parts, load_textured_obj, Camera, LightingMode, Projection, RegisteredModelPart,
    RenderSnapshot, SnapshotNode, SnapshotScene, Transform, Vec3,
};
use render_gpu::{GpuContext, GpuError, GpuRenderer, OffscreenTarget};
use std::{path::PathBuf, sync::Arc};

fn draw(with_textures: bool, gpu: Arc<GpuContext>) -> Vec<u8> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/models/sun/sun.obj");
    let mut scene = SnapshotScene::default();
    scene.set_lighting_mode(LightingMode::Pbr);
    let model = load_textured_obj(&root).expect("OBJ/MTL deben poder cargarse");
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
        scene.set_material(
            material,
            render_core::MaterialInfo::new(render_core::Vec4::ONE, 0.85, 0.0)
                .with_emissive(render_core::Vec3::new(0.55, 0.22, 0.045)),
        );
        scene.register_mesh(mesh, material, part.mesh);
        parts.push(RegisteredModelPart {
            name: part.name,
            mesh,
            material,
        });
    }
    assert_eq!(parts.len(), 772);
    let camera = Camera::new(
        Vec3::new(-0.2698, 0.0864, 12.0),
        Vec3::new(-0.2698, 0.0864, -0.1784),
        Vec3::Y,
        Projection::Perspective {
            fov_y_rad: 50.0_f32.to_radians(),
            aspect: 2.0,
            near: 0.1,
            far: 100.0,
        },
    )
    .unwrap();
    let mut snapshot = RenderSnapshot::minimal_demo();
    snapshot.nodes.clear();
    snapshot.camera_position = camera.position();
    snapshot.view = camera.view_matrix();
    snapshot.projection = camera.projection_matrix();
    let parent = SnapshotNode::destination(
        100,
        Transform::IDENTITY,
        parts[0].mesh,
        parts[0].material,
        true,
    );
    snapshot.nodes = expand_model_parts(&parent, &parts, 100);
    scene.update(&snapshot).unwrap();
    let prepared = scene.prepare(render_core::CullingConfig {
        width: 640,
        height: 320,
        ..Default::default()
    });
    println!("Submallas visibles: {}", prepared.stats().visible);
    let mut renderer = GpuRenderer::from_snapshot(gpu.clone(), &scene).unwrap();
    let target = OffscreenTarget::new(gpu, 640, 320).unwrap();
    target.render_snapshot(&mut renderer, &scene).unwrap();
    let image = target.readback().unwrap();

    let output = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/sun-preview");

    std::fs::create_dir_all(&output).unwrap();

    let filename = if with_textures {
        "sun-textured.png"
    } else {
        "sun-untextured.png"
    };

    image.write_png(output.join(filename)).unwrap();

    println!("Captura guardada: {}", output.join(filename).display());

    let colored = image
        .rgba
        .chunks_exact(4)
        .filter(|pixel| pixel[0] > 20 || pixel[1] > 20 || pixel[2] > 20)
        .count();

    let depth_pixels = image.depth.iter().filter(|depth| **depth < 1.0).count();

    println!(
        "DIAGNOSTICO GPU: texturas={}, pixeles_color={}, pixeles_depth={}",
        with_textures, colored, depth_pixels
    );

    assert!(
        depth_pixels > 100,
        "el Sol no aparece en el buffer de profundidad"
    );

    assert!(colored > 100, "el Sol se renderiza negro");

    image.rgba
}

#[test]
fn main_gpu_pipeline_uses_obj_material_png() {
    let gpu = match GpuContext::new(Default::default()) {
        Ok(gpu) => Arc::new(gpu),
        Err(GpuError::Unavailable(reason)) => {
            eprintln!("SKIP GPU: {reason}");
            return;
        }
        Err(error) => panic!("{error}"),
    };
    let textured = draw(true, gpu.clone());
    let plain = draw(false, gpu);
    let changed = textured
        .chunks_exact(4)
        .zip(plain.chunks_exact(4))
        .filter(|(a, b)| a != b)
        .count();
    println!("Pixeles diferentes con PNG: {changed}");
    assert!(
        changed > 100,
        "el pipeline principal no refleja las texturas PNG"
    );
}
