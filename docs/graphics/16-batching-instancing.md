# F7B: batching e instancing

## Alcance y contratos

Se inspeccionaron `PreparedScene`, los assets procedurales, el pipeline CPU, el
renderer GPU, shaders, presentación y pruebas de F7A antes de modificar el motor.
Se conservan las decisiones de culling/LOD de
[F7A](15-culling-lod.md), snapshots, IDs, handles, transforms y colores por vértice.
No se modifica gameplay, networking, Bevy ni render-present. F8 queda fuera del alcance.

La ruta optimizada recibe la misma `PreparedScene` que CPU. `from_prepared`,
`sync_prepared`, `render_prepared` y la ventana `present_snapshot` usan instancing
real: un `draw_indexed` por batch, con un rango de instancias y un segundo vertex
buffer con `VertexStepMode::Instance`. No hay un draw oculto por entidad.

Se conservan `GpuRenderer::new`, `from_snapshot`, `sync_snapshot` y la ruta offscreen
de snapshots como referencias. `sync_prepared_reference` conserva el algoritmo F7A
con la **misma selección** de visibilidad/LOD, lo que permite aislar F7B de F7A.
Los llamadores de escenas crudas sin handles siguen usando la referencia.

`sync_prepared` devuelve si cambió la lista de fuentes de instancias, incluyendo sus
transforms; **no** indica una reconstrucción GPU. Consultar los contadores de recursos.
`set_transform` conserva el índice de la instancia en el orden de PreparedScene,
aunque varios objetos compartan un draw. `ResourceStats.draw_count` cuenta los batches
que emiten comandos; `instance_count` cuenta entidades preparadas.

`PreparedScene::from_draws` permite construir fixtures visuales sin gameplay. No
ejecuta culling ni selecciona LOD: consume decisiones explícitas del llamador y calcula
sus contadores. La ruta normal sigue siendo `SnapshotScene::prepare`.

## Caché y buffers persistentes

La caché GPU se indexa por `(MeshHandle, LodLevel)`. Cada entrada resuelve una versión
inmutable de geometría y sus buffers de vértices e índices. Al encontrar una nueva
versión se validan índices, valores finitos y límites del dispositivo, y se sube una
sola vez. Ocultar, retirar, mover, añadir instancias o volver a un LOD ya conocido no
invalida esa geometría. La caché vive hasta destruir el renderer o cambiar
explícitamente a una ruta de referencia.

En este motor el color está almacenado en los vértices, no en un uniform de material.
Dos fuentes distintas con igual handle/LOD pueden tener colores o geometría distintos
(por ejemplo, cambiar entre la fixture HIGH y los placeholders de F6). Se conservan
versiones independientes dentro de esa clave para no reutilizar colores incorrectos.
Fuentes idénticas por Arc o por contenido comparten el buffer físico, incluso entre
LOW/HIGH iguales. No se cambian handles para representar esas versiones.

El buffer de instancias contiene MVPs de 64 bytes, calculados como `VP * model`, igual
que la referencia. Esto conserva el orden de las operaciones de transformación del
shader anterior y permite comparar color/depth exactamente en GPU.

La capacidad crece en potencias de dos y no se reduce por ocultación o retirada.
Los vectores de fuentes, matrices y batches reutilizan su capacidad. Si fuentes y VP
no cambian, no se recalculan matrices ni se escriben buffers. Si cambian, se calcula
el intervalo entre la primera y última matriz distinta y se escribe ese tramo; puede
incluir matrices intermedias sin cambios. Mover una sola instancia escribe 64 bytes;
cambiar la cámara actualiza todas las matrices activas. Geometría y pipeline permanecen.

## Compatibilidad, orden y alpha

Los batches agrupan únicamente instancias **consecutivas** compatibles por
mesh/LOD, versión física de geometría y material. El estado de pipeline está fijado
por renderer: mismo formato de color, shader, ausencia de culling de caras, depth
`Less`, escritura de depth y blending desactivado, como en F7A. Renderers de distinto
formato/estado no comparten batches.

No se ordena globalmente por material. Eso podría cambiar el ganador de superficies
coplanares con `Less`, aun siendo opacas. Una secuencia A,A,B,A se conserva y produce
tres batches si A y B son incompatibles. Dentro de un batch se conserva el orden
original de las instancias. Se mantienen todas las identidades en la lista de fuentes.

Una malla con algún alpha distinto de 1 se emite aisladamente y actúa como frontera:
no se reordena ni se fusiona con otra instancia. Se mantiene la semántica actual de
color/alpha sin blending y con escritura de profundidad. No se introducen materiales
transparentes nuevos, sorting de transparencia ni otra ecuación de mezcla.

## CPU

`CpuPipeline::draw_mesh_instances` toma geometría prestada y un iterador de matrices.
Ensamblar los índices se hace una vez por secuencia, reutilizando el mismo vector de
triángulos; transformación, clipping, raster y depth se ejecutan por instancia.
`draw_prepared_scene` detecta secuencias de la misma Arc sin ordenar ni copiar mallas.
`render_prepared_scene` usa esa ruta. `draw_mesh` sigue siendo la referencia sin caché.

El contador `PipelineStats.assembly_cache_hits` informa cuántos ensamblados se
evitaron. Los contadores de vértices y triángulos siguen representando el trabajo de
todas las instancias. Una nueva secuencia comienza siempre ensamblando su malla;
cambios de geometría, topología o llamadas normales no reutilizan índices obsoletos.
CPU sigue funcionando headless sin inicializar GPU ni introducir dependencias.

## Línea base anterior a los cambios del motor

Se añadió primero el benchmark y se ejecutó sobre F7A, antes de editar las fuentes
Rust existentes. Windows x86_64, NVIDIA RTX 4060 Laptop GPU, Vulkan, driver 616.56,
320x180, release, raster CPU secuencial, 5 frames de calentamiento y 30 muestras.

| Escena | Draws/frame | Reconstrucciones/30 frames | Buffers GPU pico, bytes | Preparación µs | Frame GPU ms | Raster CPU ms |
|---|---:|---:|---:|---:|---:|---:|
| small | 2 | 0 | 2144 | 4.950 | 0.212 | 0.041 |
| visible | 1000 | 0 | 1072000 | 934.273 | 1.919 | 9.015 |
| mixed | 200 | 0 | 214400 | 392.733 | 0.779 | 1.684 |
| churn | 750 | 30 | 2136000 | 5752.333 | 6.783 | 22.993 |

En F7A el booleano de sincronización reflejaba reconstruir el renderer; ese fue el
contador disponible antes de añadir instrumentación. No se inventan contadores de
subidas o escrituras que todavía no estaban instrumentados en esa primera corrida.

## Benchmark reproducible y comparación final

```powershell
cargo bench -p render-gpu --bench instancing_bench
```

El programa compara F7A y F7B en el mismo ejecutable, con la fixture procedural HIGH/LOW.
La cámara mira hacia -Z. `visible` coloca exactamente 1.000 barcos dentro del frustum;
`mixed` coloca 800 de ellos fuera; `small` usa dos barcos; `churn` alterna escala
0,2/2,0 cada frame para cruzar los umbrales LOW/HIGH y cambia qué cuarto de los 1.000
objetos está oculto. Quedan 750 visibles por frame, alternando 12/48 triángulos por
barco. No se cambian IDs, mallas lógicas ni gameplay.

Cada ruta tiene 5 frames de calentamiento y 30 muestras. Preparación incluye
`SnapshotScene::update`, culling/LOD y sincronización GPU. Frame GPU incluye esa
preparación, codificación, submit y espera de finalización; no es timestamp de hardware.
Raster CPU se mide por separado con geometría preparada idéntica: referencia con
`draw_mesh`, optimizado con `draw_prepared_scene`. Incluye clear y scratch del pipeline.
Se excluyen generación del snapshot de prueba, inicialización, readback y verificaciones.
No se mide presentación, vsync, simulación ni tiempo total de una aplicación real.

Resultados finales medidos el 22 de octubre de 2026:

| Escena | Ruta | Draws/frame | Preparación µs | Frame GPU ms | Raster CPU ms |
|---|---|---:|---:|---:|---:|
| small | F7A | 2 | 4.303 | 0.179 | 0.036 |
| small | F7B | 1 | 4.837 | 0.183 | 0.037 |
| visible | F7A | 1000 | 811.697 | 1.689 | 7.349 |
| visible | F7B | 1 | 775.167 | 1.190 | 6.576 |
| mixed | F7A | 200 | 513.290 | 1.150 | 1.943 |
| mixed | F7B | 1 | 439.703 | 0.775 | 1.508 |
| churn | F7A | 750 | 3755.180 | 4.542 | 15.946 |
| churn | F7B | 1 | 737.150 | 1.403 | 17.472 |

Los siguientes contadores son acumulados en los 30 frames **medidos**, después del
calentamiento. Cada batch emite realmente un `draw_indexed`; las subidas cuentan
mallas físicas, no llamadas a `queue.write_buffer`. `buffer_rebuilds` cuenta nuevas
parejas de buffers de geometría y crecimientos del buffer de instancias, además de
la construcción de la referencia. No es un contador de asignaciones del driver.

| Escena | Ruta | Reconstrucciones | Mallas subidas | Bytes dinámicos escritos | Buffers GPU pico, bytes |
|---|---|---:|---:|---:|---:|
| small | F7A | 0 | 0 | 15360 | 2144 |
| small | F7B | 0 | 0 | 0 | 1208 |
| visible | F7A | 0 | 0 | 7680000 | 1072000 |
| visible | F7B | 0 | 0 | 0 | 66616 |
| mixed | F7A | 0 | 0 | 1536000 | 214400 |
| mixed | F7B | 0 | 0 | 0 | 17464 |
| churn | F7A | 30 | 22500 | 5760000 | 2136000 |
| churn | F7B | 0 | 0 | 1440000 | 69208 |

Los buffers GPU suman vértices, índices, uniform e instancias reservados, incluyendo
los buffers mínimos de la referencia conservada. No son VRAM total: excluyen targets,
staging de readback, pipelines, driver y metadatos. Tampoco representan RSS total.
Las primeras subidas de geometría y crecimientos ocurren durante inicialización y
calentamiento: los tests verifican sus contadores acumulados y el crecimiento posterior.

Scratch CPU medido igual en ambas rutas: small 8.384 bytes, visible/mixed 8.640 bytes,
churn 16.320 bytes. Son capacidades de vectores del pipeline según `scratch_bytes`;
no incluyen framebuffer, snapshots ni vectores de staging del renderer GPU.
En F7B se evitan 1, 999, 199 y 749 ensamblados por frame, respectivamente.

Esta corrida demuestra menos draws y memoria de buffers, y una mejora GPU marcada
ante cambios frecuentes: 4,542 → 1,403 ms en la comparación final. No demuestra una
mejora universal. La escena pequeña empeora 4 µs en GPU y 1 µs en CPU. Raster CPU
con churn empeora 15,946 → 17,472 ms pese a evitar ensamblados; no se declara una
mejora CPU general. Son medias de una corrida corta, sin intervalos de confianza;
las diferencias con la línea base inicial también muestran variación de frecuencia,
temperatura y carga.

## Corrección, tests y validación

Las pruebas cubren 1.000 instancias, desaparición total, reaparición parcial, movimiento
individual, cámara móvil, LOW/HIGH alternados y simultáneos, crecimiento a 1.500
instancias, reutilización de capacidad y de buffers de geometría, versiones de malla,
materiales incompatibles, empates coplanares y alpha en orden directo e inverso.
También verifican que LOW/HIGH físicamente idénticos no se suben dos veces.

La comparación GPU de referencia frente a instancing exige igualdad exacta de RGBA y
depth en esos tests y en cada escena final del benchmark. Las rutas CPU normal y
reutilizada producen framebuffers idénticos. Los tests CPU/GPU permiten hasta 29
píxeles con diferencia de canal >1 en 160x90 y exigen depth <1e-4 en cobertura común.
El benchmark permite hasta 288 outliers de color (>2) y profundidad (>1e-5) en 320x180;
en la corrida final hubo **cero** outliers en las ocho combinaciones. Las tolerancias
de los tests existentes de F7A permanecen intactas. Las pruebas GPU informan SKIP si no hay adaptador; aquí se
ejecutaron sobre la RTX 4060.

Validación final, todos los comandos con salida cero y 187 tests aprobados:

```powershell
cargo fmt -p render-core -p render-cpu -p render-gpu
cargo fmt -p render-core -p render-cpu -p render-gpu -- --check
cargo check -p render-core -p render-cpu -p render-gpu -p render-present --all-targets
cargo test -p render-core -p render-cpu -p render-gpu -p render-present --lib --tests
cargo test -p render-core -p render-cpu -p render-gpu -p render-present --doc
cargo clippy -p render-core -p render-cpu -p render-gpu -p render-present --all-targets -- -D warnings
cargo bench -p render-gpu --bench instancing_bench
```

Cargo mantiene el aviso del entorno al canonicalizar `C:\Users\sarah`; no hay warnings
Rust ocultados. Durante implementación se actualizaron las expectativas de F7A que
suponían liberar geometría al cambiar de escena: ahora debe permanecer cacheada.

## Limitaciones pendientes

- Se agrupan secuencias compatibles, no conjuntos globales: alternar materiales o
  LODs puede conservar muchos draws. Se prioriza el orden de renderizado existente.
- La caché retiene las versiones vistas hasta destruir el renderer; no tiene eviction.
  Sustituir continuamente el contenido de un handle incrementará su memoria retenida.
- Cambiar explícitamente a una ruta de referencia puede reconstruir el renderer y
  abandonar esa caché. Los cambios normales de visibilidad/LOD no cambian de ruta.
- Los buffers de instancias conservan su máximo de capacidad. La compactación de
  instancias visibles puede escribir un tramo amplio aunque cambien pocos IDs.
- GPU sigue calculando MVP en CPU cuando cambian fuentes o cámara. CPU sigue
  rasterizando por instancia y reconstruye scratch al crear un nuevo pipeline/frame.
- No se implementan PBR, sombras, nuevas transparencias ni F8.
