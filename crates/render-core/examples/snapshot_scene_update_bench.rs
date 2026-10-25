use std::hint::black_box;
use std::time::Instant;

use engine::{EntityId, Tick};
use render_core::{
    MeshHandle, RenderSnapshot, SnapshotNode, SnapshotScene, Transform, Vec3, VisualAssets,
};

const WARMUP: usize = 100;
const ITERATIONS: usize = 2_000;
const SIZES: &[usize] = &[16, 64, 256, 1_024];

fn main() {
    for &size in SIZES {
        let base = snapshot(size);
        let identical = base.clone();
        let changed = changed_snapshot(&base);
        let added = added_snapshot(&base);
        let removed = removed_snapshot(&base);
        let several = several_changed_snapshot(&base);

        println!("nodes={size}");
        print_result("identical", measure_single(&base, &identical));
        print_result("one_transform", measure_alternating(&base, &changed));
        print_result("add_remove", measure_alternating(&base, &added));
        print_result("remove_add", measure_alternating(&base, &removed));
        print_result("several_changes", measure_alternating(&base, &several));
    }
}

#[derive(Clone, Copy)]
struct Measurement {
    total_ns: u128,
    operations: usize,
    checksum: usize,
}

impl Measurement {
    fn average_ns(self) -> f64 {
        self.total_ns as f64 / self.operations as f64
    }
}

fn snapshot(size: usize) -> RenderSnapshot {
    let camera = render_core::Camera::demo();
    let nodes = (0..size)
        .map(|index| {
            SnapshotNode::new(
                EntityId::new(1_000 + index as u64),
                Transform::from_translation(Vec3::new(index as f32 * 0.25, 0.0, -20.0)),
                MeshHandle::SHIP_PLACEHOLDER,
                VisualAssets::SHIP,
                true,
            )
        })
        .collect();
    RenderSnapshot {
        tick: Tick(1),
        nodes,
        lights: Vec::new(),
        camera_position: camera.position(),
        view: camera.view_matrix(),
        projection: camera.projection_matrix(),
    }
}

fn changed_snapshot(base: &RenderSnapshot) -> RenderSnapshot {
    let mut changed = base.clone();
    changed.nodes[base.nodes.len() / 2].transform.translation.x += 1.0;
    changed.tick = Tick(2);
    changed
}

fn added_snapshot(base: &RenderSnapshot) -> RenderSnapshot {
    let mut added = base.clone();
    let index = base.nodes.len();
    added.nodes.push(SnapshotNode::new(
        EntityId::new(1_000 + index as u64),
        Transform::from_translation(Vec3::new(index as f32 * 0.25, 0.0, -20.0)),
        MeshHandle::SHIP_PLACEHOLDER,
        VisualAssets::SHIP,
        true,
    ));
    added.tick = Tick(2);
    added
}

fn removed_snapshot(base: &RenderSnapshot) -> RenderSnapshot {
    let mut removed = base.clone();
    removed.nodes.pop();
    removed.tick = Tick(3);
    removed
}

fn several_changed_snapshot(base: &RenderSnapshot) -> RenderSnapshot {
    let mut changed = base.clone();
    for (index, node) in changed.nodes.iter_mut().enumerate().step_by(4) {
        node.transform.translation.y = index as f32 * 0.5;
    }
    changed.tick = Tick(4);
    changed
}

fn measure_single(base: &RenderSnapshot, target: &RenderSnapshot) -> Measurement {
    let mut scene = SnapshotScene::default();
    scene.update(base).expect("base fixture must be valid");
    for _ in 0..WARMUP {
        black_box(scene.update(target).expect("target fixture must be valid"));
    }

    let start = Instant::now();
    let mut checksum: usize = 0;
    for _ in 0..ITERATIONS {
        let update = scene.update(target).expect("target fixture must be valid");
        checksum = checksum
            .wrapping_add(update.added)
            .wrapping_add(update.removed)
            .wrapping_add(update.changed)
            .wrapping_add(black_box(scene.snapshot().nodes.len()));
    }
    Measurement {
        total_ns: start.elapsed().as_nanos(),
        operations: ITERATIONS,
        checksum,
    }
}

fn measure_alternating(left: &RenderSnapshot, right: &RenderSnapshot) -> Measurement {
    let mut scene = SnapshotScene::default();
    scene.update(left).expect("left fixture must be valid");
    for iteration in 0..WARMUP {
        let target = if iteration % 2 == 0 { right } else { left };
        black_box(scene.update(target).expect("alternating fixture must be valid"));
    }

    let start = Instant::now();
    let mut checksum: usize = 0;
    for iteration in 0..ITERATIONS {
        let target = if iteration % 2 == 0 { right } else { left };
        let update = scene.update(target).expect("alternating fixture must be valid");
        checksum = checksum
            .wrapping_add(update.added)
            .wrapping_add(update.removed)
            .wrapping_add(update.changed)
            .wrapping_add(black_box(scene.snapshot().nodes.len()));
    }
    Measurement {
        total_ns: start.elapsed().as_nanos(),
        operations: ITERATIONS,
        checksum,
    }
}

fn print_result(name: &str, measurement: Measurement) {
    println!(
        "{name}: iterations={}, total_ns={}, average_ns={:.2}, checksum={}",
        measurement.operations,
        measurement.total_ns,
        measurement.average_ns(),
        measurement.checksum
    );
}