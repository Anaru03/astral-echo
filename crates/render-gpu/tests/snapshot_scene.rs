use std::sync::Arc;

use render_core::{RenderSnapshot, SnapshotNode, SnapshotScene, Transform, Vec3};
use render_gpu::{GpuContext, GpuError, GpuRenderer, OffscreenTarget};

#[test]
fn snapshot_updates_preserve_buffers_until_topology_changes() {
    let gpu = match GpuContext::new(Default::default()) {
        Ok(gpu) => Arc::new(gpu),
        Err(GpuError::Unavailable(reason)) => {
            eprintln!("SKIP GPU: {reason}");
            return;
        }
        Err(error) => panic!("{error}"),
    };
    let mut snapshot = RenderSnapshot::minimal_demo();
    let mut scene = SnapshotScene::default();
    scene.update(&snapshot).unwrap();
    let mut renderer = GpuRenderer::from_snapshot(gpu.clone(), &scene).unwrap();
    let stats = renderer.resources();
    let target = OffscreenTarget::new(gpu, 321, 181).unwrap();
    target.render_snapshot(&mut renderer, &scene).unwrap();
    let first = target.readback().unwrap();
    snapshot.nodes[0].transform.translation += Vec3::new(8.0, 0.0, -3.0);
    snapshot.nodes.reverse();
    scene.update(&snapshot).unwrap();
    assert!(!renderer.sync_snapshot(&scene).unwrap());
    let moved = renderer.resources();
    assert_eq!(
        (moved.vertex_bytes, moved.index_bytes, moved.uniform_bytes),
        (stats.vertex_bytes, stats.index_bytes, stats.uniform_bytes)
    );
    target.render_snapshot(&mut renderer, &scene).unwrap();
    assert_ne!(target.readback().unwrap().rgba, first.rgba);
    let mut added = SnapshotNode::destination(
        999,
        Transform::from_translation(Vec3::new(-8.0, 0.0, -8.0)),
        render_core::MeshHandle::DESTINATION_PLACEHOLDER,
        render_core::VisualAssets::DESTINATION,
        true,
    );
    snapshot.push_node(added);
    scene.update(&snapshot).unwrap();
    assert!(renderer.sync_snapshot(&scene).unwrap());
    assert_eq!(renderer.resources().draw_count, 3);
    added.visible = false;
    snapshot.push_node(added);
    scene.update(&snapshot).unwrap();
    assert!(renderer.sync_snapshot(&scene).unwrap());
    assert_eq!(renderer.resources().draw_count, 2);
    snapshot.nodes.clear();
    scene.update(&snapshot).unwrap();
    assert!(renderer.sync_snapshot(&scene).unwrap());
    target.render_snapshot(&mut renderer, &scene).unwrap();
    let empty = target.readback().unwrap();
    assert!(empty
        .rgba
        .chunks_exact(4)
        .all(|pixel| pixel == render_gpu::CLEAR_RGBA));
    assert!(empty.depth.iter().all(|depth| *depth == 1.0));
}
