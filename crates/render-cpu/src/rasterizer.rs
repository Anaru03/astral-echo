use glam::{Mat4, Vec3, Vec4};

use crate::clip::{clip_polygon_near, ClipVertex};
use crate::color::vec4_to_rgba8;
use crate::error::RasterError;
use crate::framebuffer::Framebuffer;
use crate::geometry::{RasterMesh, RasterVertex};
use crate::viewport::Viewport;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Winding {
    Both,
    Front,
    Back,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RasterConfig {
    pub winding: Winding,
    pub depth_test: bool,
    pub depth_write: bool,
}

impl Default for RasterConfig {
    fn default() -> Self {
        Self {
            winding: Winding::Both,
            depth_test: true,
            depth_write: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ScreenVertex {
    x: f32,
    y: f32,
    z: f32,
    inv_w: f32,
    color: Vec4,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RasterStats {
    pub triangles: u32,
    pub triangles_clipped: u32,
    pub triangles_culled: u32,
    pub triangles_degenerate: u32,
    pub triangles_offscreen: u32,
    pub triangles_drawn: u32,
    pub pixels_written: u32,
    pub pixels_depth_rejected: u32,
}

pub struct Rasterizer<'a> {
    framebuffer: &'a mut Framebuffer,
    viewport: Viewport,
    config: RasterConfig,
    pub stats: RasterStats,
}

impl<'a> Rasterizer<'a> {
    pub fn new(framebuffer: &'a mut Framebuffer, viewport: Viewport, config: RasterConfig) -> Self {
        Self {
            framebuffer,
            viewport,
            config,
            stats: RasterStats::default(),
        }
    }

    pub fn framebuffer(&self) -> &Framebuffer {
        self.framebuffer
    }

    pub fn draw_mesh(&mut self, mesh: &RasterMesh, mvp: Mat4) -> Result<(), RasterError> {
        for (i0, i1, i2) in mesh.triangle_indices() {
            let a = mesh.vertices.get(i0 as usize);
            let b = mesh.vertices.get(i1 as usize);
            let c = mesh.vertices.get(i2 as usize);
            let (a, b, c) = match (a, b, c) {
                (Some(a), Some(b), Some(c)) => (a, b, c),
                _ => continue,
            };
            self.draw_triangle(a, b, c, mvp)?;
        }
        Ok(())
    }

    pub fn draw_triangle(
        &mut self,
        a: &RasterVertex,
        b: &RasterVertex,
        c: &RasterVertex,
        mvp: Mat4,
    ) -> Result<(), RasterError> {
        self.stats.triangles += 1;
        let clip = [
            ClipVertex::new(
                mvp * Vec4::new(a.position.x, a.position.y, a.position.z, 1.0),
                a.color,
            ),
            ClipVertex::new(
                mvp * Vec4::new(b.position.x, b.position.y, b.position.z, 1.0),
                b.color,
            ),
            ClipVertex::new(
                mvp * Vec4::new(c.position.x, c.position.y, c.position.z, 1.0),
                c.color,
            ),
        ];
        for v in clip.iter() {
            if !v.is_finite() {
                return Err(RasterError::InvalidClip);
            }
        }
        let clipped = clip_polygon_near(&clip)?;
        if clipped.len() < 3 {
            self.stats.triangles_clipped += 1;
            return Ok(());
        }
        if clipped.len() != clip.len() {
            self.stats.triangles_clipped += 1;
        }
        for i in 1..clipped.len() - 1 {
            self.raster_triangle(&clipped[0], &clipped[i], &clipped[i + 1]);
        }
        Ok(())
    }

    fn to_screen(&self, v: &ClipVertex) -> Option<ScreenVertex> {
        if v.clip.w.abs() < f32::EPSILON {
            return None;
        }
        let inv_w = 1.0 / v.clip.w;
        let ndc_x = v.clip.x * inv_w;
        let ndc_y = v.clip.y * inv_w;
        let ndc_z = v.clip.z * inv_w;
        if !ndc_x.is_finite() || !ndc_y.is_finite() || !ndc_z.is_finite() {
            return None;
        }
        let (x, y) = self.viewport.ndc_to_pixel(ndc_x, ndc_y);
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        let z = ndc_z.clamp(0.0, 1.0);
        Some(ScreenVertex {
            x,
            y,
            z,
            inv_w,
            color: v.color,
        })
    }

    fn raster_triangle(&mut self, a: &ClipVertex, b: &ClipVertex, c: &ClipVertex) {
        let va = match self.to_screen(a) {
            Some(v) => v,
            None => {
                self.stats.triangles_degenerate += 1;
                return;
            }
        };
        let vb = match self.to_screen(b) {
            Some(v) => v,
            None => {
                self.stats.triangles_degenerate += 1;
                return;
            }
        };
        let vc = match self.to_screen(c) {
            Some(v) => v,
            None => {
                self.stats.triangles_degenerate += 1;
                return;
            }
        };
        let area = edge_function(va, vb, vc);
        if !area.is_finite() || area.abs() < 1e-8 {
            self.stats.triangles_degenerate += 1;
            return;
        }
        let front_facing = area < 0.0;
        match self.config.winding {
            Winding::Front => {
                if !front_facing {
                    self.stats.triangles_culled += 1;
                    return;
                }
            }
            Winding::Back => {
                if front_facing {
                    self.stats.triangles_culled += 1;
                    return;
                }
            }
            Winding::Both => {}
        }
        let min_x = va.x.min(vb.x).min(vc.x);
        let max_x = va.x.max(vb.x).max(vc.x);
        let min_y = va.y.min(vb.y).min(vc.y);
        let max_y = va.y.max(vb.y).max(vc.y);
        if !min_x.is_finite() || !max_x.is_finite() || !min_y.is_finite() || !max_y.is_finite() {
            self.stats.triangles_degenerate += 1;
            return;
        }
        let fb_w = self.framebuffer.width() as f32;
        let fb_h = self.framebuffer.height() as f32;
        let x0 = min_x.max(0.0).floor() as i64;
        let x1 = max_x.min(fb_w - 1.0).ceil() as i64;
        let y0 = min_y.max(0.0).floor() as i64;
        let y1 = max_y.min(fb_h - 1.0).ceil() as i64;
        if x1 < x0 || y1 < y0 {
            self.stats.triangles_offscreen += 1;
            return;
        }
        self.stats.triangles_drawn += 1;
        for py in y0..=y1 {
            let sy = (py as f32) + 0.5;
            for px in x0..=x1 {
                let sx = (px as f32) + 0.5;
                let w0 = edge_function(vb, vc, ScreenVertex { x: sx, y: sy, ..va });
                let w1 = edge_function(vc, va, ScreenVertex { x: sx, y: sy, ..va });
                let w2 = edge_function(va, vb, ScreenVertex { x: sx, y: sy, ..va });
                let inside = if front_facing {
                    w0 <= 0.0 && w1 <= 0.0 && w2 <= 0.0
                } else {
                    w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0
                };
                if !inside {
                    continue;
                }
                let denom = w0 + w1 + w2;
                if denom.abs() < f32::EPSILON {
                    continue;
                }
                let b0 = w0 / denom;
                let b1 = w1 / denom;
                let b2 = w2 / denom;
                let depth = b0 * va.z + b1 * vb.z + b2 * vc.z;
                if !depth.is_finite() {
                    continue;
                }
                let depth = depth.clamp(0.0, 1.0);
                let ux = px as u32;
                let uy = py as u32;
                if self.config.depth_test {
                    let existing = self.framebuffer.depth_at(ux, uy).unwrap_or(1.0);
                    if depth >= existing {
                        self.stats.pixels_depth_rejected += 1;
                        continue;
                    }
                }
                let inv_w = b0 * va.inv_w + b1 * vb.inv_w + b2 * vc.inv_w;
                if inv_w.abs() < f32::EPSILON {
                    continue;
                }
                let w0p = b0 * va.inv_w / inv_w;
                let w1p = b1 * vb.inv_w / inv_w;
                let w2p = b2 * vc.inv_w / inv_w;
                let color = va.color * w0p + vb.color * w1p + vc.color * w2p;
                let rgba = vec4_to_rgba8(color);
                self.framebuffer.set_pixel(ux, uy, rgba);
                if self.config.depth_write {
                    self.framebuffer.set_depth(ux, uy, depth);
                }
                self.stats.pixels_written += 1;
            }
        }
    }
}

fn edge_function(a: ScreenVertex, b: ScreenVertex, c: ScreenVertex) -> f32 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

pub fn normal_of(a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    let n = (b - a).cross(c - a);
    if n.length_squared() > f32::EPSILON {
        n.normalize()
    } else {
        Vec3::Z
    }
}
