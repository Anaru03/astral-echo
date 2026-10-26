#[path = "visual_support/mod.rs"]
#[allow(dead_code)]
mod support;

use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};

use game::render_bridge::{snapshot_from_world, BridgeParams, SURFACE_OBJECT_IDS};
use gameplay::DestinationId;
use render_core::SnapshotId;

const WARMUP_ITERATIONS: usize = 100;
const MEASURED_ITERATIONS: usize = 1_000;

struct CountingAllocator;

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static ALLOCATED_BYTES: AtomicUsize = AtomicUsize::new(0);
static REALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static REALLOCATED_BYTES: AtomicUsize = AtomicUsize::new(0);

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = System.alloc(layout);
        if !pointer.is_null() {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            ALLOCATED_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        System.dealloc(pointer, layout);
    }

    unsafe fn realloc(
        &self,
        pointer: *mut u8,
        layout: Layout,
        new_size: usize,
    ) -> *mut u8 {
        let new_pointer = System.realloc(pointer, layout, new_size);
        if !new_pointer.is_null() {
            REALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            REALLOCATED_BYTES.fetch_add(new_size, Ordering::Relaxed);
        }
        new_pointer
    }
}

#[derive(Default)]
struct Counters {
    allocations: usize,
    allocated_bytes: usize,
    reallocations: usize,
    reallocated_bytes: usize,
}

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

    diagnose("academic_with_surface_objects", &world, full_params);
    diagnose("academic_surface_objects_filtered", &world, filtered_params);

    assert_eq!(before, format!("{world:?}"));
    println!(
        "iterations: warmup={WARMUP_ITERATIONS}, measured={MEASURED_ITERATIONS}"
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

fn diagnose(name: &str, world: &gameplay::GameplayWorld, params: BridgeParams<'_>) {
    for _ in 0..WARMUP_ITERATIONS {
        black_box(snapshot_from_world(world, params));
    }

    let mut total = Counters::default();
    let mut node_len = 0;
    let mut node_capacity = 0;
    let mut light_len = 0;
    let mut light_capacity = 0;
    for _ in 0..MEASURED_ITERATIONS {
        reset_counters();
        let snapshot = snapshot_from_world(world, params);
        node_len = snapshot.nodes.len();
        node_capacity = snapshot.nodes.capacity();
        light_len = snapshot.lights.len();
        light_capacity = snapshot.lights.capacity();
        let counters = read_counters();
        total.allocations += counters.allocations;
        total.allocated_bytes += counters.allocated_bytes;
        total.reallocations += counters.reallocations;
        total.reallocated_bytes += counters.reallocated_bytes;
        black_box(snapshot);
    }

    println!(
        "{name}: nodes={node_len}/{node_capacity}, lights={light_len}/{light_capacity}, "
    );
    println!(
        "  avg_alloc_calls={:.2}, avg_allocated_bytes={:.2}, avg_realloc_calls={:.2}, avg_reallocated_bytes={:.2}",
        total.allocations as f64 / MEASURED_ITERATIONS as f64,
        total.allocated_bytes as f64 / MEASURED_ITERATIONS as f64,
        total.reallocations as f64 / MEASURED_ITERATIONS as f64,
        total.reallocated_bytes as f64 / MEASURED_ITERATIONS as f64,
    );
}

fn reset_counters() {
    for counter in [
        &ALLOCATIONS,
        &ALLOCATED_BYTES,
        &REALLOCATIONS,
        &REALLOCATED_BYTES,
    ] {
        counter.store(0, Ordering::Relaxed);
    }
}

fn read_counters() -> Counters {
    Counters {
        allocations: ALLOCATIONS.load(Ordering::Relaxed),
        allocated_bytes: ALLOCATED_BYTES.load(Ordering::Relaxed),
        reallocations: REALLOCATIONS.load(Ordering::Relaxed),
        reallocated_bytes: REALLOCATED_BYTES.load(Ordering::Relaxed),
    }
}