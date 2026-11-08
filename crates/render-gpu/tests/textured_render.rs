
use std::path::Path;
use std::sync::Arc;

use render_core::{Mat4, RasterMesh, RasterVertex, Vec2, Vec3, Vec4};

use render_gpu::{
    texture_loader::TextureImage, GpuContext, GpuTexture,
    OffscreenTarget, TexturedRenderer,
};

#[test]
fn renders_seraphim_gold_texture() {
    // Inicializar GPU
    let gpu = match GpuContext::new(Default::default()) {
        Ok(gpu) => Arc::new(gpu),
        Err(error) => {
            eprintln!("GPU unavailable: {error}");
            return;
        }
    };

    // Cargar textura PNG
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/textures/planets/seraphim_planet")
        .join("seraphim_gold.png");

    let image = TextureImage::load_png(&path)
        .expect("Could not load Seraphim texture");

    let texture = GpuTexture::from_image(&gpu, &image)
        .expect("Could not upload texture");

    // Crear cuadrado con coordenadas UV
    let vertices = vec![
        RasterVertex::new(
            Vec3::new(-0.8, -0.8, 0.5),
            Vec4::ONE,
        )
        .with_uv(Vec2::new(0.0, 1.0)),

        RasterVertex::new(
            Vec3::new(0.8, -0.8, 0.5),
            Vec4::ONE,
        )
        .with_uv(Vec2::new(1.0, 1.0)),

        RasterVertex::new(
            Vec3::new(0.8, 0.8, 0.5),
            Vec4::ONE,
        )
        .with_uv(Vec2::new(1.0, 0.0)),

        RasterVertex::new(
            Vec3::new(-0.8, 0.8, 0.5),
            Vec4::ONE,
        )
        .with_uv(Vec2::new(0.0, 0.0)),
    ];

    let mesh = RasterMesh::new(
        vertices,
        vec![0, 1, 2, 0, 2, 3],
    );

    // Crear renderizador texturizado
    let renderer = TexturedRenderer::new(
        gpu.clone(),
        wgpu::TextureFormat::Rgba8Unorm,
        &mesh,
        &texture,
    )
    .expect("Could not create textured renderer");

    // Crear destino offscreen
    let target = OffscreenTarget::new(
        gpu.clone(),
        512,
        512,
    )
    .expect("Could not create offscreen target");

    // Dibujar textura
    renderer
        .render_to_view_matrix(
            target.color_view(),
            target.depth_view(),
            Mat4::IDENTITY,
        )
        .expect("Could not render texture");

    // Leer imagen renderizada desde GPU
    let result = target
        .readback()
        .expect("Could not read rendered image");

    // Preparar ruta de salida
    let output = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/texture-validation/seraphim_gold_preview.png");

    std::fs::create_dir_all(
        output.parent().unwrap(),
    )
    .expect("Could not create output directory");

    // Guardar imagen PNG
    result
        .write_png(&output)
        .expect("Could not save rendered PNG");

    // Validaciones
    assert_eq!(result.width, 512);
    assert_eq!(result.height, 512);

    println!(
        "Imagen generada correctamente: {}",
        output.display()
    );
}
