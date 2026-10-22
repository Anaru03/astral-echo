# 05 — Roadmap F1–F10

- **F1 Render Core** — HECHA (cierre en `06-render-core.md`).
- **F2 Raster CPU básico** — HECHA (cierre en `07-cpu-rasterizer-basic.md`):
  `render-cpu/`: framebuffer RGBA8 + depth 0..1, clip del near plane,
  raster barycéntrico con color perspective-correct, winding configurable,
  test de profundidad, salida PNG; goldens `triangle/cube.png`.
- **F3 Pipeline CPU completo** — HECHA (cierre en `08-cpu-pipeline-stages.md`):
  vertex/assembly/clipping/raster/fragment/z instrumentados por etapa
  (`StageTimings`, desactivable con `PipelineConfig::timing`), escena
  compartida ahora en `render-core/src/demo.rs` (reexportada por CPU), demo ejecutable y benchmark propio offline
  (`benches/pipeline_bench.rs`, sin Criterion).
- **F4 Optimización CPU** — HECHA: tiles + `rayon` + `winit/softbuffer`; LOW 30fps.
- **F4.2 Tiles + Rayon + Auto** — HECHA (cierre en `09-parallel-tiles.md`):
  bandas disjuntas, selección adaptativa, equivalencia exacta de RGBA/depth,
  127 tests CPU y 135 del workspace en ese cierre, matriz de 36 benchmarks.
- **F4.3 Presentación CPU en ventana** — IMPLEMENTADA (detalle y validación en
  `11-cpu-window-presentation.md`): `render-present` con winit + softbuffer,
  presets LOW/MEDIUM/HIGH, resize/DPI, nearest-neighbor, limitador sin busy
  waiting, métricas separadas y tests headless.
- **F5 Backend GPU** — en progreso, sin modificar Bevy ni el backend CPU.
- **F5A Backend GPU propio** — IMPLEMENTADA y validada en Windows/Vulkan
  (detalle en `12-gpu-backend.md`): `render-gpu`, wgpu 29 + WGSL,
  escena F3 compartida, mallas indexadas, vertex colors, MVP, depth,
  PNG offscreen y ventana con recuperación de surface. 166 tests del workspace.
- **F5B** — pendiente; no iniciada. Requiere autorización independiente.
- **F6 Selección auto** — `--backend cpu|gpu|auto` en `entry.rs`.
- **F7 Culling/LOD/Batching** — `Frustum` (ya en core) + instancing + LOD.
- **F8 Iluminación/calidad** — PBR, sombras, fog, ACES, FXAA/MSAA solo GPU.
- **F9 Multijugador render** — buffer de snapshots + interpolación.
- **F10 Compat/rendimiento** — matriz Win/Linux × LOW/MED/HIGH; retirar Bevy.
