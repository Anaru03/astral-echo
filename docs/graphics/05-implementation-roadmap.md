# 05 — Roadmap F1–F10

- **F1 Render Core** — HECHA (cierre en `06-render-core.md`).
- **F2 Raster CPU básico** — HECHA (cierre en `07-cpu-rasterizer-basic.md`):
  `render-cpu/`: framebuffer RGBA8 + depth 0..1, clip del near plane,
  raster barycéntrico con color perspective-correct, winding configurable,
  test de profundidad, salida PNG; goldens `triangle/cube.png`.
- **F3 Pipeline CPU completo** — HECHA (cierre en `08-cpu-pipeline-stages.md`):
  vertex/assembly/clipping/raster/fragment/z instrumentados por etapa
  (`StageTimings`, desactivable con `PipelineConfig::timing`), escena
  compartida en `src/scene.rs`, demo ejecutable y benchmark propio offline
  (`benches/pipeline_bench.rs`, sin Criterion).
- **F4 Optimización CPU** — tiles + `rayon` + `winit/softbuffer`; LOW 30fps.
- **F4.2 Tiles + Rayon + Auto** — HECHA (cierre en `09-parallel-tiles.md`):
  bandas disjuntas, selección adaptativa, equivalencia exacta de RGBA/depth,
  127 tests CPU y 135 del workspace, matriz de 36 benchmarks. F4 completo
  sigue pendiente; no se inicia F4.3 ni F5.
- **F5 Backend GPU** — `render-gpu` (`wgpu 29`, WGSL PBR, `gltf`); fallback CPU.
- **F6 Selección auto** — `--backend cpu|gpu|auto` en `entry.rs`.
- **F7 Culling/LOD/Batching** — `Frustum` (ya en core) + instancing + LOD.
- **F8 Iluminación/calidad** — PBR, sombras, fog, ACES, FXAA/MSAA solo GPU.
- **F9 Multijugador render** — buffer de snapshots + interpolación.
- **F10 Compat/rendimiento** — matriz Win/Linux × LOW/MED/HIGH; retirar Bevy.
