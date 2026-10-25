use std::hint::black_box;
use std::time::Instant;

use render_core::{RenderSnapshot, SnapshotScene};

const WARMUP: usize = 100;
const ITERATIONS: usize = 1_000;

fn main() {
    let snapshot = RenderSnapshot::minimal_demo();
    let expected_nodes = snapshot.nodes.len();
    assert_eq!(expected_nodes, 2);

    let initial = measure_initial_scene();
    let reconstruction = measure_reconstruction(&snapshot);
    let update = measure_reused_updates(&snapshot);

    println!("snapshot_nodes={expected_nodes}");
    print_result("initial_scene", initial);
    print_result("reconstruction_pattern", reconstruction);
    print_result("reused_update_pattern", update);
    println!(
        "reconstruction_vs_reuse_speedup={:.2}x",
        reconstruction.average_ns() / update.average_ns()
    );
}

#[derive(Clone, Copy)]
struct Measurement {
    iterations: usize,
    total_ns: u128,
    checksum: usize,
}

impl Measurement {
    fn average_ns(self) -> f64 {
        self.total_ns as f64 / self.iterations as f64
    }
}

fn measure_initial_scene() -> Measurement {
    for _ in 0..WARMUP {
        black_box(SnapshotScene::default());
    }

    let start = Instant::now();
    let mut checksum = 0;
    for _ in 0..ITERATIONS {
        checksum += black_box(SnapshotScene::default()).snapshot().nodes.len();
    }
    Measurement {
        iterations: ITERATIONS,
        total_ns: start.elapsed().as_nanos(),
        checksum,
    }
}

fn measure_reconstruction(snapshot: &RenderSnapshot) -> Measurement {
    for _ in 0..WARMUP {
        let mut scene = SnapshotScene::default();
        scene.update(snapshot).expect("fixture must be valid");
        black_box(scene.snapshot().nodes.len());
    }

    let start = Instant::now();
    let mut checksum = 0;
    for _ in 0..ITERATIONS {
        let mut scene = SnapshotScene::default();
        scene.update(snapshot).expect("fixture must be valid");
        checksum += black_box(scene.snapshot().nodes.len());
    }
    Measurement {
        iterations: ITERATIONS,
        total_ns: start.elapsed().as_nanos(),
        checksum,
    }
}

fn measure_reused_updates(snapshot: &RenderSnapshot) -> Measurement {
    let mut scene = SnapshotScene::default();
    scene.update(snapshot).expect("fixture must be valid");
    for _ in 0..WARMUP {
        scene.update(snapshot).expect("fixture must be valid");
        black_box(scene.snapshot().nodes.len());
    }

    let start = Instant::now();
    let mut checksum = 0;
    for _ in 0..ITERATIONS {
        scene.update(snapshot).expect("fixture must be valid");
        checksum += black_box(scene.snapshot().nodes.len());
    }
    Measurement {
        iterations: ITERATIONS,
        total_ns: start.elapsed().as_nanos(),
        checksum,
    }
}

fn print_result(name: &str, measurement: Measurement) {
    println!(
        "{name}: iterations={}, total_ns={}, average_ns={:.2}, checksum={}",
        measurement.iterations,
        measurement.total_ns,
        measurement.average_ns(),
        measurement.checksum
    );
}