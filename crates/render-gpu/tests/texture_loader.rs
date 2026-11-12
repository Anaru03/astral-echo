use std::path::Path;

use render_gpu::texture_loader::TextureImage;

#[test]
fn loads_all_seraphim_textures_as_rgba8() {
    let base =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/textures/planets/seraphim_planet");

    let files = [
        "seraphim_wings.png",
        "seraphim_gold.png",
        "seraphim_core.png",
        "seraphim_crystals.png",
    ];

    for file in files {
        let path = base.join(file);

        let image = TextureImage::load_png(&path)
            .unwrap_or_else(|error| panic!("Failed to load {}: {}", path.display(), error));

        assert!(image.width > 0);
        assert!(image.height > 0);

        assert_eq!(
            image.pixels.len(),
            image.width as usize * image.height as usize * 4,
            "Invalid RGBA8 buffer for {file}"
        );

        println!(
            "{}: {}x{}, {} bytes",
            file,
            image.width,
            image.height,
            image.pixels.len()
        );
    }
}
