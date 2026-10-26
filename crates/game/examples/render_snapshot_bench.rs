#[path = "visual_support/mod.rs"]
#[allow(dead_code)]
mod support;

use std::hint::black_box;
use std::time::{Duration, Instant};

use game::render_bridge::{snapshot_from_world, BridgeParams, SURFACE_OBJECT_IDS};
use gameplay::DestinationId;
use render_core::SnapshotId;

const WARMUP_ITERATIONS: usize = 1_000;
const MEASURED_ITERATIONS: usize = 10_000;

fn main() {
    let world = support::fixture_world();
    let before = format!("{world:?}");

    let full_params = BridgeParams::default();
    let filtered_params = BridgeParams {
        destinations: &[DestinationId::new(999)],
        include_collectibles: false,
    };

    validate_scenario(&world, full_params, 10, true);
    validate_scenario(&world, filtered_params, 4, false);

    measure("academic_with_surface_objects", &world, full_params);
    measure("academic_surface_objects_filtered", &world, filtered_params);

    assert_eq!(before, format!("{world:?}"));
    println!(
        "iterations: warmup={WARMUP_ITERATIONS}, measured={MEASURED_ITERATIONS}, profile=release"
    );
}

fn validate_scenario(
    world: &gameplay::GameplayWorld,
    params: BridgeParams<'_>,
    expected_nodes: usize,
    expects_surface_objects: bool,
) {
    let snapshot = snapshot_from_world(world, params);
    assert_eq!(snapshot.nodes.len(), expected_nodes);
    let surface_count = snapshot
        .nodes
        .iter()
        .filter(|node| {
            matches!(node.entity, SnapshotId::Entity(id) if SURFACE_OBJECT_IDS.contains(&id))
        })
        .count();
    assert_eq!(surface_count, if expects_surface_objects { 4 } else { 0 });
    assert!(snapshot.nodes.iter().all(|node| node.transform.matrix().is_finite()));
    black_box(snapshot);
}

fn measure(name: &str, world: &gameplay::GameplayWorld, params: BridgeParams<'_>) {
    for _ in 0..WARMUP_ITERATIONS {
        black_box(snapshot_from_world(world, params));
    }

    let start = Instant::now();
    let mut checksum = 0usize;
    for _ in 0..MEASURED_ITERATIONS {
        let snapshot = snapshot_from_world(world, params);
        checksum = checksum.wrapping_add(black_box(snapshot.nodes.len()));
        black_box(snapshot);
    }
    let elapsed = start.elapsed();
    let average = elapsed / MEASURED_ITERATIONS as u32;
    print_result(name, elapsed, average, checksum);
}

fn print_result(name: &str, elapsed: Duration, average: Duration, checksum: usize) {
    println!(
        "{name}: total_ns={}, average_ns={}, checksum={checksum}",
        elapsed.as_nanos(),
        average.as_nanos()
    );
}