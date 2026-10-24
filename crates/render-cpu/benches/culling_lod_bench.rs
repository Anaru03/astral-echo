use std::hint::black_box;
use std::time::Instant;

use render_core::{CullingConfig, RenderSnapshot, SnapshotId, SnapshotScene, Vec3};
use render_cpu::{render_prepared_scene, render_snapshot_scene_reference, Framebuffer, RasterMode};

fn main() {
    println!("scene,path,considered,visible,discarded,triangles,prepare_us,frame_ms");
    for (name, count, outside) in [
        ("mixed", 1000, true),
        ("visible", 1000, false),
        ("small", 2, false),
    ] {
        let mut snapshot = RenderSnapshot::minimal_demo();
        let node = snapshot.nodes[0];
        snapshot.nodes.clear();
        for index in 0..count {
            let mut node = node;
            node.entity = SnapshotId::Destination(index);
            node.transform.translation = Vec3::new(
                if outside && index % 5 != 0 {
                    10000.0
                } else {
                    (index % 25) as f32 * 4.0 - 48.0
                },
                0.0,
                -((index / 25) as f32 * 15.0) - 50.0,
            );
            snapshot.nodes.push(node);
        }
        let mut scene = SnapshotScene::procedural_lod_fixture();
        scene.update(&snapshot).unwrap();
        let mut reference = Framebuffer::new(320, 180).unwrap();
        render_snapshot_scene_reference(&mut reference, &scene, RasterMode::Secuencial).unwrap();
        let mut f6 = SnapshotScene::default();
        f6.update(&snapshot).unwrap();
        for path in ["f6", "f7", "reference", "culling", "culling_lod"] {
            let scene = if path == "f6" || path == "f7" {
                &f6
            } else {
                &scene
            };
            let config = CullingConfig {
                width: 320,
                height: 180,
                low_threshold_px: if path == "culling" { 0.0 } else { 12.0 },
                high_threshold_px: if path == "culling" { 0.0 } else { 18.0 },
            };
            let mut framebuffer = Framebuffer::new(320, 180).unwrap();
            let mut prepare_us = 0.0;
            let mut frame_ms = 0.0;
            let mut counters = (count as usize, count as usize, 0, 0);
            for iteration in 0..65 {
                let frame_start = Instant::now();
                let start = Instant::now();
                if path == "reference" || path == "f6" {
                    let draws: Vec<_> = scene.draws().collect();
                    let triangles = draws.iter().map(|(_, mesh)| mesh.indices.len() / 3).sum();
                    black_box(&draws);
                    counters.3 = triangles;
                    if iteration >= 5 {
                        prepare_us += start.elapsed().as_secs_f64() * 1e6;
                    }
                    render_snapshot_scene_reference(
                        &mut framebuffer,
                        scene,
                        RasterMode::Secuencial,
                    )
                    .unwrap();
                } else {
                    let prepared = scene.prepare(config);
                    if iteration >= 5 {
                        prepare_us += start.elapsed().as_secs_f64() * 1e6;
                    }
                    counters = (
                        prepared.considered,
                        prepared.visible,
                        prepared.discarded,
                        prepared.triangles,
                    );
                    render_prepared_scene(
                        &mut framebuffer,
                        &prepared,
                        scene.view_projection(),
                        RasterMode::Secuencial,
                    )
                    .unwrap();
                }
                if iteration >= 5 {
                    frame_ms += frame_start.elapsed().as_secs_f64() * 1000.0;
                }
                black_box(&framebuffer);
            }
            let max_color = reference
                .color_bytes()
                .iter()
                .zip(framebuffer.color_bytes())
                .map(|(a, b)| a.abs_diff(*b))
                .max()
                .unwrap();
            let max_depth = reference
                .depth_slice_mut()
                .iter()
                .zip(framebuffer.depth_slice_mut().iter())
                .map(|(a, b)| (a - b).abs())
                .fold(0.0, f32::max);
            assert!(max_color <= 1, "{name}/{path} color {max_color}");
            assert!(max_depth < 0.00001, "{name}/{path} depth {max_depth}");
            println!(
                "{name},{path},{},{},{},{},{:.3},{:.3}",
                counters.0,
                counters.1,
                counters.2,
                counters.3,
                prepare_us / 60.0,
                frame_ms / 60.0
            );
        }
    }
}
