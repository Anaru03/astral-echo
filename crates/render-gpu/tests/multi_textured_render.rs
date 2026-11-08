
use std::path::Path;
use std::sync::Arc;

use render_core::{load_textured_obj, Mat4, Vec3};
use render_gpu::{
    texture_loader::TextureImage, GpuContext, GpuTexture,
    OffscreenTarget, TexturedRenderer,
};

#[test]
fn renders_multiple_obj_materials() {
    let gpu = match GpuContext::new(Default::default()) {
        Ok(gpu) => Arc::new(gpu),
        Err(error) => panic!("No se pudo inicializar GPU: {error}"),
    };

    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("render-core/tests/fixtures/textured_model");

    let model = load_textured_obj(root.join("example.obj"))
        .expect("No se pudo cargar el OBJ");

    assert_eq!(model.meshes.len(), 2);
    assert_eq!(model.materials.len(), 2);

    let target = OffscreenTarget::new(gpu.clone(), 512, 512)
        .expect("No se pudo crear el destino");

    let mut rendered = 0;

    for part in &model.meshes {
        let material_index = part.material_index
            .expect("Malla sin material");

        let material = &model.materials[material_index];

        let texture_path = material.diffuse_texture.as_ref()
            .expect("Material sin textura");

        let image = TextureImage::load_png(texture_path)
            .expect("No se pudo cargar PNG");

        let texture = GpuTexture::from_image(&gpu, &image)
            .expect("No se pudo subir PNG a GPU");

        let mut renderer = TexturedRenderer::new(
            gpu.clone(),
            wgpu::TextureFormat::Rgba8Unorm,
            &part.mesh,
            &texture,
        )
        .expect("No se pudo crear renderizador");

        // Centrar el OBJ de prueba dentro de la imagen.
        let mut transform = render_core::Transform::IDENTITY;
        transform.translation = Vec3::new(-0.75, -0.25, 0.0);
        transform.scale = Vec3::splat(0.5);

        renderer.set_transform(transform)
            .expect("Transformacion invalida");

        if rendered == 0 {
            renderer.render_to_view_matrix(
                target.color_view(),
                target.depth_view(),
                Mat4::IDENTITY,
            )
        } else {
            renderer.render_to_view_matrix_over(
                target.color_view(),
                target.depth_view(),
                Mat4::IDENTITY,
            )
        }
        .expect("No se pudo renderizar la submalla");

        println!(
            "Renderizado: {} | Material: {}",
            part.name,
            material.name
        );

        rendered += 1;
    }

    assert_eq!(rendered, 2);

    let result = target.readback()
        .expect("No se pudo leer la imagen");

    let output = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/texture-validation/multi_material_preview.png");

    std::fs::create_dir_all(output.parent().unwrap())
        .expect("No se pudo crear directorio");

    result.write_png(&output)
        .expect("No se pudo guardar PNG");

    println!("Imagen generada: {}", output.display());
}
