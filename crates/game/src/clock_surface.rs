use std::fs;
use std::path::Path;
use std::sync::OnceLock;

use render_core::Vec3;

#[derive(Clone, Copy)]
struct Triangle {
    a: Vec3,
    b: Vec3,
    c: Vec3,
}

pub const WALKABLE_CLOCK_RADIUS: f32 = 5.5;

pub struct ClockSurface {
    triangles: Vec<Triangle>,
}

impl ClockSurface {
    fn load() -> Option<Self> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/models/clock/clock_planet.obj");
        let source = fs::read_to_string(&path).ok()?;
        let mut vertices = Vec::new();
        let mut triangles = Vec::new();
        let mut walkable_object = true;
        for line in source.lines() {
            if let Some(name) = line.strip_prefix("o ") {
                // The two crossed orbital rings are part of the visual model,
                // but they are not the clock's walking surface.
                walkable_object = !name.starts_with("ORBITAS_");
                continue;
            }
            if let Some(values) = line.strip_prefix("v ") {
                let values = values
                    .split_whitespace()
                    .filter_map(|value| value.parse::<f32>().ok())
                    .collect::<Vec<_>>();
                if values.len() >= 3 {
                    vertices.push(Vec3::new(values[0], values[1], values[2]));
                }
                continue;
            }
            if !walkable_object {
                continue;
            }
            let Some(face) = line.strip_prefix("f ") else {
                continue;
            };
            let indices = face
                .split_whitespace()
                .filter_map(|value| value.split('/').next())
                .filter_map(|value| value.parse::<isize>().ok())
                .filter_map(|index| {
                    if index > 0 {
                        Some(index as usize - 1)
                    } else {
                        vertices.len().checked_sub(index.unsigned_abs())
                    }
                })
                .collect::<Vec<_>>();
            for triangle_index in 1..indices.len().saturating_sub(1) {
                let [ia, ib, ic] = [indices[0], indices[triangle_index], indices[triangle_index + 1]];
                let (Some(a), Some(b), Some(c)) =
                    (vertices.get(ia), vertices.get(ib), vertices.get(ic))
                else {
                    continue;
                };
                let (a, b, c) = (*a, *b, *c);
            let normal = (b - a).cross(c - a).normalize_or_zero();
            let inside_clock_body = [a, b, c].into_iter().all(|point| {
                point.x * point.x + point.z * point.z
                    <= WALKABLE_CLOCK_RADIUS * WALKABLE_CLOCK_RADIUS
            });
            // The clock is an open mechanical object. Keep both winding
            // directions, but only faces with a meaningful vertical normal;
            // this removes most shafts and vertical walls from the walkable
            // surface while retaining gear teeth and bridges.
                if normal.y.abs() >= 0.2 && inside_clock_body {
                    triangles.push(Triangle { a, b, c });
                }
            }
        }
        (!triangles.is_empty()).then_some(Self { triangles })
    }

    pub fn sample_top(&self, x: f32, z: f32) -> Option<f32> {
        let mut highest = None;
        for triangle in &self.triangles {
            let denominator = (triangle.b.z - triangle.c.z)
                * (triangle.a.x - triangle.c.x)
                + (triangle.c.x - triangle.b.x) * (triangle.a.z - triangle.c.z);
            if denominator.abs() < 0.000001 {
                continue;
            }
            let u = ((triangle.b.z - triangle.c.z) * (x - triangle.c.x)
                + (triangle.c.x - triangle.b.x) * (z - triangle.c.z))
                / denominator;
            let v = ((triangle.c.z - triangle.a.z) * (x - triangle.c.x)
                + (triangle.a.x - triangle.c.x) * (z - triangle.c.z))
                / denominator;
            let w = 1.0 - u - v;
            if u < -0.0001 || v < -0.0001 || w < -0.0001 {
                continue;
            }
            let y = u * triangle.a.y + v * triangle.b.y + w * triangle.c.y;
            if highest.is_none_or(|current: f32| y > current) {
                highest = Some(y);
            }
        }
        highest
    }
}

pub fn shared() -> Option<&'static ClockSurface> {
    static SURFACE: OnceLock<Option<ClockSurface>> = OnceLock::new();
    SURFACE.get_or_init(ClockSurface::load).as_ref()
}

pub fn surface_height(x: f32, z: f32) -> Option<f32> {
    shared()?.sample_top(x, z)
}

#[cfg(test)]
mod tests {
    use super::shared;

    #[test]
    fn clock_model_has_a_walkable_top_surface() {
        let surface = shared().expect("clock model should load");
        assert!(surface.sample_top(0.0, 0.0).is_some());
        assert!(surface.sample_top(6.5, 0.0).is_none());
    }
}
