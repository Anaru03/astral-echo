use glam::Vec4;

use crate::error::RasterError;

pub const NEAR_EPS: f32 = 1e-6;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClipVertex {
    pub clip: Vec4,
    pub color: Vec4,
}

impl ClipVertex {
    pub const fn new(clip: Vec4, color: Vec4) -> Self {
        Self { clip, color }
    }

    pub fn is_finite(&self) -> bool {
        self.clip.is_finite() && self.color.is_finite()
    }

    pub fn is_in_front(&self) -> bool {
        self.clip.w > NEAR_EPS
    }
}

fn interpolate(a: &ClipVertex, b: &ClipVertex, t: f32) -> ClipVertex {
    ClipVertex::new(
        a.clip + (b.clip - a.clip) * t,
        a.color + (b.color - a.color) * t,
    )
}

pub fn clip_polygon_near(polygon: &[ClipVertex]) -> Result<Vec<ClipVertex>, RasterError> {
    if polygon.len() < 3 {
        return Ok(Vec::new());
    }
    for v in polygon {
        if !v.is_finite() {
            return Err(RasterError::InvalidClip);
        }
    }
    let mut output: Vec<ClipVertex> = Vec::with_capacity(polygon.len() + 1);
    for i in 0..polygon.len() {
        let current = polygon[i];
        let next = polygon[(i + 1) % polygon.len()];
        let current_in = current.is_in_front();
        let next_in = next.is_in_front();
        if current_in {
            output.push(current);
        }
        if current_in != next_in {
            let dc = current.clip.w - NEAR_EPS;
            let dn = next.clip.w - NEAR_EPS;
            let denom = dc - dn;
            if denom.abs() > f32::EPSILON {
                let t = dc / denom;
                let t = t.clamp(0.0, 1.0);
                output.push(interpolate(&current, &next, t));
            }
        }
    }
    Ok(output)
}

pub fn clip_triangle_near(
    a: ClipVertex,
    b: ClipVertex,
    c: ClipVertex,
) -> Result<Vec<ClipVertex>, RasterError> {
    let polygon = [a, b, c];
    clip_polygon_near(&polygon)
}
