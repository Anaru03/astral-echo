pub mod assembly;
pub mod raster;
pub mod stages;
pub mod tiles;
pub mod vertex;

use glam::Mat4;
use std::time::Instant;

use crate::clip::{clip_triangle, ClipVertex};
use crate::error::RasterError;
use crate::framebuffer::{Color, Framebuffer};
use crate::geometry::RasterMesh;
use crate::viewport::Viewport;

pub use assembly::{assemble, AssemblyStats, Topology, Triangle};
pub use raster::{
    depth_resolve, edge_function, raster_triangle, resolve_fragment, shade_fragment, to_screen,
    FillRule, Fragment, RasterOutput, ScreenVertex,
};
pub use stages::{PipelineConfig, PipelineError, PipelineStats, Stage, StageMetrics};
pub use tiles::{RasterMode, TileConfig, TileGrid, TileRect, TileStats, rasterize_triangles_tiled};
pub use vertex::VertexStage;

/// Pipeline CPU por etapas: vertex, assembly, clipping, raster, fragment y depth.
pub struct CpuPipeline<'a> {
    framebuffer: &'a mut Framebuffer,
    viewport: Viewport,
    config: PipelineConfig,
    fill_rule: FillRule,
    tile_config: TileConfig,
    pub stats: PipelineStats,
    clip_vertices: Vec<ClipVertex>,
    triangles: Vec<Triangle>,
    fragments: Vec<Fragment>,
    shaded: Vec<Color>,
}

impl<'a> CpuPipeline<'a> {
    pub fn new(
        framebuffer: &'a mut Framebuffer,
        viewport: Viewport,
        config: PipelineConfig,
    ) -> Self {
        Self {
            framebuffer,
            viewport,
            config,
            fill_rule: FillRule::TopLeft,
            tile_config: TileConfig::default(),
            stats: PipelineStats::default(),
            clip_vertices: Vec::new(),
            triangles: Vec::new(),
            fragments: Vec::new(),
            shaded: Vec::new(),
        }
    }

    pub fn with_fill_rule(mut self, rule: FillRule) -> Self {
        self.fill_rule = rule;
        self
    }

    pub fn with_tile_config(mut self, config: TileConfig) -> Self {
        self.tile_config = config;
        self
    }

    pub fn tile_config(&self) -> TileConfig {
        self.tile_config
    }

    pub fn framebuffer(&self) -> &Framebuffer {
        self.framebuffer
    }

    pub fn viewport(&self) -> Viewport {
        self.viewport
    }

    /// Limpia el color y la profundidad del framebuffer asociado.
    pub fn clear(&mut self, color: Color) {
        self.framebuffer.clear(color);
    }

    /// Ejecuta el pipeline completo para una malla indexada.
    pub fn draw_mesh(
        &mut self,
        mesh: &RasterMesh,
        mvp: Mat4,
        topology: Topology,
    ) -> Result<PipelineStats, RasterError> {
        self.stats.draws += 1;
        let stage = VertexStage::new(mvp);
        self.stats.metrics.vertices_in += mesh.vertices.len() as u32;
        let t0 = self.timer();
        stage.transform_into(&mesh.vertices, &mut self.clip_vertices)?;
        self.stats.metrics.vertices_out += self.clip_vertices.len() as u32;
        let vertex_ns = self.elapsed(t0);

        let t1 = self.timer();
        let assembly = assemble(
            topology,
            &mesh.indices,
            mesh.vertices.len() as u32,
            &mut self.triangles,
        )?;
        let assembly_ns = self.elapsed(t1);
        self.stats.metrics.triangles_assembled += assembly.triangles;
        self.stats.metrics.triangles_rejected +=
            assembly.degenerate + assembly.invalid_indices + assembly.out_of_range;

        match self.tile_config.mode {
            RasterMode::Secuencial => {
                for i in 0..self.triangles.len() {
                    let triangle = self.triangles[i];
                    self.draw_assembled_triangle(&triangle);
                }
            }
            RasterMode::Paralelo => {
                self.draw_mesh_tiled(
                    &mesh.indices,
                    mesh.vertices.len() as u32,
                    vertex_ns,
                    assembly_ns,
                );
            }
            RasterMode::Auto => {
                // Por defecto, Auto usa el camino secuencial original para
                // mantener compatibilidad. La heurística de selección se
                // activará cuando el modo Parallel se use explícitamente.
                for i in 0..self.triangles.len() {
                    let triangle = self.triangles[i];
                    self.draw_assembled_triangle(&triangle);
                }
            }
        }
        self.stats.metrics.timings.vertex_ns += vertex_ns;
        self.stats.metrics.timings.assembly_ns += assembly_ns;
        Ok(self.stats)
    }

    /// Ruta de rasterización por tiles: procesa todos los triángulos del mesh
    /// usando binned rasterization, opcionalmente en paralelo con Rayon.
    fn draw_mesh_tiled(
        &mut self,
        _indices: &[u32],
        _vertex_count: u32,
        _vertex_ns: u64,
        _assembly_ns: u64,
    ) {
        let mut screen_triangles: Vec<(ScreenVertex, ScreenVertex, ScreenVertex)> = Vec::new();

        for i in 0..self.triangles.len() {
            let triangle = self.triangles[i];
            let indices_tri = triangle.indices();
            let (a, b, c) = match (
                self.clip_vertices.get(indices_tri[0] as usize),
                self.clip_vertices.get(indices_tri[1] as usize),
                self.clip_vertices.get(indices_tri[2] as usize),
            ) {
                (Some(a), Some(b), Some(c)) => (a, b, c),
                _ => {
                    self.stats.metrics.triangles_rejected += 1;
                    continue;
                }
            };

            let t0 = self.timer();
            let clipped = match clip_triangle(*a, *b, *c) {
                Ok(polygon) => polygon,
                Err(_) => {
                    self.stats.record_error();
                    self.stats.metrics.triangles_rejected += 1;
                    continue;
                }
            };
            let clipping_ns = self.elapsed(t0);
            self.stats.metrics.timings.clipping_ns += clipping_ns;

            if clipped.len() < 3 {
                self.stats.metrics.triangles_culled += 1;
                continue;
            }
            if clipped.len() != 3 {
                self.stats.metrics.triangles_clipped += 1;
            }

            for j in 1..clipped.len() - 1 {
                match (
                    to_screen(&clipped[0], &self.viewport),
                    to_screen(&clipped[j], &self.viewport),
                    to_screen(&clipped[j + 1], &self.viewport),
                ) {
                    (Some(va), Some(vb), Some(vc)) => {
                        let area = edge_function(&va, &vb, &vc);
                        if !area.is_finite() || area.abs() < 1e-8 {
                            self.stats.metrics.triangles_degenerate += 1;
                            continue;
                        }
                        screen_triangles.push((va, vb, vc));
                    }
                    _ => {
                        self.stats.metrics.triangles_degenerate += 1;
                    }
                }
            }
        }

        let t0 = self.timer();
        let tile_stats = rasterize_triangles_tiled(
            &screen_triangles,
            self.framebuffer,
            self.fill_rule,
            self.config.depth_test,
            self.config.depth_write,
            self.tile_config,
        );
        let raster_ns = self.elapsed(t0);

        self.stats.metrics.timings.raster_ns += raster_ns;
        self.stats.metrics.fragments_rasterized += tile_stats.fragments_rasterized;
        self.stats.metrics.pixels_written += tile_stats.pixels_written;
        self.stats.metrics.triangles_degenerate += screen_triangles.len() as u32
            - screen_triangles
                .iter()
                .filter(|_| true)
                .count() as u32;
        self.stats.metrics.fragments_shaded += tile_stats.fragments_rasterized;
        self.stats.metrics.timings.fragment_ns += tile_stats.raster_ns / 2;
        self.stats.metrics.timings.depth_ns += tile_stats.raster_ns / 2;
    }

    /// Devuelve el instante actual si el cronometraje está activo.
    fn timer(&self) -> Option<Instant> {
        if self.config.timing {
            Some(Instant::now())
        } else {
            None
        }
    }

    /// Nanosegundos transcurridos desde `start`, o cero si no había cronómetro.
    fn elapsed(&self, start: Option<Instant>) -> u64 {
        match start {
            Some(start) => start.elapsed().as_nanos() as u64,
            None => 0,
        }
    }

    fn draw_assembled_triangle(&mut self, triangle: &Triangle) {
        let indices = triangle.indices();
        let (a, b, c) = match (
            self.clip_vertices.get(indices[0] as usize),
            self.clip_vertices.get(indices[1] as usize),
            self.clip_vertices.get(indices[2] as usize),
        ) {
            (Some(a), Some(b), Some(c)) => (a, b, c),
            _ => {
                self.stats.metrics.triangles_rejected += 1;
                return;
            }
        };
        let t0 = self.timer();
        let clipped = match clip_triangle(*a, *b, *c) {
            Ok(polygon) => polygon,
            Err(_) => {
                self.stats.record_error();
                self.stats.metrics.triangles_rejected += 1;
                return;
            }
        };
        let clipping_ns = self.elapsed(t0);
        self.stats.metrics.timings.clipping_ns += clipping_ns;
        if clipped.len() < 3 {
            self.stats.metrics.triangles_culled += 1;
            return;
        }
        if clipped.len() != 3 {
            self.stats.metrics.triangles_clipped += 1;
        }
        for i in 1..clipped.len() - 1 {
            self.draw_clipped_triangle(&clipped[0], &clipped[i], &clipped[i + 1]);
        }
    }

    fn draw_clipped_triangle(&mut self, a: &ClipVertex, b: &ClipVertex, c: &ClipVertex) {
        let (va, vb, vc) = match (
            to_screen(a, &self.viewport),
            to_screen(b, &self.viewport),
            to_screen(c, &self.viewport),
        ) {
            (Some(va), Some(vb), Some(vc)) => (va, vb, vc),
            _ => {
                self.stats.metrics.triangles_degenerate += 1;
                return;
            }
        };
        let area = edge_function(&va, &vb, &vc);
        if !area.is_finite() || area.abs() < 1e-8 {
            self.stats.metrics.triangles_degenerate += 1;
            return;
        }
        let size = (self.framebuffer.width(), self.framebuffer.height());
        self.fragments.clear();
        let t0 = self.timer();
        let produced = raster_triangle(&va, &vb, &vc, size, self.fill_rule, &mut self.fragments);
        let raster_ns = self.elapsed(t0);
        self.stats.metrics.timings.raster_ns += raster_ns;
        self.stats.metrics.fragments_rasterized += produced;
        if produced == 0 {
            return;
        }
        self.stats.metrics.fragments_shaded += produced;
        // Etapa de fragment: sombreado y conversión de color.
        let t1 = self.timer();
        self.shaded.clear();
        self.shaded
            .extend(self.fragments.iter().map(shade_fragment));
        let fragment_ns = self.elapsed(t1);
        // Etapa de depth: test de profundidad y escritura en el framebuffer.
        let t2 = self.timer();
        for (fragment, color) in self.fragments.iter().zip(self.shaded.iter()) {
            match depth_resolve(
                fragment,
                *color,
                self.framebuffer,
                self.config.depth_test,
                self.config.depth_write,
            ) {
                Ok(true) => {
                    self.stats.metrics.pixels_written += 1;
                }
                Ok(false) => {
                    self.stats.metrics.fragments_depth_rejected += 1;
                }
                Err(_) => {
                    self.stats.record_error();
                }
            }
        }
        let depth_ns = self.elapsed(t2);
        self.stats.metrics.timings.fragment_ns += fragment_ns;
        self.stats.metrics.timings.depth_ns += depth_ns;
    }
}
