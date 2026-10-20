use crate::error::RasterError;

/// Etapas del pipeline CPU, en orden de ejecución.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stage {
    Vertex,
    Assembly,
    Clipping,
    Raster,
    Fragment,
    Depth,
}

impl Stage {
    pub const ALL: [Stage; 6] = [
        Stage::Vertex,
        Stage::Assembly,
        Stage::Clipping,
        Stage::Raster,
        Stage::Fragment,
        Stage::Depth,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Stage::Vertex => "vertex",
            Stage::Assembly => "assembly",
            Stage::Clipping => "clipping",
            Stage::Raster => "raster",
            Stage::Fragment => "fragment",
            Stage::Depth => "depth",
        }
    }
}

/// Tiempo acumulado por etapa del pipeline, en nanosegundos.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StageTimings {
    pub vertex_ns: u64,
    pub assembly_ns: u64,
    pub clipping_ns: u64,
    pub raster_ns: u64,
    pub fragment_ns: u64,
    pub depth_ns: u64,
}

impl StageTimings {
    pub fn total_ns(&self) -> u64 {
        self.vertex_ns
            + self.assembly_ns
            + self.clipping_ns
            + self.raster_ns
            + self.fragment_ns
            + self.depth_ns
    }

    /// Devuelve el tiempo de la etapa indicada, o `None` si no existe.
    pub fn get(&self, stage: Stage) -> Option<u64> {
        Some(match stage {
            Stage::Vertex => self.vertex_ns,
            Stage::Assembly => self.assembly_ns,
            Stage::Clipping => self.clipping_ns,
            Stage::Raster => self.raster_ns,
            Stage::Fragment => self.fragment_ns,
            Stage::Depth => self.depth_ns,
        })
    }
}

/// Contadores por etapa del pipeline.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StageMetrics {
    /// Vértices leídos de la malla.
    pub vertices_in: u32,
    /// Vértices transformados a clip space.
    pub vertices_out: u32,
    /// Triángulos producidos por el ensamblado.
    pub triangles_assembled: u32,
    /// Triángulos descartados por el ensamblado.
    pub triangles_rejected: u32,
    /// Triángulos recortados por el frustum.
    pub triangles_clipped: u32,
    /// Triángulos completamente descartados por el frustum.
    pub triangles_culled: u32,
    /// Triángulos degenerados detectados en rasterización.
    pub triangles_degenerate: u32,
    /// Fragmentos generados por la rasterización.
    pub fragments_rasterized: u32,
    /// Fragmentos que pasaron la prueba de cobertura.
    pub fragments_shaded: u32,
    /// Fragmentos descartados por el test de profundidad.
    pub fragments_depth_rejected: u32,
    /// Píxeles escritos en el framebuffer.
    pub pixels_written: u32,
    /// Tiempo acumulado por etapa.
    pub timings: StageTimings,
}

impl StageMetrics {
    pub fn merge(&mut self, other: &StageMetrics) {
        self.vertices_in += other.vertices_in;
        self.vertices_out += other.vertices_out;
        self.triangles_assembled += other.triangles_assembled;
        self.triangles_rejected += other.triangles_rejected;
        self.triangles_clipped += other.triangles_clipped;
        self.triangles_culled += other.triangles_culled;
        self.triangles_degenerate += other.triangles_degenerate;
        self.fragments_rasterized += other.fragments_rasterized;
        self.fragments_shaded += other.fragments_shaded;
        self.fragments_depth_rejected += other.fragments_depth_rejected;
        self.pixels_written += other.pixels_written;
        self.timings.vertex_ns += other.timings.vertex_ns;
        self.timings.assembly_ns += other.timings.assembly_ns;
        self.timings.clipping_ns += other.timings.clipping_ns;
        self.timings.raster_ns += other.timings.raster_ns;
        self.timings.fragment_ns += other.timings.fragment_ns;
        self.timings.depth_ns += other.timings.depth_ns;
    }
}

/// Configuración del pipeline completo.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PipelineConfig {
    pub depth_test: bool,
    pub depth_write: bool,
    /// Descarta triángulos con área de pantalla casi nula.
    pub cull_degenerate: bool,
    /// Cronometra cada etapa del pipeline.
    pub timing: bool,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            depth_test: true,
            depth_write: true,
            cull_degenerate: true,
            timing: false,
        }
    }
}

/// Resultado de ejecutar el pipeline sobre un lote de triángulos.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PipelineStats {
    pub metrics: StageMetrics,
    pub draws: u32,
    pub errors: u32,
}

impl PipelineStats {
    pub fn record_error(&mut self) {
        self.errors += 1;
    }
}

/// Alias del error de ejecución del pipeline.
pub type PipelineError = RasterError;
