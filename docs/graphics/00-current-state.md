# 00 — Estado actual (F0 auditoría)

Workspace: `astral-echo`, `resolver = "2"`, `edition 2021`.
Miembros: `crates/engine`, `crates/gameplay`, `crates/game`, `crates/render-core`,
`crates/render-cpu`.

## `crates/engine` (`entity.rs`, `math.rs`, `time.rs`)
- `math.rs`: solo `Vec3 {x,y,z:f32}` + `ZERO`. Sin `Mat4/Quat/Transform/AABB`.
  Se mantiene como DTO de simulación; la conversión a render es `render_core::from_engine`.
- `entity.rs`: `EntityId(u64)` + `EntityRegistry {next_id desde 1, known, active}`.
  `next_id` arranca en 1 → `EntityId(0)` nunca lo genera el registro (ver §06).
- `time.rs`: `Tick(u64)`, `Duration(u64 ms)`, `GameTime`. Solo lógica.

## `crates/gameplay`
Simulación determinista por comandos (`GameplayProcessor` en `pipeline.rs`:
pipeline de **lógica**, no gráfico). Sin networking en el árbol actual.

## `crates/game`
`ProjectEchoSession` + `visual.rs` (Bevy 0.19.1, referencia intacta) +
`render_bridge.rs` (F1, solo lectura `GameplayWorld → RenderSnapshot`).

## `crates/render-core` (F1)
`lib, math, transform, camera, bounds, mesh, material, scene, snapshot`.
Deps: `engine + glam 0.32`. La documentación vive en `docs/graphics/`.

## `crates/render-cpu` (F2 + F3)
`clip, color, error, framebuffer, geometry, image, rasterizer, viewport` (F2) +
`pipeline/{vertex,assembly,stages,raster}`, `scene` (F3).
Deps: `render-core + glam 0.32 + png 0.18` (PNG solo para los goldens).
Rasterizador CPU sin ventana, sin GPU y sin hilos: framebuffer RGBA8 con
depth 0..1, recorte del near plane, rasterizado barycéntrico con color
perspective-correct, winding configurable, test de profundidad y salida PNG.
Reutiliza `Camera`/`Transform` del core. Documentación en
`docs/graphics/07-cpu-rasterizer-basic.md` (F2) y
`docs/graphics/08-cpu-pipeline-stages.md` (F3).

### F3 — pipeline por etapas
`CpuPipeline` encadena las seis etapas (vertex, assembly, clipping, raster,
fragment, depth) con contadores por etapa (`StageMetrics`) y cronómetro por
etapa (`StageTimings`, desactivable con `PipelineConfig::timing`). `src/scene.rs`
define la escena de referencia compartida por el demo (`examples/render_demo.rs`)
y el benchmark (`benches/pipeline_bench.rs`, `harness = false`, reloj
`std::time::Instant`, sin dependencias externas). Línea base medida en
`docs/graphics/08-cpu-pipeline-stages.md`: 1.14 ms/frame a 640×360 con 278
triángulos, reparto ~52 % raster / ~28 % fragment / ~17 % depth.
