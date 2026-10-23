# F6.1 — Hot-swap CPU/GPU

## Estado

Se añadió `run_hot_swap_window`: un host winit único conserva la ventana y el event
loop, presenta CPU mediante softbuffer y crea/libera `SurfaceRenderer` GPU sobre la
misma ventana al pulsar `G`. `S/P/A` conservan los modos CPU; la fuente de snapshot
se actualiza antes de cada frame y los cambios registran latencia. La ruta LOW usa
CPU; presets superiores activan la composición GPU cuando el adaptador está disponible.

La validación automática cubre el host y las rutas headless; la comprobación visual
CPU→GPU→CPU requiere un display compatible y no se ejecutó en este entorno.

`gameplay_visual --window` ya usa este host. El mundo se consulta mediante el
adaptador de snapshot existente y no se avanza el tick. La selección inicial respeta
`--backend cpu|gpu|auto`; `G` permite repetir CPU→GPU→CPU durante la misma sesión.

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
## Corrección de recreación CPU

Al regresar de GPU, la superficie `softbuffer` se crea y se dimensiona inmediatamente
con el tamaño físico no nulo de la ventana antes de llamar a `buffer_mut`. Durante una
minimización se omite la presentación mientras alguna dimensión sea cero; al restaurar
la ventana, el evento de resize vuelve a dimensionar la superficie. Esto evita el panic
`Must set size of surface before calling buffer_mut()` sin ocultarlo mediante
`catch_unwind`.
