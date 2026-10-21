use rayon::prelude::*;

use crate::framebuffer::Framebuffer;
use crate::pipeline::raster::{edge_function, is_top_left, FillRule, ScreenVertex};

/// Rectángulo de tile en coordenadas de píxel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TileRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl TileRect {
    pub fn contains(&self, x: u32, y: u32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

/// Modo de rasterización.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RasterMode {
    Secuencial,
    Paralelo,
    Auto,
}

/// Configuración de rasterización por tiles.
#[derive(Clone, Copy, Debug)]
pub struct TileConfig {
    pub tile_size: u32,
    pub mode: RasterMode,
}

impl Default for TileConfig {
    fn default() -> Self {
        Self {
            tile_size: 16,
            mode: RasterMode::Auto,
        }
    }
}

/// Rejilla de tiles que divide un framebuffer.
#[derive(Clone, Debug)]
pub struct TileGrid {
    pub fb_w: u32,
    pub fb_h: u32,
    pub tile_size: u32,
    pub tiles_w: u32,
    pub tiles_h: u32,
}

impl TileGrid {
    pub fn new(fb_w: u32, fb_h: u32, tile_size: u32) -> Self {
        let tiles_w = (fb_w + tile_size - 1) / tile_size;
        let tiles_h = (fb_h + tile_size - 1) / tile_size;
        Self {
            fb_w,
            fb_h,
            tile_size,
            tiles_w,
            tiles_h,
        }
    }

    pub fn tile_count(&self) -> u32 {
        self.tiles_w * self.tiles_h
    }

    pub fn tile_rect(&self, tx: u32, ty: u32) -> TileRect {
        let x = tx * self.tile_size;
        let y = ty * self.tile_size;
        let w = self.tile_size.min(self.fb_w.saturating_sub(x));
        let h = self.tile_size.min(self.fb_h.saturating_sub(y));
        TileRect { x, y, w, h }
    }

    /// Devuelve todos los tiles que intersectan el bounding box dado.
    pub fn tiles_intersected_by_bbox(
        &self,
        min_x: f32,
        max_x: f32,
        min_y: f32,
        max_y: f32,
    ) -> Vec<TileRect> {
        if min_x > self.fb_w as f32 || max_x < 0.0 || min_y > self.fb_h as f32 || max_y < 0.0 {
            return Vec::new();
        }
        let tx0 = (min_x.max(0.0).floor() as u32) / self.tile_size;
        let ty0 = (min_y.max(0.0).floor() as u32) / self.tile_size;
        let tx1 = ((max_x.min(self.fb_w as f32 - 1.0).ceil() as u32).saturating_sub(1))
            / self.tile_size;
        let ty1 = ((max_y.min(self.fb_h as f32 - 1.0).ceil() as u32).saturating_sub(1))
            / self.tile_size;

        let mut result = Vec::new();
        for ty in ty0..=ty1.min(self.tiles_h - 1) {
            for tx in tx0..=tx1.min(self.tiles_w - 1) {
                result.push(self.tile_rect(tx, ty));
            }
        }
        result
    }
}

/// Estadísticas agregadas de rasterización por tiles.
#[derive(Clone, Copy, Debug, Default)]
pub struct TileStats {
    pub triangles_binned: u32,
    pub fragments_rasterized: u32,
    pub pixels_written: u32,
    pub bin_ns: u64,
    pub raster_ns: u64,
}

/// Información de un triángulo preparado para binning.
#[derive(Clone, Copy, Debug)]
struct BinnedTriangle {
    pub a: ScreenVertex,
    pub b: ScreenVertex,
    pub c: ScreenVertex,
    pub min_x: i64,
    pub max_x: i64,
    pub min_y: i64,
    pub max_y: i64,
}

/// Calcula el bounding box entero de un triángulo en espacio de pantalla.
fn triangle_bbox(a: &ScreenVertex, b: &ScreenVertex, c: &ScreenVertex) -> (i64, i64, i64, i64) {
    let min_x = a.x.min(b.x).min(c.x).floor() as i64;
    let max_x = a.x.max(b.x).max(c.x).ceil() as i64;
    let min_y = a.y.min(b.y).min(c.y).floor() as i64;
    let max_y = a.y.max(b.y).max(c.y).ceil() as i64;
    (min_x, max_x, min_y, max_y)
}

/// Rasteriza un triángulo dentro de un tile rect, escribiendo directamente
/// en los slices de color y profundidad del framebuffer.
fn raster_triangle_in_tile(
    a: &ScreenVertex,
    b: &ScreenVertex,
    c: &ScreenVertex,
    tile: &TileRect,
    fb_w: u32,
    color: &mut [u8],
    depth: &mut [f32],
    fill_rule: FillRule,
    depth_test: bool,
    depth_write: bool,
    stats: &mut TileStats,
) {
    let area = edge_function(a, b, c);
    if !area.is_finite() || area.abs() < 1e-8 {
        return;
    }

    let (b, c) = if area > 0.0 { (c, b) } else { (b, c) };
    let denom = edge_function(a, b, c);
    if denom.abs() < f32::EPSILON {
        return;
    }

    let (min_x, max_x, min_y, max_y) = triangle_bbox(a, b, c);

    // Limitar al tile y al framebuffer.
    let x0 = min_x.max(tile.x as i64).max(0) as u32;
    let x1 = max_x.min((tile.x + tile.w - 1) as i64).min(fb_w as i64 - 1) as u32;
    let y0 = min_y.max(tile.y as i64).max(0) as u32;
    let y1 = max_y.min((tile.y + tile.h - 1) as i64) as u32;

    if x1 < x0 || y1 < y0 {
        return;
    }

    for py in y0..=y1 {
        let sy = (py as f32) + 0.5;
        for px in x0..=x1 {
            let sx = (px as f32) + 0.5;

            let w0 = edge_function(b, c, &ScreenVertex { x: sx, y: sy, ..*a });
            let w1 = edge_function(c, a, &ScreenVertex { x: sx, y: sy, ..*a });
            let w2 = edge_function(a, b, &ScreenVertex { x: sx, y: sy, ..*a });

            if w0 > 0.0 || w1 > 0.0 || w2 > 0.0 {
                continue;
            }

            if fill_rule == FillRule::TopLeft {
                if w0 == 0.0 && !is_top_left(b, c) {
                    continue;
                }
                if w1 == 0.0 && !is_top_left(c, a) {
                    continue;
                }
                if w2 == 0.0 && !is_top_left(a, b) {
                    continue;
                }
            }

            let b0 = w0 / denom;
            let b1 = w1 / denom;
            let b2 = w2 / denom;

            let depth_val = b0 * a.z + b1 * b.z + b2 * c.z;
            if !depth_val.is_finite() {
                continue;
            }
            let depth_val = depth_val.clamp(0.0, 1.0);

            if depth_test {
                let di = (py as usize) * (fb_w as usize) + (px as usize);
                if depth_val >= depth[di] {
                    stats.fragments_rasterized += 1;
                    continue;
                }
            }

            let inv_w = b0 * a.inv_w + b1 * b.inv_w + b2 * c.inv_w;
            if inv_w.abs() < f32::EPSILON {
                continue;
            }

            let w0p = b0 * a.inv_w / inv_w;
            let w1p = b1 * b.inv_w / inv_w;
            let w2p = b2 * c.inv_w / inv_w;
            let color_val = a.color * w0p + b.color * w1p + c.color * w2p;
            let rgba = crate::color::vec4_to_rgba8(color_val);

            let ci = ((py as usize) * (fb_w as usize) + (px as usize)) * 4;
            color[ci] = rgba.r;
            color[ci + 1] = rgba.g;
            color[ci + 2] = rgba.b;
            color[ci + 3] = rgba.a;

            if depth_write {
                let di = (py as usize) * (fb_w as usize) + (px as usize);
                depth[di] = depth_val;
            }

            stats.fragments_rasterized += 1;
            stats.pixels_written += 1;
        }
    }
}

/// Procesa una banda horizontal de filas del framebuffer.
/// Cada worker recibe slices mutables disjuntos de color y profundidad.
fn process_band(
    triangles: &[BinnedTriangle],
    grid: &TileGrid,
    fb_w: u32,
    band_y_start: u32,
    band_y_end: u32,
    color: &mut [u8],
    depth: &mut [f32],
    fill_rule: FillRule,
    depth_test: bool,
    depth_write: bool,
    stats: &mut TileStats,
) {
    for tri in triangles.iter() {
        // Verificar si el triángulo intersecta esta banda verticalmente.
        if tri.max_y < band_y_start as i64 || tri.min_y >= band_y_end as i64 {
            continue;
        }

        // Determinar tiles intersectados por el bounding box del triángulo.
        let tiles = grid.tiles_intersected_by_bbox(
            tri.min_x as f32,
            tri.max_x as f32,
            tri.min_y as f32,
            tri.max_y as f32,
        );

        for tile in tiles {
            // Solo procesar tiles dentro de esta banda.
            if tile.y < band_y_start || tile.y >= band_y_end {
                continue;
            }

            raster_triangle_in_tile(
                &tri.a,
                &tri.b,
                &tri.c,
                &tile,
                fb_w,
                color,
                depth,
                fill_rule,
                depth_test,
                depth_write,
                stats,
            );
        }
    }
}

/// Ejecuta rasterización por tiles sobre una lista de triángulos.
pub fn rasterize_triangles_tiled(
    triangles: &[(ScreenVertex, ScreenVertex, ScreenVertex)],
    fb: &mut Framebuffer,
    fill_rule: FillRule,
    depth_test: bool,
    depth_write: bool,
    config: TileConfig,
) -> TileStats {
    let fb_w = fb.width();
    let fb_h = fb.height();
    let grid = TileGrid::new(fb_w, fb_h, config.tile_size);

    let t_bin = std::time::Instant::now();

    // Preparar triángulos con bounding boxes.
    let binned: Vec<BinnedTriangle> = triangles
        .iter()
        .map(|(a, b, c)| {
            let (min_x, max_x, min_y, max_y) = triangle_bbox(a, b, c);
            BinnedTriangle {
                a: *a,
                b: *b,
                c: *c,
                min_x,
                max_x,
                min_y,
                max_y,
            }
        })
        .collect();

    let bin_ns = t_bin.elapsed().as_nanos() as u64;

    let mut stats = TileStats {
        triangles_binned: binned.len() as u32,
        ..Default::default()
    };

    let t_raster = std::time::Instant::now();

    match config.mode {
        RasterMode::Secuencial => {
            // Secuencial: procesar todo en un solo hilo.
            let (color, depth) = fb.color_depth_slices_mut();
            process_band(
                &binned,
                &grid,
                fb_w,
                0,
                fb_h,
                color,
                depth,
                fill_rule,
                depth_test,
                depth_write,
                &mut stats,
            );
        }
        RasterMode::Paralelo | RasterMode::Auto => {
            // Paralelo: dividir framebuffer en bandas horizontales y procesar con Rayon.
            let num_bands = rayon::current_num_threads().max(1);
            let band_height = fb_h.div_euclid(num_bands as u32).max(1);

            let (color, depth) = fb.color_depth_slices_mut();

            let band_stats: Vec<TileStats> = color
                .par_chunks_mut((band_height as usize) * (fb_w as usize) * 4)
                .zip(depth.par_chunks_mut((band_height as usize) * (fb_w as usize)))
                .enumerate()
                .map(|(i, (color_band, depth_band))| {
                    let band_y_start = (i as u32) * band_height;
                    let band_y_end = ((i as u32) + 1) * band_height;
                    // Última banda cubre hasta el final.
                    let band_y_end = if i + 1 == num_bands {
                        fb_h
                    } else {
                        band_y_end
                    };

                    let mut local_stats = TileStats::default();
                    process_band(
                        &binned,
                        &grid,
                        fb_w,
                        band_y_start,
                        band_y_end,
                        color_band,
                        depth_band,
                        fill_rule,
                        depth_test,
                        depth_write,
                        &mut local_stats,
                    );
                    local_stats
                })
                .collect();

            // Reducir estadísticas.
            for s in band_stats {
                stats.fragments_rasterized += s.fragments_rasterized;
                stats.pixels_written += s.pixels_written;
            }
        }
    }

    stats.raster_ns = t_raster.elapsed().as_nanos() as u64;
    stats.bin_ns = bin_ns;
    stats
}
