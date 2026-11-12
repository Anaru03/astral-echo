use std::path::Path;

use render_gpu::{texture_loader::TextureImage, GpuContext, GpuTexture};

#[test]
fn uploads_seraphim_texture_to_gpu() {
    // Crear contexto GPU.
    let gpu = match GpuContext::new(Default::default()) {
        Ok(gpu) => gpu,
        Err(error) => {
            eprintln!("GPU no disponible: {error}");
            return;
        }
    };

    // Ruta de una textura real del proyecto.
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/textures/planets/seraphim_planet")
        .join("seraphim_gold.png");

    // Decodificar PNG.
    let image = TextureImage::load_png(&path).expect("No se pudo cargar la textura PNG");

    assert_eq!(
        image.pixels.len(),
        image.width as usize * image.height as usize * 4
    );

    // Crear textura GPU.
    let texture = GpuTexture::from_image(&gpu, &image).expect("No se pudo crear la textura GPU");

    assert_eq!(texture.width, image.width);
    assert_eq!(texture.height, image.height);

    // Verificar errores reportados por el contexto.
    gpu.check().expect("La GPU reporto un error");

    println!("Textura GPU creada: {}x{}", texture.width, texture.height);
}
