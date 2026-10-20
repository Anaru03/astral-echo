# F3 — Pipeline CPU por etapas

Estado: **completa**. Cierra la fase F3 del roadmap (`05-implementation-roadmap.md`).

## Objetivo

Separar el rasterizador CPU de F2 (`07-cpu-rasterizer-basic.md`) en un pipeline
explícito por etapas, instrumentar cada etapa para poder medir su coste
individual, y establecer una línea base de rendimiento reproducible sobre la que
trabajar en F4.

## Arquitectura

`crates/render-cpu/src/pipeline/` contiene las seis etapas:

| Módulo | Etapa | Responsabilidad |
| --- | --- | --- |
| `vertex.rs` | vertex | transformación de vértices a clip space y descarte de no finitos |
| `assembly.rs` | assembly | ensamblado de primitivas desde índices, detección de degenerados |
| `clip.rs` (F2) | clipping | recorte contra el near plane y reemisión de polígonos |
| `raster.rs` | raster | rasterizado barycéntrico con interpolación perspective-correct |
| `fragment.rs` (F2) | fragment | sombreado por fragmento y conversión a RGBA8 |
| `depth.rs` (F2) | depth | test de profundidad y escritura en el framebuffer |

`pipeline/mod.rs` define `CpuPipeline`, que encadena las etapas y expone:

- `PipelineConfig { width, height, timing }` — `timing` activa o desactiva la
  instrumentación.
- `StageMetrics` — contadores acumulados por etapa (vértices, triángulos,
  fragmentos, píxeles escritos, errores).
- `StageTimings` — nanosegundos acumulados por etapa.
- `clear(color)` — reinicia color, profundidad, contadores y cronómetros.
- `draw_mesh(mesh, mvp, topology)` — ejecuta el pipeline completo para una malla.
- `resolve_fragment(...)` — atajo que encadena fragment + depth (compatibilidad
  con la API de F2).

`stages.rs` define el enum `Stage` (con `Stage::name()`) usado por la
instrumentación y por los reportes.

## Decisiones de diseño

- **Fragment y depth son etapas reales, no una etapa fusionada.**
  `shade_fragment` convierte el color del fragmento a RGBA8 y `depth_resolve`
  hace el test de profundidad y la escritura. `resolve_fragment` se conserva
  como atajo que encadena ambas, para no romper la API de F2.
- **Los buffers intermedios se reutilizan entre draws.** `clip_vertices`,
  `triangles`, `fragments` y `shaded` son vectores miembros del pipeline; no se
  reasignan por triángulo. `shaded` evita recalcular el color en la etapa de
  profundidad.
- **`vertices_out` se acumula** entre draws (`+=`), igual que el resto de
  contadores. Antes se sobrescribía con el valor del último draw, lo que
  falseaba los totales de la escena completa.
- **La instrumentación no cambia el resultado del render.** El timing solo
  envuelve las llamadas existentes; la imagen produced es idéntica con
  `timing: true` y `timing: false`.

## Escena compartida

`crates/render-cpu/src/scene.rs` define la escena de referencia usada por el
demo y por el benchmark, para que ambos midan exactamente lo mismo:

- `grid_mesh(subdivisions, extent, height)`: terreno en cuadrícula con color
  por altura normalizada.
- `demo_scene()`: terreno (domo de radio 22, altura 3) + 12 cubos apoyados
  sobre la superficie del domo + 6 triángulos grandes flotantes. 19 draws,
  278 triángulos.
- `demo_camera(aspect)`: cámara en `(0, 26, 46)` mirando al origen, FOV 60°,
  near 0.1, far 500.

## Demo

`cargo run -p render-cpu --offline --example render_demo` escribe dos PNG en
`target/render-output/`:

- `pipeline_scene.png` — escena de referencia con `timing: true`.
- `pipeline_scene_rotated.png` — la misma escena girada 0.35 rad alrededor de
  Y, para comprobar la estabilidad del pipeline.

El demo falla con error si cualquiera de las dos imágenes sale vacía.

## Benchmark offline

`crates/render-cpu/benches/pipeline_bench.rs` usa `harness = false` (declarado
en `Cargo.toml`) y mide con `std::time::Instant`. **No usa Criterion ni
ninguna dependencia externa**: el workspace no tiene red y `criterion` no está
disponible en la caché local.

Escenarios:

1. Escena de referencia a 640×360.
2. Escalado de resolución: 320×180, 640×360 y 1280×720 con la misma escena.
3. Escena mínima de 1 triángulo: coste puro del pipeline sin geometría.
4. Escalado de carga: terreno de 8×8, 16×16 y 32×32 divisiones a 640×360.

Cada escenario reporta avg/min/max/p95 del tiempo de frame, ms por megapíxel
(calculado desde `width × height`, no hardcodeado), triángulos, píxeles y
fragmentos por frame, y el desglose de nanosegundos y porcentaje por etapa.


## Resultados medidos

Entorno: Windows, perfil `release` (opt-level 3), CPU sin GPU, **200
iteraciones** por escenario. Con menos iteraciones el ruido del sistema
distorsiona las cifras (una ejecución con 60 iteraciones dio avg de
1.15–1.81 ms para la misma escena); con 200 la dispersión entre ejecuciones
baja del ~30 % al ~5 %.

### Escenario de referencia (640×360, 278 triángulos, 19 draws)

| Métrica | Valor |
| --- | --- |
| avg | 1.14 ms/frame |
| p95 | 1.19 ms |
| min | 1.10 ms |
| max | 1.60 ms |
| ms/Mpx | 4.95 |
| px/frame | 49 311 |
| frag/frame | 50 272 |
| fps (avg) | 876 |

### Reparto por etapa (640×360, ns por frame)

| Etapa | ns | % |
| --- | --- | --- |
| vertex | 1 580 | 0.1 |
| assembly | 1 052 | 0.1 |
| clipping | 31 541 | 2.9 |
| raster | 569 432 | 51.6 |
| fragment | 309 462 | 28.1 |
| depth | 190 104 | 17.2 |
| **total** | **1 103 172** | **100** |

### Escalado de resolución (misma escena)

| Resolución | avg (ms) | p95 (ms) | ms/Mpx | fragmentos/frame |
| --- | --- | --- | --- | --- |
| 320×180 | 0.36 | 0.38 | 6.23 | 12 545 |
| 640×360 | 1.13 | 1.21 | 4.92 | 50 272 |
| 1280×720 | 4.23 | 4.73 | 4.59 | 201 038 |

El coste escala de forma aproximadamente lineal con el número de fragmentos
(×4 de píxeles → ×4 de fragmentos → ×3.7 de tiempo). El coste por megapíxel
baja al subir la resolución porque las etapas de geometría (vertex, assembly)
quedan diluidas.

### Escalado de carga (terreno, 640×360)

| Subdivisiones | Triángulos | avg (ms) | p95 (ms) | % clipping |
| --- | --- | --- | --- | --- |
| 8×8 | 128 | 0.95 | 1.00 | 1.6 |
| 16×16 | 512 | 1.15 | 1.22 | 5.0 |
| 32×32 | 2 048 | 1.78 | 2.25 | 14.9 |

Al multiplicar los triángulos por 16 el tiempo solo se multiplica por 1.9: la
escena está limitada por píxeles, no por geometría. El porcentaje de
*clipping* crece con el número de triángulos (el recorte se hace por triángulo,
sin culling por objeto), lo que señala el siguiente cuello de botella.

### Lectura de las cifras

- El **raster domina** con ~52 % del tiempo: es el objetivo natural de F4
  (tiles, recorrido por bands, `rayon`).
- **Fragment + depth son el 45 %** restante. Separar las etapas permite medir
  cuánto cuesta el sombreado frente al test de profundidad.
- **Vertex + assembly son el 0.2 %**: la geometría no es el cuello de botella
  para esta escena; el culling por objeto (F7) no aporta todavía.
- La escena mínima (1 triángulo, 22 px) cuesta 0.001 ms/frame (p95 0.002):
  el coste fijo del pipeline es despreciable y todo el tiempo está en el
  relleno de píxeles.


## Validación real

- `cargo fmt -p render-cpu -- --check` → exit 0.
- `cargo check -p render-cpu --offline --all-targets` → exit 0, sin warnings.
- `cargo test -p render-cpu --offline` → exit 0, **101 tests** en verde:
  - `pipeline_stages` (19): dibuja triángulos, contadores por etapa, culling
    fuera del frustum, recorte, test de profundidad, acumulación de stats entre
    draws, vértices no finitos, valores del buffer de profundidad, color
    escrito, y los 4 tests nuevos de instrumentación (`timing` desactivado
    deja los tiempos a cero, `timing` activado mide las 6 etapas, `clear`
    reinicia color y profundidad, `vertices_out` acumula entre draws).
  - suites heredadas de F2: `clip` (6), `clip_frustum` (18), `depth_order` (5),
    `framebuffer_image` (12), `rasterization` (10), `transform_viewport` (13),
    `vertex_assembly` (18).
- `cargo run -p render-cpu --offline --example render_demo` → exit 0.
  `draws=19 verts_in=387 verts_out=387 tris_assembled=278 tris_clipped=0
  tris_culled=0 tris_rejected=0 tris_degenerate=0`,
  `fragments_rasterized=89390 fragments_shaded=89390 depth_rejected=1675
  pixels_written=87715 errors=0`. Ambas imágenes verificadas a ojo: el domo de
  terreno se ve con el degradado por altura, los cubos apoyan sobre la
  superficie (con oclusión correcta por profundidad) y los triángulos flotantes
  se recortan bien.
- `cargo bench -p render-cpu --offline` → exit 0 con las cifras de arriba.

## Errores encontrados y corregidos

1. `shade_fragment` indexaba el resultado de `vec4_to_rgba8` como si fuera un
   array (`rgba[0]`), pero la función devuelve un `Color`: no compilaba. Se
   corrigió la llamada, no la firma de `vec4_to_rgba8`.
2. `vertices_out` se asignaba (`=`) en vez de acumularse (`+=`), por lo que el
   total de la escena (19 draws) solo reportaba los vértices del último draw.
   El demo ahora informa `verts_in=387 verts_out=387`, que sí cuadra.
3. La escena original enterraba los cubos dentro del domo del terreno (los
   cubos estaban en `y = 0.4` y el domo alcanza `y = 3`): solo se veían
   siluetas parciales. Se añadió `terrain_height(x, z)` y los cubos se apoyan
   ahora sobre la superficie.
4. `Stage::name()` no existía al escribir el reporte del benchmark: se añadió
   al enum en `stages.rs`.
5. La cámara original `(0, 38, 62) → (18, 0, -18)` encuadraba el terreno de
   lado y dejaba la escena descentrada: se cambió a `(0, 26, 46) → (0, 0, 0)`.
6. El benchmark reportaba `min/max` pero no `p95`, que era lo pedido en
   `01-performance-baseline.md`. Se añadió el cálculo del percentil 95 por
   escenario; con 60 iteraciones las cifras eran demasiado ruidosas, por lo que
   el escenario pasó a 200 iteraciones.

## Riesgos pendientes / alcance de F4

- **El recorte sigue siendo solo el near plane.** Falta far plane, planos
  laterales y culling por objeto (`Frustum` ya existe en `render-core`).
- **No hay batching ni instancing**: cada draw recorre la geometría completa.
- **`depth_resolve` es un test de profundidad por fragmento**, sin early-Z: el
  coste de la etapa depth (~17 %) es el de recorrer todos los fragmentos
  rasterizados. Un early-Z por tile (F4) lo reduciría.
- **Sin occlusion culling, sin LOD, sin multithreading**: el raster es
  monohilo.
- **Las cifras son de una sola máquina y de una sola escena.** Sirven como
  línea base relativa (reparto por etapa, escalado), no como objetivo
  absoluto para F4. F10 repetirá la matriz Win/Linux × LOW/MED/HIGH.

## Estado definitivo

F3 lista: pipeline CPU completo por etapas (vertex, assembly, clipping, raster,
fragment, depth) con instrumentación por etapa desactivable, escena compartida
entre demo y benchmark, demo ejecutable que valida sus propias salidas,
benchmark propio sin dependencias externas y línea base medida y documentada.
**No se inició F4.**
