# F8A: materiales e iluminación

## Alcance y uso

Se inspeccionaron las APIs reales de geometría, materiales, snapshots, preparación,
pipeline CPU, caché GPU, instancing, shaders y presentación, además de
[F7B](16-batching-instancing.md). No se modificaron gameplay, networking, Bevy ni
render-present. No se implementan sombras, HDR, bloom, postprocesado ni F8B.

La preparación compartida transporta materiales y `LightingEnvironment`. La ruta
existente de snapshots permite activar iluminación sin adaptar datos de gameplay:

```rust
scene.set_lighting_mode(render_core::LightingMode::BlinnPhong);
scene.set_material(handle, render_core::MaterialInfo::fixture_rough());
let prepared = scene.prepare(config);
```

CPU y GPU consumen esa misma escena mediante sus APIs `render_prepared_scene` y
`render_prepared`. La ventana GPU ya usa esa preparación. El modo por defecto sigue
siendo `Unlit`, para conservar el resultado de F7B; Lambert, BlinnPhong y Pbr son
selecciones explícitas. Auto conserva su selección de backend y no decide el modelo
de iluminación. En hardware modesto se puede seleccionar Lambert o BlinnPhong.

Las rutas de referencia de escenas crudas y `sync_prepared_reference` siguen siendo
unlit. No se mezclan los costes de esas rutas con una comparación de iluminación.

## Datos, compatibilidad y normales

`RasterVertex` incorpora `normal`. `RasterVertex::new(position, color)` conserva su
firma, colores y posiciones; inicia una normal cero, que significa normal no provista.
`with_normal` permite autoría explícita. Las mallas existentes que usan constructores
continúan funcionando; un consumidor externo que use literales de la estructura debe
incluir el nuevo campo. El tipo `Vertex` con normal y UV existente no se reemplaza.

`RasterMesh::resolved_normals` respeta normales explícitas válidas y resuelve las
faltantes con normales de triángulos ponderadas por área. Índices inválidos y áreas
degeneradas no aportan una normal; el pipeline sigue validando los índices. Cubos
procedurales tienen normales por cara. La subdivisión HIGH interpola normales de la
malla original, en lugar de perderlas al duplicar vértices. No se cambian posiciones,
índices, IDs, handles ni decisiones de culling/LOD.

Las normales pasan a mundo con la inversa transpuesta del modelo y se normalizan.
Esto incluye rotación, escala negativa y escala no uniforme. Una transformación
singular produce normal cero: conserva ambient/emissive y evita NaN. La normalización
de direcciones se reescala antes de calcular la longitud para tolerar magnitudes muy
pequeñas o grandes. No se invierte una matriz por vértice: se calcula por instancia.

`MaterialInfo` ya contenía los parámetros requeridos. Se reutilizan `base_color`
(albedo, accesible también mediante `albedo()`), `metallic`, `roughness` y `emissive`.
El albedo multiplica el vertex color en modos lit; unlit devuelve el vertex color
original e ignora material/emisión. Un material no registrado usa albedo blanco.
`set_material` sanea datos: albedo y metallic en [0,1], roughness en [0,045;1], emisión
no negativa y sustitución de valores no finitos. El suelo de roughness evita una
distribución especular singular. No se añade una segunda copia del albedo.

Los overrides pertenecen a SnapshotScene/PreparedScene y no alteran RenderSnapshot.
Un cambio de material actualiza datos de instancia; no invalida geometría GPU.

## Luces y modelos

`LightingEnvironment::from_snapshot` consume las luces existentes: conserva la
primera direccional válida y hasta cuatro puntuales válidas, en orden del snapshot.
Se descartan luces con datos no finitos, intensidad negativa, dirección nula o rango
puntual no positivo. Direcciones se normalizan y colores negativos se limitan a cero.
Sin luces directas quedan ambient y emissive. `casts_shadow` no produce sombras.

Ambient por defecto: (0,03;0,03;0,03). Cámara y ambient pueden configurarse en el
entorno preparado. La luz direccional usa la dirección opuesta a la dirección de viaje
de la luz. Para puntuales se emplea:

```text
falloff = clamp(1 - distance² / range², 0, 1)
attenuation = falloff² / (1 + distance²)
```

Lambert usa albedo × max(N·L,0). Blinn-Phong añade especular con half-vector,
F0 interpolado entre 0,04 y albedo según metallic, y exponente derivado de roughness:
`clamp(2 / roughness² - 2, 1, 512)`. Ambient y emisión se suman antes de limitar LDR.

En CPU y GPU los modos básicos se evalúan **por vértice (Gouraud)**. El color lit usa
el clipping e interpolación con perspectiva existentes. Esto permite comparar ambos
backends sin introducir atributos nuevos en todos los buffers del raster CPU.

GPU Pbr evalúa Cook-Torrance por fragmento: distribución GGX con alpha = roughness²,
Fresnel Schlick, aproximación geométrica Schlick-GGX y difuso Lambert reducido por
metallic y Fresnel. Es iluminación directa, sin IBL ni texturas. Referencia conceptual
para GGX/Fresnel y parametrización metallic/roughness:
[documentación técnica de Filament](https://google.github.io/filament/main/filament.html#materialsystem).
La aproximación geométrica elegida aquí es sencilla; no replica todo el modelo de Filament.

CPU también ofrece la fórmula GGX mediante `shade_surface` y modo Pbr, evaluada por
vértice. Sirve como referencia numérica y opción headless. No se promete igualdad de
color entre CPU Gouraud Pbr y GPU por fragmento, especialmente en highlights pequeños.
La equivalencia de color compartida corresponde a Lambert/Blinn-Phong; la geométrica
y de profundidad se conserva en todos los modos.

La salida es LDR [0,1] y no usa unidades fotométricas calibradas. Los highlights y la
emisión pueden saturar. Se mantiene el tratamiento de superficies sRGB del renderer
anterior para presentar los mismos valores numéricos; no se incorpora un pipeline HDR
o una gestión completa de color. El metal puede verse oscuro sin una luz que refleje.

## Integración con F7B y memoria

La geometría posicional conserva sus buffers de 28 bytes por vértice GPU. Unlit sigue
usando instancias de 64 bytes. Las normales se suben a un buffer adicional de 12 bytes
por vértice, una vez por versión de geometría, al activar iluminación. Visibilidad,
LOD, cámara, luces y parámetros de material no vuelven a subir esos atributos estáticos.

La ruta lit reserva instancias de 208 bytes: MVP, tres filas de modelo, tres filas de
normal matrix y parámetros de material. El entorno ocupa 288 bytes para cámara,
ambient, modo y cinco luces. Los buffers crecen en potencias de dos y se reutilizan.
Las matrices/materiales solo se escriben si cambia su contenido; cambiar únicamente
luces o ambient escribe 288 bytes. Cambiar un solo material de la fixture escribe
208 bytes. Los buffers unlit permanecen disponibles para alternar sin reconstrucción.
La caché retiene los buffers lit incluso al volver a unlit.

El pipeline usa tres vertex buffers y 16 atributos. Lambert/Blinn-Phong evitan GGX
por fragmento mediante el modo del entorno. La primera activación compila/crea el
pipeline lit; cambios posteriores de modelo usan ese mismo pipeline.

Se conservan batches consecutivos por mesh/LOD, versión y material, con depth Less
y sin blending. Albedo con alpha distinto de 1, igual que vertex alpha, impide fusionar
instancias lit. No se reordena transparencia ni se cambia su semántica de profundidad.

En CPU se reutilizan índices ensamblados y geometría prestada. Las normales resueltas
se calculan por secuencia de una misma Arc y se comparten entre sus instancias durante
el frame. No hay copias de geometría por entidad. En esta compilación RasterVertex
ocupa 48 bytes; los atributos nuevos sí incrementan memoria de las mallas CPU.

## Fixtures y regresiones visuales

`material_fixture()` construye tres cubos blancos con normales por cara, materiales
metálico, rugoso y emisivo, rotaciones y dos luces. No usa assets externos. Los helpers
`fixture_metal`, `fixture_rough` y `fixture_emissive` también sirven para escenas propias.

Los tests gráficos añaden escala negativa, no uniforme y singular; verifican depth
GPU idéntico entre Unlit/Lambert/BlinnPhong/Pbr y retorno unlit con RGBA idéntico.
Prueban persistencia de geometría/buffers, escrituras dinámicas de material/ambient,
modo alpha, cámara y la referencia analítica de GGX. Las muestras centrales GGX fueron
[2,1,1], [6,15,23], [131,55,34] y [0,0,0], iguales a la referencia cuantizada en esta GPU.
Se verifican cambios de roughness y metallic manteniendo albedo constante.

CPU funciona sin GPU en los tres modos lit. Los tests comparan sus modos de raster
secuencial, paralelo y Auto con igualdad de framebuffer. Los tests existentes de F7B,
incluyendo culling, LOD, aparición/desaparición, instancing y orden, se mantienen.

En las comparaciones nuevas CPU/GPU básicas se permiten menos de 30 píxeles con
diferencia de canal >2 en 320x180 y error medio por canal <0,25. La última ejecución
tuvo dos outliers y error medio máximo 0,00632813, incluyendo unlit. El benchmark tuvo
como máximo cuatro outliers en modos básicos. No se exige igualdad Pbr por píxel
entre Gouraud y fragment shading. Las pruebas GPU se omiten explícitamente sin
adaptador; aquí se ejecutaron con la RTX 4060.

Las pruebas guardan imágenes en `target/f8a-validation/{Lambert,BlinnPhong,Pbr}.png`;
se inspeccionó la salida GGX. Son artefactos generados, no assets requeridos en runtime.

## Medición antes de modificar el motor

Se ejecutó `cargo bench -p render-gpu --bench instancing_bench` sobre F7B antes de
editar las fuentes. Windows x86_64, RTX 4060 Laptop GPU, Vulkan, driver 616.56,
320x180, release, CPU secuencial, 5 warmup + 30 muestras. Se repitió el mismo benchmark
después para revisar unlit. Esta tabla muestra su ruta instanciada:

| Escena | GPU ms antes/después | CPU ms antes/después | Buffers GPU bytes antes/después |
|---|---:|---:|---:|
| small | 0,175 / 0,155 | 0,041 / 0,032 | 1208 / 1208 |
| visible | 1,351 / 1,335 | 8,075 / 7,179 | 66616 / 66616 |
| mixed | 0,717 / 0,721 | 1,478 / 1,452 | 17464 / 17464 |
| churn | 1,843 / 2,289 | 16,134 / 16,381 | 69208 / 69208 |

Unlit conserva un draw por frame, cero reconstrucciones/subidas de geometría durante
las muestras y las mismas reservas GPU. `churn` conserva 1.440.000 bytes dinámicos
escritos en 30 muestras; las escenas estáticas, cero. RGBA/depth de sus referencias
pasan las comparaciones existentes, con cero outliers en la última corrida.

La regresión medida de churn unlit, 1,843 → 2,289 ms GPU, queda pendiente de análisis
de rendimiento con más muestras y timestamps GPU. No se atribuye una causa ni se
declara mejora general a partir de estas corridas cortas.

## Coste de iluminación

```powershell
cargo bench -p render-gpu --bench lighting_bench
```

Corrida final del 22 de octubre de 2026: 320x180, release, 4 warmup + 20 muestras por
modo. `fixture`: 3 objetos, 36 triángulos, 2 luces, 3 draws. `visible1000`: 1.000
objetos dentro del frustum, 12.000 triángulos, 5 luces, **un draw**. `mixed`: 1.000
considerados, 800 descartados, 200 visibles, 2.400 triángulos, 5 luces, **un draw**.

Aquí se prepara culling/LOD una vez para aislar el coste de iluminación. Preparación
en la tabla es `sync_prepared`; frame GPU incluye sincronización, codificación, submit
y espera de finalización. CPU mide clear, normales, iluminación y raster por separado.
Se excluyen generación de escena, readback y validación. No son tiempos del juego,
presentación o timestamps de hardware. Tampoco son directamente comparables con el
benchmark anterior, que actualiza snapshot, culling y LOD cada frame.

| Escena | Modo | Preparación µs | GPU ms | CPU ms | Buffers GPU bytes |
|---|---|---:|---:|---:|---:|
| fixture | Unlit | 0,850 | 0,241 | 0,383 | 1336 |
| fixture | Lambert | 1,260 | 0,176 | 0,361 | 2744 |
| fixture | BlinnPhong | 1,290 | 0,185 | 0,360 | 2744 |
| fixture | Pbr | 1,420 | 0,186 | 0,344 | 2744 |
| visible1000 | Unlit | 32,840 | 0,431 | 6,837 | 66616 |
| visible1000 | Lambert | 71,800 | 0,584 | 11,361 | 280184 |
| visible1000 | BlinnPhong | 124,945 | 0,671 | 12,208 | 280184 |
| visible1000 | Pbr | 144,325 | 0,747 | 13,886 | 280184 |
| mixed | Unlit | 7,395 | 0,276 | 1,451 | 17464 |
| mixed | Lambert | 12,985 | 0,301 | 2,150 | 71288 |
| mixed | BlinnPhong | 18,575 | 0,419 | 2,562 | 71288 |
| mixed | Pbr | 24,320 | 0,298 | 2,559 | 71288 |

Para 1.000 visibles, GGX añade 0,316 ms GPU y 7,049 ms CPU frente a unlit en esta
corrida. El coste CPU es de GGX por vértice, no del algoritmo GPU por fragmento.
La memoria extra reservada es 213.568 bytes GPU. Incluye capacidad de instancias lit,
normales y entorno, y retiene el buffer unlit. No es VRAM total ni RSS: excluye targets,
readback, driver, pipelines y metadatos. Los buffers no se liberan al alternar modos.

Primera activación de Lambert por renderer, incluyendo shader/pipeline, sincronización
y espera: 67,516 ms (fixture), 70,327 ms (1.000 visibles), 69,153 ms (mixed). El primer
frame unlit midió 5,442 / 0,554 / 0,499 ms, respectivamente. BlinnPhong/Pbr posteriores
ya usan el pipeline lit creado. Este coste frío debe contemplarse al precalentar un
renderer; no se modifica la carga o gameplay para ocultarlo.

Los tiempos menores de algunas escenas iluminadas pequeñas reflejan variación de
frecuencias/cachés y ruido de una corrida corta; no demuestran que iluminar sea más
barato. No hay intervalos de confianza ni garantía de rendimiento en otro hardware.

## Validación y límites

196 tests aprobados en los crates gráficos, más la ejecución focalizada posterior de
las nuevas comprobaciones de escrituras dinámicas y alpha. Comandos con salida cero:

```powershell
cargo fmt -p render-core -p render-cpu -p render-gpu
cargo fmt -p render-core -p render-cpu -p render-gpu -- --check
cargo check -p render-core -p render-cpu -p render-gpu -p render-present --all-targets
cargo test -p render-core -p render-cpu -p render-gpu -p render-present --lib --tests
cargo test -p render-gpu --test lighting -- --nocapture
cargo test -p render-core -p render-cpu -p render-gpu -p render-present --doc
cargo clippy -p render-core -p render-cpu -p render-gpu -p render-present --all-targets -- -D warnings
cargo bench -p render-gpu --bench lighting_bench
cargo bench -p render-gpu --bench instancing_bench
```

Persiste el aviso del entorno de Cargo al canonicalizar `C:\Users\sarah`; no hay
warnings Rust ocultados. No se añadieron dependencias.

Límites pendientes: luces limitadas a 1+4, Gouraud en CPU y modos GPU básicos,
highlights sensibles a tessellation/LOD en Gouraud, GGX directo sin entorno, saturación
LDR, coste frío del pipeline, retención de caché/capacidad de F7B y regresión unlit
churn observada. No se incorporan otras fases para resolverlos. F8B no iniciada.
