use engine::{Duration, Vec3};
use gameplay::{
    Destination, DestinationId, DestinationType, DiscoveryState, EnemyArchetype,
    EnemyConfiguration, GameplayCommand, GameplayProcessor, GameplayWorld, ItemCategory,
    ItemDefinition,
};
use render_core::{Camera, RenderSnapshot, SnapshotScene};
use render_cpu::{Framebuffer, RasterMode};
use render_gpu::GpuImage;

pub fn fixture_world() -> GameplayWorld {
    let mut world = GameplayWorld::new();
    world.start();
    world.create_entity();
    let ship = world.create_entity();
    world.initialize_navigation_ship(ship).unwrap();
    let player = world.create_player().unwrap();
    let mut processor = GameplayProcessor::new();
    processor.add_system(gameplay::systems::PlayerSystem);
    processor.add_system(gameplay::systems::CombatSystem);
    let results = processor.update(
        &mut world,
        [
            GameplayCommand::MovePlayer {
                player,
                direction: Vec3 {
                    x: 1.0,
                    y: 0.0,
                    z: 0.0,
                },
                delta: Duration(1000),
            },
            GameplayCommand::CreateEnemy {
                configuration: EnemyConfiguration::for_archetype(
                    EnemyArchetype::Shooter,
                    Vec3 {
                        x: -10.0,
                        y: 0.0,
                        z: -12.0,
                    },
                ),
            },
        ],
    );
    assert!(results
        .iter()
        .all(|r| matches!(r.status, gameplay::CommandStatus::Success { .. })));
    world
        .register_destination(Destination {
            id: DestinationId::new(1),
            destination_type: DestinationType::Planet,
            position: Vec3 {
                x: 12.0,
                y: 0.0,
                z: -22.0,
            },
            travel_duration: Duration(1000),
            fuel_cost: 1,
            discovery: DiscoveryState::Known,
        })
        .unwrap();
    let item = gameplay::types::ItemId(70);
    world
        .register_item_definition(ItemDefinition::new(item, ItemCategory::MineralSample, 20))
        .unwrap();
    world
        .create_collectible(
            item,
            1,
            Vec3 {
                x: 9.0,
                y: 0.0,
                z: -9.0,
            },
            2.0,
        )
        .unwrap();
    world
}

pub fn snapshot(world: &GameplayWorld, width: u32, height: u32) -> RenderSnapshot {
    let camera = Camera::new(
        render_core::Vec3::new(8.0, 18.0, 26.0),
        render_core::Vec3::new(1.0, 0.0, -10.0),
        render_core::Vec3::Y,
        render_core::Projection::Perspective {
            fov_y_rad: 45.0_f32.to_radians(),
            aspect: width as f32 / height as f32,
            near: 0.1,
            far: 200.0,
        },
    )
    .unwrap();
    game::render_bridge::snapshot_from_world_with_camera(world, Default::default(), &camera)
}

pub fn cpu_image(scene: &SnapshotScene, width: u32, height: u32) -> GpuImage {
    let mut framebuffer = Framebuffer::new(width, height).unwrap();
    render_cpu::render_snapshot_scene(&mut framebuffer, scene, RasterMode::Secuencial).unwrap();
    let rgba = framebuffer.color_bytes().to_vec();
    let depth = framebuffer.depth_slice_mut().to_vec();
    GpuImage {
        width,
        height,
        rgba,
        depth,
    }
}

pub fn compare(cpu: &GpuImage, gpu: &GpuImage) -> (f64, usize, usize) {
    assert_eq!((cpu.width, cpu.height), (gpu.width, gpu.height));
    assert_eq!(cpu.rgba.len(), gpu.rgba.len());
    assert_eq!(cpu.depth.len(), gpu.depth.len());
    let mean = cpu
        .rgba
        .iter()
        .zip(&gpu.rgba)
        .map(|(a, b)| f64::from(a.abs_diff(*b)))
        .sum::<f64>()
        / cpu.rgba.len() as f64;
    let colors = cpu
        .rgba
        .chunks_exact(4)
        .zip(gpu.rgba.chunks_exact(4))
        .filter(|(a, b)| a.iter().zip(*b).any(|(a, b)| a.abs_diff(*b) > 2))
        .count();
    let depths = cpu
        .depth
        .iter()
        .zip(&gpu.depth)
        .filter(|(a, b)| !a.is_finite() || !b.is_finite() || (*a - *b).abs() > 1e-5)
        .count();
    assert!(mean <= 0.25, "mean channel error {mean}");
    assert!(
        colors as f64 / cpu.depth.len() as f64 <= 0.005,
        "color outliers {colors}"
    );
    assert!(
        depths as f64 / cpu.depth.len() as f64 <= 0.005,
        "depth outliers {depths}"
    );
    (mean, colors, depths)
}
