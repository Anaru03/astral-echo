use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use game::render_bridge::{
    academic_planet_transform, BUILDING_PLANET_MESH, PLANET_VISUAL_RADIUS,
};
use render_core::{
    Camera, LightingMode, MaterialInfo, Projection, RenderLight, RenderSnapshot,
    SnapshotLight, SnapshotNode, SnapshotScene, Transform, Vec3, VisualAssets,
};
use render_gpu::{GpuContext, GpuError, GpuOptions, GpuRenderer, OffscreenTarget};

const MODEL_PATH: &str = "assets/models/building/building_planet.obj";
const MODEL_CENTER: Vec3 = Vec3::new(0.022138, 0.575, -0.0159805);
const MODEL_MAX_DIMENSION: f32 = 15.45;
const WIDTH: u32 = 800;
const HEIGHT: u32 = 600;

#[derive(Clone, Copy)]
struct View {
    name: &'static str,
    camera_position: Vec3,
}

fn views() -> [View; 2] {
    [
        View {
            name: "main",
            camera_position: Vec3::new(0.0, 0.0, 30.0),
        },
        View {
            name: "orbit",
            camera_position: Vec3::new(19.442222, 12.0, 19.442222),
        },
    ]
}

fn camera(view: View) -> Result<Camera, Box<dyn std::error::Error>> {
    let target = academic_planet_transform(0.0).translation;
    Camera::new(
        target + view.camera_position,
        target,
        Vec3::Y,
        Projection::Perspective {
            fov_y_rad: 45.0_f32.to_radians(),
            aspect: WIDTH as f32 / HEIGHT as f32,
            near: 0.1,
            far: 100.0,
        },
    )
    .map_err(|error| format!("invalid validation camera: {error:?}").into())
}

fn building_planet_transform() -> Transform {
    let mut transform = academic_planet_transform(0.0);
    let scale = (PLANET_VISUAL_RADIUS * 2.0) / MODEL_MAX_DIMENSION;
    transform.scale = Vec3::splat(scale);
    transform.translation -= transform.rotation * (transform.scale * MODEL_CENTER);
    transform
}

fn snapshot(view: View) -> Result<RenderSnapshot, Box<dyn std::error::Error>> {
    let camera = camera(view)?;
    let mut snapshot = RenderSnapshot::new(
        engine::Tick(0),
        camera.position(),
        camera.view_matrix(),
        camera.projection_matrix(),
    );
    snapshot.push_node(SnapshotNode::destination(
        9_002,
        building_planet_transform(),
        BUILDING_PLANET_MESH,
        VisualAssets::PLANET,
        true,
    ));
    snapshot.push_light(SnapshotLight::new(RenderLight {
        color: Vec3::ONE,
        intensity: 1.2,
        kind: render_core::RenderLightKind::Directional {
            direction: Vec3::new(-0.35, -0.8, -0.2).normalize(),
        },
        casts_shadow: false,
    }));
    Ok(snapshot)
}

fn scene(
    mesh: &render_core::RasterMesh,
    snapshot: &RenderSnapshot,
) -> Result<SnapshotScene, Box<dyn std::error::Error>> {
    let mut scene = SnapshotScene::default();
    scene.register_mesh(BUILDING_PLANET_MESH, VisualAssets::PLANET, mesh.clone());
    scene.set_lighting_mode(LightingMode::Pbr);
    scene.set_material(VisualAssets::PLANET, MaterialInfo::planet());
    scene.update(snapshot)?;
    Ok(scene)
}

fn output_path(dir: &Path, backend: &str, view: View) -> PathBuf {
    dir.join(format!("building_planet_{backend}_{}.png", view.name))
}

fn ensure_new(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if path.exists() {
        return Err(format!("refusing to overwrite existing output {}", path.display()).into());
    }
    Ok(())
}

fn render_cpu(
    mesh: &render_core::RasterMesh,
    output_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    for view in views() {
        let snapshot = snapshot(view)?;
        let mut scene = scene(mesh, &snapshot)?;
        let mut framebuffer = render_cpu::Framebuffer::new(WIDTH, HEIGHT)?;
        let path = output_path(output_dir, "cpu", view);
        ensure_new(&path)?;
        let started = Instant::now();
        let (prepared, pipeline, preparation_ns) =
            render_cpu::render_snapshot_scene_with_timing(
                &mut framebuffer,
                &scene,
                render_cpu::RasterMode::Secuencial,
            )?;
        let elapsed_ms = started.elapsed().as_secs_f64() * 1_000.0;
        render_cpu::write_png(&path, &framebuffer)?;
        println!(
            "CPU view={} path={} resolution={}x{} elapsed_ms={:.3} preparation_ns={} triangles={} vertices_in={} culled={} degenerate={} pixels_written={}",
            view.name,
            path.display(),
            WIDTH,
            HEIGHT,
            elapsed_ms,
            preparation_ns,
            prepared.triangles,
            pipeline.metrics.vertices_in,
            pipeline.metrics.triangles_culled,
            pipeline.metrics.triangles_degenerate,
            pipeline.metrics.pixels_written,
        );
        scene.update(&snapshot)?;
    }
    Ok(())
}

fn render_gpu(
    mesh: &render_core::RasterMesh,
    output_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let gpu = match GpuContext::new(GpuOptions::default()) {
        Ok(gpu) => Arc::new(gpu),
        Err(GpuError::Unavailable(reason)) => {
            println!("GPU unavailable: {reason}");
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };
    println!("GPU adapter={:?}", gpu.adapter_info());
    for view in views() {
        let snapshot = snapshot(view)?;
        let scene = scene(mesh, &snapshot)?;
        let prepared = scene.prepare(render_core::CullingConfig {
            width: WIDTH,
            height: HEIGHT,
            ..Default::default()
        });
        let mut renderer = GpuRenderer::from_prepared(gpu.clone(), &prepared)?;
        let target = OffscreenTarget::new(gpu.clone(), WIDTH, HEIGHT)?;
        let path = output_path(output_dir, "gpu", view);
        ensure_new(&path)?;
        let started = Instant::now();
        target.render_prepared(&mut renderer, &prepared, scene.view_projection())?;
        gpu.wait()?;
        let image = target.readback()?;
        let elapsed_ms = started.elapsed().as_secs_f64() * 1_000.0;
        image.write_png(&path)?;
        println!(
            "GPU view={} path={} resolution={}x{} elapsed_ms={:.3} triangles={} draws={}",
            view.name,
            path.display(),
            WIDTH,
            HEIGHT,
            elapsed_ms,
            prepared.triangles,
            prepared.stats().draws,
        );
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("usage: building_planet_validation <output-directory>")?;
    std::fs::create_dir_all(&output_dir)?;
    let model_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../").join(MODEL_PATH);
    let load_started = Instant::now();
    let mesh = render_core::load_obj(&model_path)?;
    println!(
        "OBJ path={} load_ms={:.3} vertices={} indices={} triangles={}",
        model_path.display(),
        load_started.elapsed().as_secs_f64() * 1_000.0,
        mesh.vertices.len(),
        mesh.indices.len(),
        mesh.indices.len() / 3,
    );
    render_cpu(&mesh, &output_dir)?;
    render_gpu(&mesh, &output_dir)?;
    Ok(())
}