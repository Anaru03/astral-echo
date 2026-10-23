use std::time::Instant;

use rayon::prelude::*;

use crate::framebuffer::Framebuffer;
use crate::pipeline::raster::{
    edge_function, raster_triangle_visit, shade_fragment, FillRule, ScreenVertex,
};

/// Rectangle que representa un tile en la cuadrícula de tiles.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl TileRect {
    pub fn contains(self, px: f32, py: f32) -> bool {
        px >= self.x as f32
            && px < self.x as f32 + self.w as f32
            && py >= self.y as f32
            && py < self.y as f32 + self.h as f32
    }
}

/// Cuadrícula de tiles para rasterizado paralelizado.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileGrid {
    pub tiles_w: u32,
    pub tiles_h: u32,
    pub tile_size: u32,
    pub tile_count_inner: u32,
    pub framebuffer_w: u32,
    pub framebuffer_h: u32,
}

impl TileGrid {
    pub fn new(width: u32, height: u32, tile_size: u32) -> Self {
        let tile_size = tile_size.max(1);
        let tiles_w = width.div_ceil(tile_size);
        let tiles_h = height.div_ceil(tile_size);
        let tile_count = tiles_w * tiles_h;
        TileGrid {
            tiles_w,
            tiles_h,
            tile_size,
            tile_count_inner: tile_count,
            framebuffer_w: width,
            framebuffer_h: height,
        }
    }

    pub fn tile_rect(&self, tile_col: u32, tile_row: u32) -> TileRect {
        let x = tile_col.saturating_mul(self.tile_size);
        let y = tile_row.saturating_mul(self.tile_size);
        let w = self.tile_size.min(self.framebuffer_w.saturating_sub(x));
        let h = self.tile_size.min(self.framebuffer_h.saturating_sub(y));
        TileRect { x, y, w, h }
    }

    pub fn tile_count(&self) -> u32 {
        self.tile_count_inner
    }

    pub fn tiles_intersected_by_bbox(&self, x0: f32, y0: f32, x1: f32, y1: f32) -> Vec<TileRect> {
        let mut result = Vec::new();
        if let Some((min_col, min_row, max_col, max_row)) = self.bbox_tiles(x0, y0, x1, y1) {
            for col in min_col..=max_col {
                for row in min_row..=max_row {
                    result.push(self.tile_rect(col, row));
                }
            }
        }
        result
    }

    fn bbox_tiles(&self, x0: f32, y0: f32, x1: f32, y1: f32) -> Option<(u32, u32, u32, u32)> {
        if self.tiles_w == 0
            || self.tiles_h == 0
            || ![x0, y0, x1, y1].iter().all(|v| v.is_finite())
            || x1 < x0
            || y1 < y0
            || x1 < 0.0
            || y1 < 0.0
            || x0 >= self.framebuffer_w as f32
            || y0 >= self.framebuffer_h as f32
        {
            return None;
        }
        Some((
            x0.max(0.0) as u32 / self.tile_size,
            y0.max(0.0) as u32 / self.tile_size,
            (x1.max(0.0) as u32 / self.tile_size).min(self.tiles_w - 1),
            (y1.max(0.0) as u32 / self.tile_size).min(self.tiles_h - 1),
        ))
    }
}

/// Modo de rasterización.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum RasterMode {
    #[default]
    Secuencial,
    Paralelo,
    Auto,
}

#[derive(Clone, Copy, Debug)]
pub struct RasterWorkload {
    pub framebuffer_pixels: u64,
    pub estimated_work: u64,
    pub bin_entries: usize,
    pub tile_count: usize,
    pub active_bands: usize,
    pub workers: usize,
}

impl RasterMode {
    pub fn select(self, work: RasterWorkload) -> Self {
        if self != Self::Auto {
            return self;
        }
        let workers = work.workers.min(work.active_bands);
        let minimum_work = 262_144 + 32_768 * workers.clamp(2, 4).saturating_sub(2) as u64;
        let bin_cost = (work.bin_entries as u64)
            .saturating_mul(16)
            .saturating_add((work.tile_count as u64).saturating_mul(8));
        if workers > 1
            && work.framebuffer_pixels >= 320 * 180
            && work.estimated_work >= minimum_work
            && work.estimated_work >= bin_cost.saturating_mul(2)
        {
            Self::Paralelo
        } else {
            Self::Secuencial
        }
    }
}

/// Configuración de tiling.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileConfig {
    pub tile_size: u32,
    pub mode: RasterMode,
}

impl Default for TileConfig {
    fn default() -> Self {
        Self {
            tile_size: 16,
            mode: RasterMode::Secuencial,
        }
    }
}

/// Estadísticas de tiling.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TileStats {
    pub pixels_written: u32,
    pub fragments_rasterized: u32,
    pub raster_ns: u64,
    pub bin_ns: u64,
    pub fragments_depth_rejected: u32,
    pub selected_mode: RasterMode,
    pub workers_used: usize,
    pub scratch_bytes: usize,
    pub estimated_work: u64,
    pub bin_entries: usize,
    pub parallel_draws: u32,
    pub sequential_draws: u32,
}

/// Rasteriza triángulos usando tiling.
/// Para cada tile, solo procesa los fragmentos que caen dentro del rectángulo del tile.
pub fn rasterize_triangles_tiled(
    triangles: &[(ScreenVertex, ScreenVertex, ScreenVertex)],
    framebuffer: &mut Framebuffer,
    fill_rule: FillRule,
    depth_test: bool,
    depth_write: bool,
    config: TileConfig,
) -> TileStats {
    let grid = TileGrid::new(framebuffer.width(), framebuffer.height(), config.tile_size);
    let mut stats = TileStats::default();
    if triangles.is_empty() {
        return stats;
    }
    let start = Instant::now();
    let mut bins = vec![Vec::new(); grid.tile_count() as usize];
    for (index, (a, b, c)) in triangles.iter().enumerate() {
        let area = edge_function(a, b, c);
        if !area.is_finite() || area.abs() < 1e-8 {
            continue;
        }
        if let Some((min_col, min_row, max_col, max_row)) = grid.bbox_tiles(
            a.x.min(b.x).min(c.x),
            a.y.min(b.y).min(c.y),
            a.x.max(b.x).max(c.x),
            a.y.max(b.y).max(c.y),
        ) {
            let x0 = a.x.min(b.x).min(c.x).max(0.0).floor() as u64;
            let y0 = a.y.min(b.y).min(c.y).max(0.0).floor() as u64;
            let x1 =
                (a.x.max(b.x)
                    .max(c.x)
                    .min(grid.framebuffer_w as f32 - 1.0)
                    .ceil() as u64)
                    .min(u64::from(grid.framebuffer_w) - 1);
            let y1 =
                (a.y.max(b.y)
                    .max(c.y)
                    .min(grid.framebuffer_h as f32 - 1.0)
                    .ceil() as u64)
                    .min(u64::from(grid.framebuffer_h) - 1);
            stats.estimated_work = stats
                .estimated_work
                .saturating_add((x1 - x0 + 1).saturating_mul(y1 - y0 + 1));
            for row in min_row..=max_row {
                for col in min_col..=max_col {
                    bins[(row * grid.tiles_w + col) as usize].push(index);
                    stats.bin_entries += 1;
                }
            }
        }
    }
    stats.scratch_bytes = bins.capacity() * std::mem::size_of::<Vec<usize>>()
        + bins
            .iter()
            .map(|bin| bin.capacity() * std::mem::size_of::<usize>())
            .sum::<usize>();
    stats.selected_mode = config.mode.select(RasterWorkload {
        framebuffer_pixels: framebuffer.pixel_count(),
        estimated_work: stats.estimated_work,
        bin_entries: stats.bin_entries,
        tile_count: bins.len(),
        active_bands: bins
            .chunks(grid.tiles_w as usize)
            .filter(|row| row.iter().any(|bin| !bin.is_empty()))
            .count(),
        workers: rayon::current_num_threads(),
    });
    stats.parallel_draws = u32::from(stats.selected_mode == RasterMode::Paralelo);
    stats.sequential_draws = u32::from(stats.selected_mode == RasterMode::Secuencial);
    stats.bin_ns = start.elapsed().as_nanos() as u64;
    let job = RasterJob {
        triangles,
        bins: &bins,
        grid,
        fill_rule,
        depth_test,
        depth_write,
    };
    let start = Instant::now();
    let band_pixels = grid.framebuffer_w as usize * grid.tile_size.min(grid.framebuffer_h) as usize;
    let (color, depth) = framebuffer.color_depth_slices_mut();
    if stats.selected_mode == RasterMode::Paralelo {
        let results: Vec<_> = color
            .par_chunks_mut(band_pixels * 4)
            .zip(depth.par_chunks_mut(band_pixels))
            .enumerate()
            .map(|(row, (color, depth))| job.draw_band(row as u32, color, depth))
            .collect();
        let mut seen = vec![false; rayon::current_num_threads()];
        stats.scratch_bytes +=
            results.capacity() * std::mem::size_of::<BandOutput>() + seen.capacity();
        for result in results {
            stats.accumulate(result);
            if let Some(worker) = result.worker {
                seen[worker] = true;
            }
        }
        stats.workers_used = seen.into_iter().filter(|used| *used).count();
    } else {
        for (row, (color, depth)) in color
            .chunks_mut(band_pixels * 4)
            .zip(depth.chunks_mut(band_pixels))
            .enumerate()
        {
            let result = job.draw_band(row as u32, color, depth);
            if result.active {
                stats.workers_used = 1;
            }
            stats.accumulate(result);
        }
    }
    stats.raster_ns = start.elapsed().as_nanos() as u64;
    stats
}

#[derive(Clone, Copy, Default)]
struct BandOutput {
    fragments: u32,
    written: u32,
    rejected: u32,
    worker: Option<usize>,
    active: bool,
}

impl TileStats {
    fn accumulate(&mut self, band: BandOutput) {
        self.fragments_rasterized += band.fragments;
        self.pixels_written += band.written;
        self.fragments_depth_rejected += band.rejected;
    }

    pub fn merge(&mut self, other: Self) {
        self.fragments_rasterized += other.fragments_rasterized;
        self.pixels_written += other.pixels_written;
        self.fragments_depth_rejected += other.fragments_depth_rejected;
        self.bin_ns += other.bin_ns;
        self.raster_ns += other.raster_ns;
        self.selected_mode = other.selected_mode;
        self.workers_used = self.workers_used.max(other.workers_used);
        self.scratch_bytes = self.scratch_bytes.max(other.scratch_bytes);
        self.estimated_work += other.estimated_work;
        self.bin_entries += other.bin_entries;
        self.parallel_draws += other.parallel_draws;
        self.sequential_draws += other.sequential_draws;
    }
}

struct RasterJob<'a> {
    triangles: &'a [(ScreenVertex, ScreenVertex, ScreenVertex)],
    bins: &'a [Vec<usize>],
    grid: TileGrid,
    fill_rule: FillRule,
    depth_test: bool,
    depth_write: bool,
}

impl RasterJob<'_> {
    fn draw_band(&self, row: u32, color: &mut [u8], depth: &mut [f32]) -> BandOutput {
        let mut output = BandOutput::default();
        let start_y = row * self.grid.tile_size;
        let width = self.grid.framebuffer_w as usize;
        for col in 0..self.grid.tiles_w {
            let rect = self.grid.tile_rect(col, row);
            let indices = &self.bins[(row * self.grid.tiles_w + col) as usize];
            for &index in indices {
                output.active = true;
                let (a, b, c) = &self.triangles[index];
                output.fragments += raster_triangle_visit(
                    a,
                    b,
                    c,
                    (self.grid.framebuffer_w, self.grid.framebuffer_h),
                    self.fill_rule,
                    (rect.x, rect.y, rect.x + rect.w, rect.y + rect.h),
                    |fragment| {
                        let pixel = (fragment.y - start_y) as usize * width + fragment.x as usize;
                        if self.depth_test && fragment.depth >= depth[pixel] {
                            output.rejected += 1;
                            return;
                        }
                        let rgba = shade_fragment(&fragment).to_rgba8();
                        color[pixel * 4..pixel * 4 + 4].copy_from_slice(&rgba);
                        if self.depth_write {
                            depth[pixel] = fragment.depth;
                        }
                        output.written += 1;
                    },
                );
            }
        }
        if output.active {
            output.worker = rayon::current_thread_index();
        }
        output
    }
}
