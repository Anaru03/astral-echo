
use std::path::Path;
use std::sync::Arc;

use render_core::Mat4;
use render_gpu::{
    GpuContext, OffscreenTarget, TexturedModelRenderer,
};

#[test]
fn loads_and_renders_textured_model_automatically() {
    let gpu = Arc::new(
        GpuContext::new(Default::default())
            .expect("No se pudo inicializar GPU"),
    );

    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../render-core/tests/fixtures/textured_model/example.obj");

    let renderer = TexturedModelRenderer::load(
        gpu.clone(),
        wgpu::TextureFormat::Rgba8Unorm,
        &path,
    )
    .expect("No se pudo cargar el modelo texturizado");

    assert_eq!(renderer.part_count(), 2);
    assert_eq!(renderer.texture_count(), 2);

    let target = OffscreenTarget::new(
        gpu.clone(),
        512,
        512,
    )
    .expect("No se pudo crear el destino");

    // Encuadrar el OBJ de prueba dentro de la imagen.
    let vp = Mat4::from_scale_rotation_translation(
        render_core::Vec3::splat(0.5),
        render_core::Quat::IDENTITY,
        render_core::Vec3::new(-0.75, -0.25, 0.0),
    );

    renderer
        .render_to_view_matrix(
            target.color_view(),
            target.depth_view(),
            vp,
        )
        .expect("No se pudo renderizar el modelo");

    let result = target
        .readback()
        .expect("No se pudo leer la imagen");

    let output = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/texture-validation/automatic_model_preview.png");

    std::fs::create_dir_all(output.parent().unwrap())
        .expect("No se pudo crear la carpeta");

    result
        .write_png(&output)
        .expect("No se pudo guardar PNG");

    println!("Mallas: {}", renderer.part_count());
    println!("Texturas: {}", renderer.texture_count());
    println!("Imagen generada: {}", output.display());
}
