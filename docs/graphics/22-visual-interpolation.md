# F9 — Interpolación visual temporal

`render-core::SnapshotBuffer` conserva una cantidad acotada de snapshots ordenados
por tiempo de presentación. Descarta inserciones fuera de orden, reemplaza duplicados
del mismo instante y limita la memoria mediante una capacidad configurable.

La muestra interpolada mantiene IDs, mallas, materiales, visibilidad y cámara del
snapshot más reciente. Solo transforma posición, rotación mediante slerp y escala
para identidades compatibles. Los cambios de mesh/material y los saltos mayores que
`teleport_distance` se aplican de forma inmediata. Apariciones y desapariciones no
se inventan ni se extrapolan.

La capa es independiente de `GameplayWorld`, networking y Bevy. CPU y GPU deben
recibir el mismo `RenderSnapshot` producido por `sample`, incluso después de un
hot-swap. Los tests cubren determinismo, orden temporal, capacidad y teletransporte.
La medición de latencia y frame time requiere ejecutar la demo con display.

## Regresión F9/F6.1

La causa confirmada de figuras ausentes era mantener simultáneamente las superficies
CPU y Vulkan sobre la misma ventana. El host libera la superficie CPU antes de crear
GPU y la recrea al volver a CPU, solicita redraw y conserva la última escena válida.
También registra nodos y tamaño del buffer durante cada transición.

## F9B

`run_hot_swap_window` publica snapshots con un reloj monotónico y consume `sample`
antes de cada frame. La misma `SnapshotScene` interpolada se entrega a CPU y GPU;
`G` solo cambia el presentador. `gameplay_visual --window` conserva el bridge de
solo lectura y no modifica el tick autoritativo. El buffer mantiene una capacidad de
8 snapshots y el retraso por defecto es 16,67 ms.

La validación headless de render-present y la compilación del ejemplo pasan. La
prueba visual de movimiento continuo, resize y recuperación tras minimizar requiere
un display; no se declara validada aquí. Clippy completo de `game` queda bloqueado
por advertencias preexistentes en gameplay/networking ajenas a esta integración.
