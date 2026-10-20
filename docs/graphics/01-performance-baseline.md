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
