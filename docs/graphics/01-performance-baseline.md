# 01 — Línea base de rendimiento (F0)

## Ejecución
- `cargo run -p game -- --visual` → Bevy (`visual.rs` + `entry.rs`).
- `cargo run -p game -- --runtime` → consola sin GPU.
- `cargo run -p game` → escenario batch.

## Métricas — `NO MEDIDO`
Sin instrumentación (`FrameTimeDiagnostics`, contadores de draws/tris,
`AdapterInfo`) no hay FPS/P95/CPU/GPU/RAM/VRAM. No se inventan cifras.
Entorno headless: no se ejecutó `--visual` con ventana.

## Problemas confirmados en `visual.rs`
1. `update_hud_system`: `Text::new(hud_text())` cada frame (alloc + relayout).
2. `sync_world_system`: 5 `single_mut()` + `enemies().find()` +
   `collectibles().find()` por frame, sin dirty flag.
3. `hud_text`: recalcula salud/casco/fuel/conteos aunque nada cambie.
4. 5 `.glb` cargados que no existen en el repo → fallback a primitivas.
5. Sin frustum culling, instancing, LOD, MSAA, bloom ni tone mapping.

## Benchmark propuesto (F2+)
Escena fija, cámara `(0,38,62)→(18,0,-18)`, 600 frames, `640×360` CPU /
`1280×720` GPU, `VSync off`, reporte avg/P95/draws/tris + `bench.png`.

## Medición del pipeline CPU (F3, completada)

La propuesta anterior se ejecutó en CPU con el benchmark propio de
`crates/render-cpu/benches/pipeline_bench.rs` (`harness = false`, reloj
`std::time::Instant`, sin dependencias externas ni red).

- Escena: terreno en domo + 12 cubos + 6 triángulos, 19 draws, 278 triángulos.
- Cámara: `(0,26,46)→(0,0,0)`, FOV 60°, near 0.1, far 500.
- `640×360`: **1.14 ms/frame** avg (p95 1.19, min 1.10, max 1.60), 49 311 px y
  50 272 fragmentos por frame, ~876 fps teóricos.
- `1280×720`: **4.23 ms/frame** avg (p95 4.73), 201 038 fragmentos por frame.
- Reparto por etapa a 640×360: raster ~52 %, fragment ~28 %, depth ~17 %,
  clipping ~3 %, vertex + assembly ~0.2 %.

Detalle completo, metodología y validación en
`docs/graphics/08-cpu-pipeline-stages.md`.

**Pendiente para F4/F10**: matriz Win/Linux × LOW/MED/HIGH, comparación contra
GPU (`render-gpu` aún no existe) y `bench.png` como referencia visual de la
escena de benchmark.
