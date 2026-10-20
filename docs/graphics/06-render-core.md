# 06 — Render Core F1 (cierre)

## Resultados reales de validación
- `cargo fmt -p render-core -- --check` → exit 0.
- `cargo fmt --all -- --check` → exit 1 **solo por archivos preexistentes**
  (`gameplay/src/*.rs`, `game/src/main.rs`, `visual.rs`, `runtime_types.rs`).
  Ningún diff en `render-core/*`, `game/src/render_bridge.rs` ni
  `game/src/lib.rs`. No se tocó gameplay (prohibido por alcance).
- `cargo check --workspace` → exit 0 (`Finished dev profile`).
- `cargo test --workspace --no-fail-fast` → exit 0. Todos los suites
  `ok. 0 passed; 0 failed` (engine, game lib/bin, gameplay, render-core,
  doc-tests). Cero tests porque fueron eliminados por pedido; la
  validación matemática la asume F2 con goldens.
- `cargo tree -p render-core --depth 1` → `engine + glam v0.32.1` únicamente.
- `cargo tree -p render-core -i bevy|wgpu|winit` → `did not match any
  packages` en los tres: **cero dependencias gráficas**.

## Revisiones de diseño
- Model: `Transform::matrix() = T·R·S` vía
  `Mat4::from_scale_rotation_translation`. View: `Mat4::look_at_rh`.
  Projection: `perspective_rh_wgpu` manual 0..1 (near→0, far→1).
- Profundidad 0..1 compatible wgpu/WGSL.
- Normales: `transpose(inverse(M))` + renormalizar (`transform_normal` y
  `Transform::normal_matrix`).
- Frustum: extracción por filas de `P·V`, 6 planos normalizados, test
  p-vertex en `intersects_aabb`.
- `EntityId` estable (`Copy`, `Ord`); `from_engine/to_engine` roundtrip.
- `RenderSnapshot` solo lectura: `snapshot_from_world(&GameplayWorld)`
  nunca muta la sim; `push_node` reemplaza por entidad (sin duplicar draws).
- `EntityId(0)` nave: `EntityRegistry::new()` arranca `next_id = 1`, el
  valor 0 es inalcanzable para entidades reales → sin colisión. Destinos
  en `9000+id` fuera del rango de entidades de gameplay.
- `render-core` usable sin GPU ni ventana: puro `engine + glam`, sin
  `std` gráfico, sin hilos, sin assets.

## Errores encontrados y corregidos
1. `from_engine` unresolved en `render_bridge.rs`: `lib.rs` no
   re-exportaba las funciones libres de `math`. Fix: re-export
   `from_engine/to_engine/transform_normal/perspective_rh_wgpu`
   (commit `08b0cf4` ya lo incluye; verificado con `cargo check -p game`).
2. Procesos `cargo` colgados bloqueando el file lock del target durante la
   validación: se terminaron los PIDs viejos y se reejecutó limpio.
3. `cargo fmt --all` contamina `gameplay/game` preexistentes: revertido con
   `git checkout`; F1 no incluye esos archivos.

## Dependencias verificadas
`glam 0.32.1` (ya transitiva de Bevy 0.19.1, sin conflicto). Sin
`bytemuck` (sin necesidad concreta en F1). Bevy 0.19.1 intacto como
referencia; gameplay sin cambios.

## Riesgos pendientes
- Sin tests unitarios (eliminados por pedido): F2 debe traer goldens
  `triangle/cube.png` + asserts de VP antes de optimizar.
- `perspective_rh_wgpu` manual: contrastar contra `Mat4::perspective_rh`
  de glam en F2 (golden lo detectaría).
- Bridge sin orientación/escala (gameplay no la expone): traslación + cubos.
- `fmt --all` seguirá rojo hasta formatear `gameplay/game` (fuera de F1).

## Estado definitivo
F1 lista: compila, tests (vacíos) pasan, cero deps gráficas, bridge solo
lectura, docs en `docs/graphics/`. **No se inició F2.**
