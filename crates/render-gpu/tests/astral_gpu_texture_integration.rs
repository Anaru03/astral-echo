use render_core::{
    expand_model_parts, load_textured_obj, Camera, LightingMode, Projection, RegisteredModelPart,
    RenderSnapshot, SnapshotNode, SnapshotScene, Transform, Vec3,
};
use render_gpu::{GpuContext, GpuError, GpuRenderer, OffscreenTarget};
use std::{path::PathBuf, sync::Arc};

fn draw(with_textures: bool, gpu: Arc<GpuContext>) -> Vec<u8> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../render-core/tests/fixtures/textured_model/example.obj");
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
        scene.register_mesh(mesh, material, part.mesh);
        parts.push(RegisteredModelPart {
            name: part.name,
            mesh,
            material,
        });
    }
    for part in &parts {
        println!(
            "MATERIAL GPU: texturas={}, nombre={}, ruta={:?}",
            with_textures,
            part.name,
            scene
                .prepare(Default::default())
                .texture_path(part.material)
                .map(|p| p.to_path_buf()),
        );
    }
    assert_eq!(parts.len(), 2);
    let camera = Camera::new(
        Vec3::new(1.5, 0.5, 5.0),
        Vec3::new(1.5, 0.5, 0.0),
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
    assert_eq!(
        prepared.stats().visible,
        2,
        "ambas submallas deben ser visibles"
    );
    let mut renderer = GpuRenderer::from_snapshot(gpu.clone(), &scene).unwrap();
    let target = OffscreenTarget::new(gpu, 640, 320).unwrap();
    target.render_snapshot(&mut renderer, &scene).unwrap();
    let image = target.readback().unwrap();

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
