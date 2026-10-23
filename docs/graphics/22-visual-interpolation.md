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
