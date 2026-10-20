# 00 — Estado actual (F0 auditoría)

Workspace: `astral-echo`, `resolver = "2"`, `edition 2021`.
Miembros: `crates/engine`, `crates/gameplay`, `crates/game`, `crates/render-core`.

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
Deps: `engine + glam 0.32`. Sin tests (eliminados por pedido) ni comentarios
en `.rs`; la documentación vive en `docs/graphics/`.
