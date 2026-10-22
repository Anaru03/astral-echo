pub mod assembly;
pub mod raster;
pub mod stages;
pub mod tiles;
pub mod vertex;

use glam::Mat4;
use std::sync::Arc;
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
pub use tiles::{
    rasterize_triangles_tiled, RasterMode, RasterWorkload, TileConfig, TileGrid, TileRect,
    TileStats,
};
use vertex::VertexLighting;
pub use vertex::VertexStage;

/// Pipeline CPU por etapas: vertex, assembly, clipping, raster, fragment y depth.
pub struct CpuPipeline<'a> {
    framebuffer: &'a mut Framebuffer,
    viewport: Viewport,
    config: PipelineConfig,
    fill_rule: FillRule,
    tile_config: Option<TileConfig>,
    pub tile_stats: TileStats,
    pub stats: PipelineStats,
    clip_vertices: Vec<ClipVertex>,
    triangles: Vec<Triangle>,
    fragments: Vec<Fragment>,
    shaded: Vec<Color>,
    assembly: AssemblyStats,
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
            tile_config: None,
            tile_stats: TileStats::default(),
            stats: PipelineStats::default(),
            clip_vertices: Vec::new(),
            triangles: Vec::new(),
            fragments: Vec::new(),
            shaded: Vec::new(),
            assembly: AssemblyStats::default(),
        }
    }

    pub fn with_fill_rule(mut self, rule: FillRule) -> Self {
        self.fill_rule = rule;
        self
    }

    pub fn with_tile_config(mut self, config: TileConfig) -> Self {
        self.tile_config = Some(config);
        self
    }

    pub fn tile_config(&self) -> TileConfig {
        self.tile_config.unwrap_or_default()
    }

    pub fn scratch_bytes(&self) -> usize {
        self.clip_vertices.capacity() * std::mem::size_of::<ClipVertex>()
            + self.triangles.capacity() * std::mem::size_of::<Triangle>()
            + self.fragments.capacity() * std::mem::size_of::<Fragment>()
            + self.shaded.capacity() * std::mem::size_of::<Color>()
            + self.tile_stats.scratch_bytes
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
        self.draw_mesh_inner(mesh, mvp, topology, false, None)
    }

    pub fn draw_mesh_instances(
        &mut self,
        mesh: &RasterMesh,
        matrices: impl IntoIterator<Item = Mat4>,
        topology: Topology,
    ) -> Result<PipelineStats, RasterError> {
        let mut reuse = false;
        for mvp in matrices {
            self.draw_mesh_inner(mesh, mvp, topology, reuse, None)?;
            reuse = true;
        }
        Ok(self.stats)
    }

    pub fn draw_prepared_scene(
        &mut self,
        scene: &render_core::PreparedScene,
        vp: Mat4,
    ) -> Result<PipelineStats, RasterError> {
        if scene.lighting.mode != render_core::LightingMode::Unlit {
            return self.draw_lit_scene(scene, vp);
        }
        let mut draws = scene.draws().peekable();
        while let Some(first) = draws.next() {
            let matrices = std::iter::once(vp * first.node.transform.matrix()).chain(
                std::iter::from_fn(|| {
                    let next = draws.peek()?;
                    if !Arc::ptr_eq(&first.mesh, &next.mesh) {
                        return None;
                    }
                    let next = draws.next()?;
                    Some(vp * next.node.transform.matrix())
                }),
            );
            self.draw_mesh_instances(&first.mesh, matrices, Topology::TriangleList)?;
        }
        Ok(self.stats)
    }

    fn draw_lit_scene(
        &mut self,
        scene: &render_core::PreparedScene,
        vp: Mat4,
    ) -> Result<PipelineStats, RasterError> {
        let mut draws = scene.draws().peekable();
        while let Some(first) = draws.next() {
            let normals = first.mesh.resolved_normals();
            let mut draw = first;
            let mut reuse = false;
            loop {
                let model = draw.node.transform.matrix();
                let lighting = VertexLighting {
                    model,
                    normal_matrix: render_core::normal_matrix(model),
                    normals: &normals,
                    material: scene.material(draw.node.material),
                    environment: &scene.lighting,
                };
                self.draw_mesh_inner(
                    &draw.mesh,
                    vp * model,
                    Topology::TriangleList,
                    reuse,
                    Some(&lighting),
                )?;
                reuse = true;
                if !draws
                    .peek()
                    .is_some_and(|next| Arc::ptr_eq(&first.mesh, &next.mesh))
                {
                    break;
                }
                draw = draws.next().expect("peeked draw");
            }
        }
        Ok(self.stats)
    }

    fn draw_mesh_inner(
        &mut self,
        mesh: &RasterMesh,
        mvp: Mat4,
        topology: Topology,
        reuse: bool,
        lighting: Option<&VertexLighting<'_>>,
    ) -> Result<PipelineStats, RasterError> {
        self.stats.draws += 1;
        let stage = VertexStage::new(mvp);
        self.stats.metrics.vertices_in += mesh.vertices.len() as u32;
        let t0 = self.timer();
        if let Some(lighting) = lighting {
            stage.transform_lit_into(&mesh.vertices, lighting, &mut self.clip_vertices)?;
        } else {
            stage.transform_into(&mesh.vertices, &mut self.clip_vertices)?;
        }
        self.stats.metrics.vertices_out += self.clip_vertices.len() as u32;
        let vertex_ns = self.elapsed(t0);

        let t1 = self.timer();
        if !reuse {
            self.assembly = assemble(
                topology,
                &mesh.indices,
                mesh.vertices.len() as u32,
                &mut self.triangles,
            )?;
        }
        self.stats.assembly_cache_hits += u32::from(reuse);
        let assembly = self.assembly;
        let assembly_ns = self.elapsed(t1);
        self.stats.metrics.triangles_assembled += assembly.triangles;
        self.stats.metrics.triangles_rejected +=
            assembly.degenerate + assembly.invalid_indices + assembly.out_of_range;

        if self.tile_config.is_some() {
            self.draw_mesh_tiled();
        } else {
            for i in 0..self.triangles.len() {
                let triangle = self.triangles[i];
                self.draw_assembled_triangle(&triangle);
            }
        }
        self.stats.metrics.timings.vertex_ns += vertex_ns;
        self.stats.metrics.timings.assembly_ns += assembly_ns;
        Ok(self.stats)
    }

    /// Ruta de rasterización por tiles: procesa todos los triángulos del mesh
    /// usando binned rasterization, opcionalmente en paralelo con Rayon.
    fn draw_mesh_tiled(&mut self) {
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
        let mut tile_stats = rasterize_triangles_tiled(
            &screen_triangles,
            self.framebuffer,
            self.fill_rule,
            self.config.depth_test,
            self.config.depth_write,
            self.tile_config.unwrap_or_default(),
        );
        let raster_ns = self.elapsed(t0);

        self.stats.metrics.timings.raster_ns += raster_ns;
        self.stats.metrics.fragments_rasterized += tile_stats.fragments_rasterized;
        self.stats.metrics.pixels_written += tile_stats.pixels_written;
        self.stats.metrics.fragments_shaded += tile_stats.fragments_rasterized;
        self.stats.metrics.fragments_depth_rejected += tile_stats.fragments_depth_rejected;
        tile_stats.scratch_bytes += screen_triangles.capacity()
            * std::mem::size_of::<(ScreenVertex, ScreenVertex, ScreenVertex)>();
        if !self.config.timing {
            tile_stats.bin_ns = 0;
            tile_stats.raster_ns = 0;
        }
        self.tile_stats.merge(tile_stats);
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
