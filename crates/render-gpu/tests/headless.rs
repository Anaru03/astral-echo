#[path = "../examples/support/mod.rs"]
mod support;

use render_core::{demo_camera, demo_scene, RasterMesh, RasterVertex, Transform, Vec3, Vec4};
use render_gpu::{
    GpuContext, GpuError, GpuOptions, GpuRenderer, OffscreenTarget, PackedScene, ReadbackLayout,
    CLEAR_RGBA,
};
use std::sync::{Arc, OnceLock};

fn adapter() -> Option<Arc<GpuContext>> {
    static GPU: OnceLock<Result<Arc<GpuContext>, GpuError>> = OnceLock::new();
    match GPU.get_or_init(|| GpuContext::new(GpuOptions::default()).map(Arc::new)) {
        Ok(gpu) => Some(gpu.clone()),
        Err(GpuError::Unavailable(e)) => {
            eprintln!("SKIP GPU: {e}");
            None
        }
        Err(e) => panic!("GPU initialization failed: {e}"),
    }
}

#[test]
fn absent_backend_is_explicit() {
    assert!(matches!(
        GpuContext::new(GpuOptions {
            backends: wgpu::Backends::empty(),
            force_fallback_adapter: false
        }),
        Err(GpuError::Unavailable(_))
    ));
    let image = support::cpu_image(&demo_scene(), &demo_camera(1.0).unwrap(), 32, 32);
    assert!(image.rgba.chunks_exact(4).any(|p| p != CLEAR_RGBA));
}

#[test]
fn png_round_trip_and_invalid_images() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/f5a-validation/round-trip.png");
    let image = render_gpu::GpuImage {
        width: 2,
        height: 1,
        rgba: vec![1, 2, 3, 4, 255, 128, 64, 32],
        depth: vec![1.0; 2],
    };
    image.write_png(&path).unwrap();
    let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).unwrap()));
    let mut reader = decoder.read_info().unwrap();
    let mut bytes = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut bytes).unwrap();
    assert_eq!((info.width, info.height), (2, 1));
    assert_eq!(&bytes[..info.buffer_size()], image.rgba);
    let mut invalid = image;
    invalid.rgba.pop();
    assert!(invalid
        .write_png("target/f5a-validation/invalid.png")
        .is_err());
    invalid.width = 0;
    assert!(invalid
        .write_png("target/f5a-validation/invalid.png")
        .is_err());
}

#[test]
fn malformed_public_readback_layout_returns_error() {
    let layout = ReadbackLayout {
        width: 1,
        height: 1,
        bytes_per_row: 0,
        buffer_size: 0,
    };
    assert!(layout.unpack(&[]).is_err());
}

#[test]
fn readback_padding_and_invalid_dimensions() {
    for (width, height) in [(1, 1), (64, 2), (65, 3), (321, 181)] {
        let layout = ReadbackLayout::new(width, height).unwrap();
        assert_eq!(layout.bytes_per_row % 256, 0);
        let mut bytes = vec![255; layout.buffer_size as usize];
        for row in bytes.chunks_exact_mut(layout.bytes_per_row as usize) {
            row[..width as usize * 4].fill(42);
        }
        assert_eq!(
            layout.unpack(&bytes).unwrap(),
            vec![42; width as usize * height as usize * 4]
        );
        assert!(layout.unpack(&bytes[..bytes.len() - 1]).is_err());
    }
    assert!(ReadbackLayout::new(0, 2).is_err());
    assert!(ReadbackLayout::new(u32::MAX, 2).is_err());
}

#[test]
fn scene_packing_rebases_indices_and_rejects_invalid_inputs() {
    let scene = demo_scene();
    let packed = PackedScene::new(&scene).unwrap();
    assert_eq!(packed.draws.len(), 19);
    assert_eq!(packed.indices.len() / 3, 278);
    assert!(packed
        .indices
        .iter()
        .all(|i| (*i as usize) < packed.vertices.len()));
    let mut bad = scene.clone();
    bad[0].0.indices[0] = u32::MAX;
    assert!(PackedScene::new(&bad).is_err());
    bad = scene.clone();
    bad[0].0.vertices[0].position.x = f32::NAN;
    assert!(PackedScene::new(&bad).is_err());
    bad = scene;
    bad[0].0.indices.pop();
    assert!(PackedScene::new(&bad).is_err());
}

#[test]
fn f3_color_and_depth_match_cpu_at_all_sizes() {
    let Some(gpu) = adapter() else {
        return;
    };
    println!("TEST ADAPTER {:?}", gpu.adapter_info());
    let scene = demo_scene();
    let mut renderer =
        GpuRenderer::new(gpu.clone(), wgpu::TextureFormat::Rgba8Unorm, &scene).unwrap();
    for (w, h) in [(320, 180), (640, 360), (1280, 720), (321, 181)] {
        let camera = demo_camera(w as f32 / h as f32).unwrap();
        let target = OffscreenTarget::new(gpu.clone(), w, h).unwrap();
        target.render(&mut renderer, &camera).unwrap();
        let image = target.readback().unwrap();
        let cpu = support::cpu_image(&scene, &camera, w, h);
        let comparison = support::compare(&cpu, &image);
        println!("{w}x{h}: {comparison:?}");
        assert!(comparison.passes(), "{comparison:?}");
        assert!(image.depth.iter().any(|z| *z < 1.0));
    }
}

#[test]
fn persistent_resources_resize_and_determinism() {
    let Some(gpu) = adapter() else {
        return;
    };
    let scene = demo_scene();
    let mut renderer =
        GpuRenderer::new(gpu.clone(), wgpu::TextureFormat::Rgba8Unorm, &scene).unwrap();
    let stats = renderer.resources();
    let mut target = OffscreenTarget::new(gpu, 65, 37).unwrap();
    let camera = demo_camera(65.0 / 37.0).unwrap();
    target.render(&mut renderer, &camera).unwrap();
    let a = target.readback().unwrap();
    target.render(&mut renderer, &camera).unwrap();
    let b = target.readback().unwrap();
    assert_eq!(a.rgba, b.rgba);
    assert_eq!(a.depth, b.depth);
    assert!(target.resize(0, 1).is_err());
    assert_eq!(target.size(), (65, 37));
    target.resize(97, 53).unwrap();
    target.render(&mut renderer, &camera).unwrap();
    assert_eq!(target.readback().unwrap().rgba.len(), 97 * 53 * 4);
    let before_transform = target.readback().unwrap();
    let mut transform = scene[0].1;
    transform.translation.y += 5.0;
    renderer.set_transform(0, transform).unwrap();
    assert!(renderer.set_transform(usize::MAX, transform).is_err());
    assert_eq!(renderer.resources().uniform_bytes, stats.uniform_bytes);
    target.render(&mut renderer, &camera).unwrap();
    assert_ne!(target.readback().unwrap().rgba, before_transform.rgba);
}

#[test]
fn empty_scene_clears_color_and_depth() {
    let Some(gpu) = adapter() else {
        return;
    };
    let mut renderer = GpuRenderer::new(gpu.clone(), wgpu::TextureFormat::Rgba8Unorm, &[]).unwrap();
    let target = OffscreenTarget::new(gpu, 7, 5).unwrap();
    target
        .render(&mut renderer, &demo_camera(1.4).unwrap())
        .unwrap();
    let image = target.readback().unwrap();
    assert!(image.rgba.chunks_exact(4).all(|p| p == CLEAR_RGBA));
    assert!(image.depth.iter().all(|z| *z == 1.0));
}

#[test]
fn invalid_formats_extents_and_transforms_do_not_poison_device() {
    let Some(gpu) = adapter() else {
        return;
    };
    assert!(GpuRenderer::new(gpu.clone(), wgpu::TextureFormat::Rgba16Float, &[]).is_err());
    assert!(OffscreenTarget::new(gpu.clone(), u32::MAX, 1).is_err());
    let target = OffscreenTarget::new(gpu.clone(), 32, 32).unwrap();
    let mut incompatible = GpuRenderer::new(
        gpu.clone(),
        wgpu::TextureFormat::Bgra8UnormSrgb,
        &demo_scene(),
    )
    .unwrap();
    let camera = demo_camera(1.0).unwrap();
    assert!(target.render(&mut incompatible, &camera).is_err());
    let mut renderer =
        GpuRenderer::new(gpu.clone(), wgpu::TextureFormat::Rgba8Unorm, &demo_scene()).unwrap();
    let mut invalid = Transform::IDENTITY;
    invalid.translation.x = f32::NAN;
    assert!(renderer.set_transform(0, invalid).is_err());
    target.render(&mut renderer, &camera).unwrap();
    gpu.wait().unwrap();
    gpu.check().unwrap();
}

#[test]
fn perspective_color_winding_and_depth_order_match_cpu() {
    let Some(gpu) = adapter() else {
        return;
    };
    let triangle = RasterMesh::triangle(
        RasterVertex::new(Vec3::new(-8.0, 0.0, 0.0), Vec4::new(1.0, 0.0, 0.0, 1.0)),
        RasterVertex::new(Vec3::new(8.0, 0.0, 0.0), Vec4::new(0.0, 1.0, 0.0, 1.0)),
        RasterVertex::new(Vec3::new(0.0, 15.0, -15.0), Vec4::new(0.0, 0.0, 1.0, 1.0)),
    );
    let mut reverse = triangle.clone();
    reverse.indices.reverse();
    let mut back = Transform::IDENTITY;
    back.translation.z = -3.0;
    let scene = vec![(triangle, Transform::IDENTITY), (reverse, back)];
    let camera = demo_camera(1.0).unwrap();
    let mut renderer =
        GpuRenderer::new(gpu.clone(), wgpu::TextureFormat::Rgba8Unorm, &scene).unwrap();
    let target = OffscreenTarget::new(gpu.clone(), 256, 256).unwrap();
    target.render(&mut renderer, &camera).unwrap();
    let a = target.readback().unwrap();
    assert!(a.depth.iter().any(|z| *z < 1.0));
    assert!(support::compare(&support::cpu_image(&scene, &camera, 256, 256), &a).passes());
    let mut reversed_scene = scene;
    reversed_scene.reverse();
    let mut renderer =
        GpuRenderer::new(gpu, wgpu::TextureFormat::Rgba8Unorm, &reversed_scene).unwrap();
    target.render(&mut renderer, &camera).unwrap();
    let b = target.readback().unwrap();
    assert_eq!(a.rgba, b.rgba);
    assert_eq!(a.depth, b.depth);
}

#[test]
fn nearer_mesh_occludes_later_far_mesh_with_reversed_winding() {
    let Some(gpu) = adapter() else {
        return;
    };
    let camera = render_core::Camera::new(
        Vec3::ZERO,
        Vec3::NEG_Z,
        Vec3::Y,
        render_core::Projection::Perspective {
            fov_y_rad: std::f32::consts::FRAC_PI_2,
            aspect: 1.0,
            near: 0.1,
            far: 10.0,
        },
    )
    .unwrap();
    let make = |z: f32, scale: f32, color: Vec4| {
        RasterMesh::triangle(
            RasterVertex::new(Vec3::new(-0.6 * scale, -0.6 * scale, z), color),
            RasterVertex::new(Vec3::new(0.6 * scale, -0.6 * scale, z), color),
            RasterVertex::new(Vec3::new(0.0, 0.6 * scale, z), color),
        )
    };
    let near = make(-1.0, 1.0, Vec4::new(1.0, 0.0, 0.0, 1.0));
    let mut far = make(-2.0, 2.0, Vec4::new(0.0, 0.0, 1.0, 1.0));
    far.indices.reverse();
    let mut scene = vec![(near, Transform::IDENTITY), (far, Transform::IDENTITY)];
    let target = OffscreenTarget::new(gpu.clone(), 64, 64).unwrap();
    for _ in 0..2 {
        let mut renderer =
            GpuRenderer::new(gpu.clone(), wgpu::TextureFormat::Rgba8Unorm, &scene).unwrap();
        target.render(&mut renderer, &camera).unwrap();
        let image = target.readback().unwrap();
        let center = 32 * 64 + 32;
        assert_eq!(&image.rgba[center * 4..center * 4 + 4], &[255, 0, 0, 255]);
        assert!((image.depth[center] - 0.9090909).abs() < 1e-5);
        assert!(support::compare(&support::cpu_image(&scene, &camera, 64, 64), &image).passes());
        scene.reverse();
    }
}

#[test]
fn near_plane_clipping_and_perspective_varyings_match_cpu() {
    let Some(gpu) = adapter() else {
        return;
    };
    let camera = render_core::Camera::new(
        Vec3::ZERO,
        Vec3::NEG_Z,
        Vec3::Y,
        render_core::Projection::Perspective {
            fov_y_rad: std::f32::consts::FRAC_PI_2,
            aspect: 1.0,
            near: 0.1,
            far: 10.0,
        },
    )
    .unwrap();
    let mesh = RasterMesh::triangle(
        RasterVertex::new(Vec3::new(-0.5, -0.5, -1.0), Vec4::new(1.0, 0.0, 0.0, 1.0)),
        RasterVertex::new(Vec3::new(0.5, -0.5, -1.0), Vec4::new(0.0, 1.0, 0.0, 1.0)),
        RasterVertex::new(Vec3::new(0.0, 0.5, -0.05), Vec4::new(0.0, 0.0, 1.0, 1.0)),
    );
    let scene = vec![(mesh, Transform::IDENTITY)];
    let mut renderer =
        GpuRenderer::new(gpu.clone(), wgpu::TextureFormat::Rgba8Unorm, &scene).unwrap();
    let target = OffscreenTarget::new(gpu, 128, 128).unwrap();
    target.render(&mut renderer, &camera).unwrap();
    let image = target.readback().unwrap();
    assert!(image.depth.iter().filter(|z| **z < 1.0).count() > 1000);
    let comparison = support::compare(&support::cpu_image(&scene, &camera, 128, 128), &image);
    assert!(comparison.passes(), "{comparison:?}");
}
