# F6.1 — Hot-swap CPU/GPU

## Estado

La transición real con `G` en la misma ventana queda bloqueada por las APIs actuales:
`render-present::run_window` posee el event loop y la superficie `softbuffer`, mientras
`render-gpu::SurfaceRenderer` crea y posee su propia superficie `wgpu` y su ciclo de
presentación. Ninguno expone una superficie/adaptador reutilizable para que un único
event loop conserve la misma ventana al cambiar de backend.

Se añadió `render_present::HotSwapState` como contrato mínimo seguro. Conserva el
snapshot, resolución y estado de backend, registra latencia de transición y mantiene
CPU cuando la creación GPU falla, incluyendo el motivo. Sus pruebas cubren cambios
repetidos, errores de adaptador y resize.

## Cambio mínimo requerido

Exponer en `render-gpu` una construcción de `SurfaceRenderer` sobre una superficie y
ventana proporcionadas por un event loop común, y mover la propiedad del event loop a
una capa de presentación compartida. Después se puede conectar `G` sin duplicar
postprocesado, presets ni snapshots. Hasta entonces no se declara validada la
transición visual real ni se inventan mediciones de frame/memoria.
