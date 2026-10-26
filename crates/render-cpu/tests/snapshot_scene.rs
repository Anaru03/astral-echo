use render_core::{CullingConfig, RenderSnapshot, SnapshotScene, Vec3};
use render_cpu::{
    render_snapshot_scene, render_snapshot_scene_with_stats, Framebuffer, RasterMode,
};

#[test]
fn all_cpu_modes_render_the_same_snapshot_and_clear_removed_objects() {
    let mut scene = SnapshotScene::default();
    let mut snapshot = RenderSnapshot::minimal_demo();
    snapshot.nodes[1].transform.translation = Vec3::new(8.0, 0.0, -8.0);
    scene.update(&snapshot).unwrap();
    let mut reference = Framebuffer::new(321, 181).unwrap();
    render_snapshot_scene(&mut reference, &scene, RasterMode::Secuencial).unwrap();
    assert!(reference
        .color_bytes()
        .chunks_exact(4)
        .any(|p| p != [8, 10, 18, 255]));
    for mode in [RasterMode::Paralelo, RasterMode::Auto] {
        let mut framebuffer = Framebuffer::new(321, 181).unwrap();
        render_snapshot_scene(&mut framebuffer, &scene, mode).unwrap();
        assert_eq!(framebuffer, reference);
    }
    snapshot.nodes.clear();
    scene.update(&snapshot).unwrap();
    let stats = render_snapshot_scene(&mut reference, &scene, RasterMode::Secuencial).unwrap();
    assert_eq!(stats.draws, 0);
    assert!(reference
        .color_bytes()
        .chunks_exact(4)
        .all(|p| p == [8, 10, 18, 255]));
    assert!(reference.depth_slice_mut().iter().all(|d| *d == 1.0));
}

#[test]
fn prepared_scene_reports_culling_and_lod_work() {
    let scene = SnapshotScene::procedural_lod_fixture();
    let prepared = scene.prepare(CullingConfig {
        width: 640,
        height: 360,
        low_threshold_px: 8.0,
        high_threshold_px: 20.0,
    });
    assert_eq!(prepared.considered, 0);
    assert_eq!(prepared.visible, 0);
    assert_eq!(prepared.discarded, 0);
}

#[test]
fn snapshot_render_reports_stats_from_its_prepared_scene() {
    let mut scene = SnapshotScene::default();
    scene.update(&RenderSnapshot::minimal_demo()).unwrap();
    let mut framebuffer = Framebuffer::new(321, 181).unwrap();
    let (stats, _) = render_snapshot_scene_with_stats(
        &mut framebuffer,
        &scene,
        RasterMode::Secuencial,
    )
    .unwrap();
    let expected = scene
        .prepare(CullingConfig {
            width: 321,
            height: 181,
            ..Default::default()
        })
        .stats();
    assert_eq!(stats, expected);
}

#[test]
fn game_over_overlay_reaches_a_black_frame_without_drawing_scene_nodes() {
    let mut scene = SnapshotScene::default();
    let mut snapshot = RenderSnapshot::minimal_demo();
    snapshot.overlay.fade_to_black = 1.0;
    snapshot.overlay.game_over = true;
    scene.update(&snapshot).unwrap();

    let mut framebuffer = Framebuffer::new(32, 18).unwrap();
    framebuffer.clear(render_cpu::Color::new(255, 0, 255, 255));
    let (prepared, pipeline) = render_snapshot_scene_with_stats(
        &mut framebuffer,
        &scene,
        RasterMode::Secuencial,
    )
    .unwrap();

    assert_eq!(prepared.visible, 2);
    assert_eq!(pipeline.draws, 0);
    assert!(framebuffer
        .color_bytes()
        .chunks_exact(4)
        .all(|pixel| pixel == [0, 0, 0, 255]));
}

#[test]
fn optimized_color_and_depth_match_reference_with_low_and_partial_objects() {
    let mut scene = SnapshotScene::procedural_lod_fixture();
    let mut snapshot = RenderSnapshot::minimal_demo();
    snapshot.view = render_core::Mat4::IDENTITY;
    snapshot.camera_position = Vec3::ZERO;
    snapshot.projection =
        render_core::Mat4::perspective_rh(std::f32::consts::FRAC_PI_2, 321.0 / 181.0, 1.0, 1000.0);
    snapshot.nodes[0].transform.translation = Vec3::new(18.0, 0.0, -10.0);
    snapshot.nodes[1].transform.translation = Vec3::new(0.0, 0.0, -10.0);

    let mut outside = snapshot.nodes[0];
    outside.entity = render_core::SnapshotId::Destination(100);
    outside.transform.translation.x = 10000.0;
    snapshot.nodes.push(outside);
    let mut far = snapshot.nodes[0];
    far.entity = render_core::SnapshotId::Destination(101);
    far.transform.translation = Vec3::new(0.0, 0.0, -300.0);
    snapshot.nodes.push(far);
    scene.update(&snapshot).unwrap();
    let before = scene.snapshot().clone();
    let prepared = scene.prepare(CullingConfig {
        width: 321,
        height: 181,
        ..Default::default()
    });
    assert!(prepared.discarded > 0);
    assert!(prepared
        .draws()
        .any(|d| d.visibility == render_core::VisibilityClass::Partial));
    assert!(prepared
        .draws()
        .any(|d| d.lod == render_core::LodLevel::Low));
    let mut reference = Framebuffer::new(321, 181).unwrap();
    render_cpu::render_snapshot_scene_reference(&mut reference, &scene, RasterMode::Secuencial)
        .unwrap();
    let mut optimized = Framebuffer::new(321, 181).unwrap();
    render_cpu::render_prepared_scene(
        &mut optimized,
        &prepared,
        scene.view_projection(),
        RasterMode::Secuencial,
    )
    .unwrap();
    let max_color = reference
        .color_bytes()
        .iter()
        .zip(optimized.color_bytes())
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap();
    let max_depth = reference
        .depth_slice_mut()
        .iter()
        .zip(optimized.depth_slice_mut().iter())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f32::max);
    assert!(max_color <= 1, "color delta {max_color}");
    assert!(max_depth < 0.00001, "depth delta {max_depth}");
    assert_eq!(scene.snapshot(), &before);
}