# 05 — Roadmap F1–F10

- **F1 Render Core** — HECHA (cierre en `06-render-core.md`).
- **F2 Raster CPU básico** — HECHA (cierre en `07-cpu-rasterizer-basic.md`):
  `render-cpu/`: framebuffer RGBA8 + depth 0..1, clip del near plane,
  raster barycéntrico con color perspective-correct, winding configurable,
  test de profundidad, salida PNG; goldens `triangle/cube.png`.
- **F3 Pipeline CPU completo** — vertex/assembly/clipping/raster/fragment/z,
  instrumentado por etapa.
- **F4 Optimización CPU** — tiles + `rayon` + `winit/softbuffer`; LOW 30fps.
- **F5 Backend GPU** — `render-gpu` (`wgpu 29`, WGSL PBR, `gltf`); fallback CPU.
- **F6 Selección auto** — `--backend cpu|gpu|auto` en `entry.rs`.
- **F7 Culling/LOD/Batching** — `Frustum` (ya en core) + instancing + LOD.
- **F8 Iluminación/calidad** — PBR, sombras, fog, ACES, FXAA/MSAA solo GPU.
- **F9 Multijugador render** — buffer de snapshots + interpolación.
- **F10 Compat/rendimiento** — matriz Win/Linux × LOW/MED/HIGH; retirar Bevy.
