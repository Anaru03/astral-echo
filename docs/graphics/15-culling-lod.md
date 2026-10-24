# F7A: Frustum Culling + LOD

## Alcance y APIs

Las decisiones pertenecen a `render-core::SnapshotScene::prepare(CullingConfig)`.
`PreparedScene` contiene los mismos `SnapshotNode` y handles del snapshot, la malla
seleccionada, LOW/HIGH, clasificación Outside/Partial/Inside, extensión proyectada
en píxeles y contadores `considered`, `visible`, `discarded`, `triangles`.
`discarded` incluye entidades ocultas y fuera del frustum; los triángulos son los
enviados al pipeline antes de clipping/rasterización, no fragmentos ni draws reales
del driver. `visible + discarded == considered`.

Se inspeccionaron las APIs de render-core, render-cpu, render-gpu, SnapshotScene y
la presentación. No se modificaron gameplay, networking, Bevy ni render-present.
El ejemplo de integración existente sigue llamando a `render_snapshot_scene` y
`present_snapshot`; esas rutas ahora preparan visibilidad con el tamaño del target.
Auto conserva la selección de backend existente.

CPU expone `render_prepared_scene`; GPU expone `GpuRenderer::from_prepared`,
`sync_prepared` y `OffscreenTarget::render_prepared`. Para comparar backends se debe
preparar **una sola vez** y entregar esa misma `PreparedScene` y matriz VP a ambos.
La matriz, viewport y snapshot deben corresponder al mismo frame.

## Algoritmo y estabilidad

Se cachea la AABB local de las mallas. Cada frame transforma sus ocho esquinas para
obtener una AABB mundial conservadora, incluyendo rotación y escalas no uniformes,
negativas o nulas. Se rechaza un objeto solamente si el vértice positivo de su AABB
queda fuera de algún plano. Una tolerancia relativa al error float conserva contactos
de borde; una AABB parcialmente visible se procesa y el pipeline recorta triángulos.
La extracción usa profundidad WGPU `[0,1]`: plano cercano = fila 2, lejano = fila 3 - fila 2.

El tamaño proyectado es la mayor extensión horizontal/vertical de las ocho esquinas
en NDC, convertida a píxeles con width/height. Incluye FOV y aspect de la matriz real.
Si la AABB cruza el plano cercano o tiene w no positivo, se conserva HIGH. No se limita
la extensión a la pantalla, para evitar degradar objetos parcialmente visibles grandes.

Umbrales por defecto: HIGH pasa a LOW por debajo de 12 px; LOW vuelve a HIGH a partir
de 18 px. Dentro del intervalo conserva el estado anterior. Sin historial comienza en
HIGH salvo tamaño inferior al umbral LOW. Usar dimensiones positivas y umbrales
finitos `0 <= low <= high`; ambos en cero fuerzan HIGH y permiten aislar culling.

El historial visual está separado del snapshot, indexado por `SnapshotId`, y comprueba
mesh/material antes de reutilizarlo. Sobrevive a actualizaciones y a ocultación/culling;
se elimina al desaparecer la identidad. No cambia tick, entidades, handles ni transforms.
Cada SnapshotScene mantiene un historial de cámara/viewport: usar escenas separadas
para vistas independientes. La mutabilidad interna con RefCell hace que SnapshotScene
no sea Sync; PreparedScene comparte mallas inmutables mediante Arc.

GPU compara identidad, handles, LOD y referencias de malla; actualiza transforms si
siguen iguales y reconstruye buffers si cambia la lista visible o la geometría, incluso
con igual número de objetos. `render_prepared` sincroniza realmente la geometría;
antes solamente verificaba el número de draws. El coste de reconstrucción por cambios
frecuentes sigue siendo una limitación; no se implementó batching ni instancing.

## Geometría procedural y referencias

`SnapshotScene::default()` conserva exactamente las mallas de F6. Los placeholders
actuales ya son cuboides de 12 triángulos u octaedros de 8: sus variantes LOW/HIGH
comparten la misma malla. Seleccionar LOW no reduce esos mínimos geométricos; la
reducción real de triángulos con las mallas actuales procede del culling.

`SnapshotScene::procedural_lod_fixture()` crea explícitamente HIGH subdividiendo
cada triángulo en cuatro y LOW usando la forma original. Son geometrías procedurales,
sin assets externos, de igual superficie, color y silueta. No se aumenta la geometría
por defecto para aparentar una mejora frente a F6. Las variantes y AABB se generan una
vez; preparar frames clona Arc, no vértices por objeto. La ruta GPU aún clona geometría
en `owned_draws` al reconstruir buffers, como el renderer existente.

Referencias conservadas:

- CPU: `render_snapshot_scene_reference`, visible flag sin culling ni LOD.
- GPU: `from_snapshot`, `sync_snapshot`, `OffscreenTarget::render_snapshot`.
- HIGH con culling: preparar con umbrales cero.
- Ventana GPU: `present_snapshot` usa preparación; la API de presentación por matriz
  permite continuar usando un renderer de referencia preparado por el llamador.

## Reproducción y metodología

Medido el 22 de octubre de 2026, Windows x86_64, release, 320x180, cámara demo.
GPU: NVIDIA GeForce RTX 4060 Laptop GPU, Vulkan, driver 616.56. CPU raster secuencial.
Cada ruta tiene 5 frames de calentamiento y 60 muestras; las tablas son medias de una
corrida, sin intervalos de confianza y sujetas a frecuencia, temperatura y carga.

```powershell
cargo bench -p render-cpu --bench culling_lod_bench
cargo bench -p render-gpu --bench culling_lod_bench
```

Ambos programas crean la misma distribución de objetos. `mixed`: 1.000 barcos, 800
colocados muy fuera de pantalla; `visible`: 1.000 barcos en una cuadrícula mayormente
visible; `small`: dos barcos. La cuadrícula también excede planos lateral y lejano,
por lo que el número final de visibles no es exactamente 200/1.000.

Rutas: `f6` referencia con mallas originales; `f7` preparación con esas mismas mallas;
`reference` fixture HIGH sin culling; `culling` fixture HIGH con culling; `culling_lod`
fixture con culling y selección LOW/HIGH. Las comparaciones `reference`/`culling_lod`
miden la fixture, no la mejora frente a F6.

Frame CPU incluye preparación, clear y raster; la preparación de referencia materializa
una vista de draws y cuenta triángulos. Frame GPU incluye preparación, sincronización,
submit y espera de finalización; no es timestamp de hardware. Preparación GPU incluye
`sync_snapshot`/`sync_prepared`. Se excluyen inicialización, carga inicial de buffers,
readback y exportación. La escena es estática tras calentamiento: no se mide el coste
repetido de reconstruir buffers por cambios de visibilidad/LOD. No se mide presentación,
vsync ni la simulación. Las validaciones de imagen se ejecutan fuera del tiempo medido.

## Resultados CPU

| Escena | Ruta | Visibles/descartados | Triángulos | Preparación µs | Frame ms |
|---|---|---:|---:|---:|---:|
| mixed | f6 | 1000/0 | 12000 | 11.697 | 1.816 |
| mixed | f7 | 149/851 | 1788 | 65.642 | 0.700 |
| mixed | reference | 1000/0 | 48000 | 13.457 | 6.051 |
| mixed | culling | 149/851 | 7152 | 68.798 | 1.959 |
| mixed | culling_lod | 149/851 | 1788 | 68.428 | 0.752 |
| visible | f6 | 1000/0 | 12000 | 10.648 | 4.259 |
| visible | f7 | 742/258 | 8904 | 184.550 | 3.537 |
| visible | reference | 1000/0 | 48000 | 12.193 | 11.469 |
| visible | culling | 742/258 | 35616 | 202.338 | 10.776 |
| visible | culling_lod | 742/258 | 8904 | 217.987 | 4.438 |
| small | f6 | 2/0 | 24 | 0.117 | 0.039 |
| small | f7 | 2/0 | 24 | 0.995 | 0.047 |
| small | reference | 2/0 | 96 | 0.145 | 0.070 |
| small | culling | 2/0 | 96 | 0.900 | 0.066 |
| small | culling_lod | 2/0 | 24 | 1.058 | 0.043 |

## Resultados GPU

| Escena | Ruta | Visibles/descartados | Triángulos | Preparación µs | Frame ms |
|---|---|---:|---:|---:|---:|
| mixed | f6 | 1000/0 | 12000 | 20.375 | 0.518 |
| mixed | f7 | 149/851 | 1788 | 112.678 | 0.375 |
| mixed | reference | 1000/0 | 48000 | 22.780 | 0.588 |
| mixed | culling | 149/851 | 7152 | 116.735 | 0.407 |
| mixed | culling_lod | 149/851 | 1788 | 122.215 | 0.371 |
| visible | f6 | 1000/0 | 12000 | 20.603 | 0.535 |
| visible | f7 | 742/258 | 8904 | 350.233 | 0.918 |
| visible | reference | 1000/0 | 48000 | 26.110 | 0.732 |
| visible | culling | 742/258 | 35616 | 384.162 | 1.017 |
| visible | culling_lod | 742/258 | 8904 | 350.848 | 0.900 |
| small | f6 | 2/0 | 24 | 0.248 | 0.180 |
| small | f7 | 2/0 | 24 | 2.292 | 0.196 |
| small | reference | 2/0 | 96 | 0.240 | 0.177 |
| small | culling | 2/0 | 96 | 2.305 | 0.193 |
| small | culling_lod | 2/0 | 24 | 2.143 | 0.207 |

En esta corrida `mixed` demuestra mejora frente a F6 en ambos backends. `visible`
mejora CPU pero empeora GPU por preparación; `small` empeora en ambos. Reducir
triángulos no garantiza reducir frame time. No se afirma una mejora general, ni una
mejora LOD sobre los placeholders mínimos de F6. No se añade selección adaptativa de
culling o cambios a Auto a partir de estas muestras.

## Corrección y validación

Tests: seis planos y contactos, AABB parcial, entidades ocultas, cámara móvil, escalas
negativas/no uniformes con rotación, FOV/resolución, transiciones en ambos sentidos,
histéresis entre actualizaciones, eliminación/reaparición, mallas cacheadas, estabilidad
de snapshot y actualización GPU con igual cantidad de draws y diferente identidad/LOD.
La prueba CPU y la GPU incluyen un objeto realmente parcial y otro LOW distante.
La ausencia de GPU sigue permitiendo ejecutar CPU; los tests GPU informan SKIP cuando
no hay adaptador. En esta máquina sí se ejecutaron con la RTX 4060.

Los tests focalizados exigen color por canal <= 1, profundidad HIGH/LOW < 1e-5 y
profundidad CPU/GPU < 1e-4 en cobertura común; permiten menos de 60 píxeles distintos
CPU/GPU en 321x181 por reglas de borde. Los benchmarks CPU exigen color <= 1 y
profundidad < 1e-5 en toda la imagen.

El benchmark GPU reutiliza `examples/support::compare`, con los umbrales existentes:
error medio por canal <= 0.25, outliers de color (>2) <= 0.5%, outliers de profundidad
(>1e-5) <= 0.5%. Comparando F6 y F7 con mallas originales, GPU dio cero diferencias de
color y profundidad en las tres escenas. HIGH subdividido frente a LOW produjo un
píxel distinto en las escenas densas (de 57.600), con error máximo de profundidad
0.00016415119. La comparación CPU/GPU tuvo como máximo tres outliers de color y tres
de profundidad, error medio 0.0171832 y profundidad máxima 0.00061041117. Son
comparaciones toleradas, no igualdad bit a bit. La primera comprobación GPU que
exigía igualdad por píxel para la fixture falló; se cuantificó el resultado y se reutilizó
la tolerancia ya establecida, sin cambiar sus límites.

Validación final con salida cero:

```powershell
cargo fmt -p render-core -p render-cpu -p render-gpu
cargo fmt -p render-core -p render-cpu -p render-gpu -- --check
cargo check -p render-core -p render-cpu -p render-gpu -p render-present --all-targets
cargo test -p render-core -p render-cpu -p render-gpu -p render-present --lib --tests
cargo test -p render-core -p render-cpu -p render-gpu -p render-present --doc
cargo clippy -p render-core -p render-cpu -p render-gpu -p render-present --all-targets -- -D warnings
```

La invocación inicial `cargo test --all-targets` también lanzó el benchmark heredado
`pipeline_bench` en debug; se interrumpió ese benchmark y se ejecutaron las suites de
tests explícitamente y los benchmarks F7A en release. No se declara esa invocación
interrumpida como exitosa. Cargo emite un aviso del entorno al canonicalizar
`C:\Users\sarah`; Clippy no reporta warnings Rust.

F7B, batching, instancing, PBR, sombras y mecánicas nuevas quedan fuera del alcance.
