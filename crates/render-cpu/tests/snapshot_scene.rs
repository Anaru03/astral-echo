use render_core::{CullingConfig, RenderSnapshot, SnapshotScene, Vec3};
use render_cpu::{render_snapshot_scene, Framebuffer, RasterMode};

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
    let scene = SnapshotScene::default();
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
