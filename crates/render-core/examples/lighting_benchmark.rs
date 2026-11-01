use std::hint::black_box;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use render_core::{
    load_obj, shade_surface_sanitized_normalized, LightingEnvironment, LightingMode, MaterialInfo,
    RenderLight, Vec3, Vec4,
};

const WARMUP_REPETITIONS: usize = 2;
const MEASURED_REPETITIONS: usize = 7;

#[derive(Clone, Copy)]
struct Input {
    color: Vec4,
    position: Vec3,
    normal: Vec3,
}

struct Measurement {
    samples: usize,
    repetitions: usize,
    durations: Vec<Duration>,
    checksum: Vec4,
}

fn inputs() -> Result<Vec<Input>, Box<dyn std::error::Error>> {
    let path: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/models/shooter/shooter.obj");
    let mesh = load_obj(path)?;
    let normals = mesh.resolved_normals();
    Ok(mesh
        .vertices
        .iter()
        .zip(normals)
        .map(|(vertex, normal)| Input {
            color: vertex.color,
            position: vertex.position,
            normal,
        })
        .collect())
}

fn environment(lights: &[RenderLight]) -> LightingEnvironment {
    let mut environment = LightingEnvironment::default();
    environment.mode = LightingMode::Lambert;
    environment.camera_position = Vec3::new(0.0, 2.0, 12.0);
    environment.ambient = Vec3::splat(0.03);
    environment.set_lights(lights.iter().copied());
    environment
}

fn measure(environment: &LightingEnvironment, material: MaterialInfo, values: &[Input]) -> Measurement {
    for _ in 0..WARMUP_REPETITIONS {
        let mut checksum = Vec4::ZERO;
        for input in values {
            checksum += shade_surface_sanitized_normalized(
                environment,
                material,
                input.color,
                input.position,
                input.normal,
            );
        }
        black_box(checksum);
    }

    let mut durations = Vec::with_capacity(MEASURED_REPETITIONS);
    let mut checksum = Vec4::ZERO;
    for _ in 0..MEASURED_REPETITIONS {
        let started = Instant::now();
        let mut batch_checksum = Vec4::ZERO;
        for input in values {
            batch_checksum += shade_surface_sanitized_normalized(
                environment,
                material,
                input.color,
                input.position,
                input.normal,
            );
        }
        durations.push(started.elapsed());
        checksum += black_box(batch_checksum);
    }

    Measurement {
        samples: values.len(),
        repetitions: MEASURED_REPETITIONS,
        durations,
        checksum: black_box(checksum),
    }
}

fn median(values: &[Duration]) -> Duration {
    let mut nanos: Vec<_> = values.iter().map(Duration::as_nanos).collect();
    nanos.sort_unstable();
    Duration::from_nanos(nanos[nanos.len() / 2] as u64)
}

fn print_result(name: &str, measurement: &Measurement) {
    let total = measurement
        .durations
        .iter()
        .map(Duration::as_secs_f64)
        .sum::<f64>();
    let min = measurement.durations.iter().min().unwrap().as_secs_f64();
    let max = measurement.durations.iter().max().unwrap().as_secs_f64();
    let average = total / measurement.repetitions as f64;
    let median = median(&measurement.durations).as_secs_f64();
    let per_call_ns = average * 1_000_000_000.0 / measurement.samples as f64;
    println!(
        "config={name} samples={} repetitions={} total_ms={:.3} average_ms={:.3} median_ms={:.3} min_ms={:.3} max_ms={:.3} average_ns_per_call={:.3} checksum={:?}",
        measurement.samples,
        measurement.repetitions,
        total * 1000.0,
        average * 1000.0,
        median * 1000.0,
        min * 1000.0,
        max * 1000.0,
        per_call_ns,
        measurement.checksum,
    );
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let values = inputs()?;
    let material = MaterialInfo::default().sanitized();
    let point = RenderLight::demo_point();
    let directional = RenderLight::demo_sun();

    println!(
        "lighting_benchmark source=assets/models/shooter/shooter.obj samples={} warmup_repetitions={} measured_repetitions={} mode=Lambert material=MaterialInfo::default lights=demo_point_then_demo_sun",
        values.len(), WARMUP_REPETITIONS, MEASURED_REPETITIONS
    );
    print_result("none", &measure(&environment(&[]), material, &values));
    print_result(
        "directional",
        &measure(&environment(&[directional]), material, &values),
    );
    print_result("point", &measure(&environment(&[point]), material, &values));
    print_result(
        "both",
        &measure(&environment(&[point, directional]), material, &values),
    );
    Ok(())
}