# F4.2 — Rasterización paralela por bandas y selección Auto

Fecha de validación: 20 de octubre de 2026. Alcance: F4.2 exclusivamente.

## Ejecución y memoria

`rasterize_triangles_tiled` divide el framebuffer en bandas de `tile_size`
filas y conserva tiles lógicos dentro de cada banda. El binning agrega índices
de triángulos en orden de entrada a una tabla de tiles indexada directamente.
No busca tiles linealmente ni crea un vector temporal de rectángulos por
triángulo.

En modo paralelo, `par_chunks_mut` divide los bytes RGBA y los valores de
profundidad en regiones disjuntas; `zip` asocia ambos slices de cada banda.
Un worker recibe solamente esos préstamos mutables. El origen de la banda
se usa para traducir la dirección de escritura, sin trasladar los vértices.
Los tiles de borde y la última banda pueden ser parciales.

La cobertura, el winding, la regla top-left, el muestreo al centro del píxel,
la profundidad y la interpolación por `inv_w` se calculan en
`raster_triangle_visit`, compartido con el rasterizador original. Una función
receptora resuelve inmediatamente cada fragmento: no hay vector de fragmentos
en el camino tiled, asignaciones por fragmento, `unsafe`, `Mutex` por píxel,
copias completas del framebuffer ni escrituras compartidas entre workers.

Cada tile recorre los índices en orden de entrada. Esto conserva el orden de
escritura de RGBA dentro de cada píxel, incluidos valores alfa no opacos y
empates de profundidad. La operación heredada es reemplazo de RGBA; F4.2 no
agrega composición source-over ni clasificación de transparentes. Las pruebas
verifican también que invertir el orden de entrada cambia el resultado alfa
de la misma manera en secuencial y paralelo.

## Modos e integración

`CpuPipeline::new` conserva la ruta original. Llamar a `with_tile_config`
activa la ruta tiled, incluso para `RasterMode::Secuencial`. Los modos
`Secuencial` y `Paralelo` fuerzan la ejecución solicitada; `Auto` selecciona
una de ellas por draw después de preparar y binnear sus triángulos.

```rust
let pipeline = CpuPipeline::new(&mut framebuffer, viewport, config)
    .with_tile_config(TileConfig {
        tile_size: 16,
        mode: RasterMode::Auto,
    });
```

`TileConfig::default()` usa tiles de 16 y modo secuencial. Para mantener el
contrato auditado de `TileGrid`, un tamaño explícito de cero se normaliza a
uno; no conviene usarlo en resoluciones grandes por el coste de la tabla.

La selección pura `RasterMode::select(RasterWorkload)` utiliza:

- `W`: workers disponibles en el pool actual, limitado a bandas no vacías.
- `P`: cantidad de píxeles del framebuffer.
- `E`: suma de muestras estimadas por los bounding boxes recortados.
- `B`: cantidad de referencias triángulo/tile del binning.
- `T`: cantidad total de tiles.

Auto usa paralelo si se cumplen todas estas condiciones:

```text
W > 1
P >= 320 * 180
E >= 262144 + 32768 * (clamp(W, 2, 4) - 2)
E >= 2 * (16 * B + 8 * T)
```

No mide el hardware ni calibra por frame. La estimación cuesta O(triángulos)
durante el binning y un recorrido de la tabla para contar bandas activas.
El coste del binning ya realizado también forma parte de los tiempos de Auto.
Los coeficientes son unidades de trabajo, no una predicción en nanosegundos.
Se ajustaron una vez tras la primera matriz: el modelo inicial penalizaba
demasiado las referencias de binning y rechazaba cargas que sí se beneficiaban
de Rayon. No se reescribieron los commits anteriores al hacer ese ajuste.

## Instrumentación

`TileStats` informa modo seleccionado, workers realmente observados por draw,
trabajo estimado, referencias de binning, fragmentos, escrituras, rechazos de
profundidad, tiempo de binning y tiempo de ejecución de bandas. Los contadores
se reducen después de unir los workers, sin atomics por fragmento.

`CpuPipeline::tile_stats` acumula los draws; `workers_used` es el máximo de
workers observados en un draw y `scratch_bytes` es el máximo temporal, no la
suma de memoria de draws que ya terminaron. `CpuPipeline::scratch_bytes()`
agrega las capacidades de los vectores persistentes del pipeline.

La memoria reportada incluye payload de vectores de vértices transformados,
triángulos, bins y resultados por banda; para el original también incluye sus
vectores de fragmentos y colores. Excluye framebuffer, mallas de entrada,
metadatos del allocator, pilas y memoria interna de Rayon. Es un inventario de
capacidades del renderer, no una medida de RSS del proceso.

En tiled, rasterización, sombreado y profundidad están fusionados. El tiempo
de bandas incluye esas tres operaciones y el join/reducción; no se divide
artificialmente entre etapas. El benchmark compara ese tiempo con la suma
raster + fragment + depth del original. El tiempo total de frame sigue siendo
la medida principal e incluye vertex, assembly, clipping, preparación y binning.
Con `PipelineConfig::timing = false` los tiempos publicados del pipeline quedan
en cero; los contadores y la decisión Auto siguen disponibles.

## Pruebas automatizadas

Se conservan los 117 tests existentes y se agregan diez en
`crates/render-cpu/tests/parallel_tiles.rs`:

- Trabajo observado en varios workers de un pool controlado de Rayon.
- Igualdad de bytes RGBA y bits del depth buffer, hasta 1280×720.
- Resoluciónes no divisibles y triángulos cruzando múltiples tiles.
- Las cuatro combinaciones de depth test y depth write.
- Orden de escritura alfa, incluyendo orden de entrada invertido.
- Determinismo con pools de uno, dos y cuatro workers y renders repetidos.
- Ausencia de doble cobertura en una arista compartida.
- Auto en escenas pequeñas, grandes y con un único worker disponible.
- Rechazo de paralelismo por binning excesivo, poca resolución o una sola banda.
- Draw vacío y comparación de la escena F3 con el pipeline original.

Total validado: **127 tests de render-cpu y 135 del workspace**, sin fallos.
El segundo total incluye los ocho tests de render-core.

## Benchmark reproducible

```powershell
cargo bench -p render-cpu --offline
```

El benchmark usa 50 muestras tras tres frames de calentamiento por caso y un
pool de `min(workers disponibles, 8)`. Se puede configurar con
`ASTRAL_BENCH_ITERATIONS`, `ASTRAL_BENCH_WORKERS` y `ASTRAL_BENCH_OUTPUT`.
Estas opciones no afectan la heurística ni calibran cada frame.

La matriz tiene 36 casos: original, tiled secuencial, tiled paralelo y Auto;
320×180, 640×360 y 1280×720; y estas escenas:

- F3: `demo_scene()`, 278 triángulos y 19 draws.
- Geometría elevada: terreno 128×128, 32768 triángulos, un draw.
- Overdraw elevado: 32 copias del terreno 8×8 en una sola malla, 4096
  triángulos. Mantener un draw permite medir el coste del raster bajo overdraw.

Se mide el pipeline completo con cronometraje habilitado en todos los modos.
El clear, la creación del pool, la comparación y la exportación PNG quedan
fuera del tiempo medido. Las asignaciones efectuadas durante el draw sí se
miden. Cada modo termina con comparación exacta de RGBA y profundidad contra
el original; un solo píxel distinto hace fallar el benchmark.

Se generan `summary.csv`, `samples.csv` y 36 PNG en `target/f42-bench/`.
El CSV de resumen contiene Avg, P95 por rango más cercano, Min, Max, speedup
contra original y contra tiled secuencial, workers configurados/observados,
memoria temporal y diferencia contra original, tiempos de binning/raster,
selecciones de Auto y hashes de color + profundidad. El resumen curado de esta
ejecución se conserva en [benchmarks/f42-windows-x86_64.csv](benchmarks/f42-windows-x86_64.csv).

Entorno: Windows x86_64, Rust 1.97.0, Rayon 1.12.0, perfil bench optimizado,
16 workers disponibles y ocho configurados. Identificador del procesador:
AMD64 Family 25 Model 117 Stepping 2. La ejecución se hizo por modo en orden
fijo, en una sola máquina; los tiempos incluyen ruido del sistema y no son
una garantía de rendimiento en otro equipo. Un speedup menor que uno indica
regresión; no se descartan muestras lentas.

## Resultados medidos

La tabla y las observaciones siguientes se generan a partir del resumen de la
ejecución final, sin mezclar muestras de la primera matriz.

| Escena | Resolución | Original ms | Tiled seq ms | Paralelo ms | Auto ms | Paralelo/original | Paralelo/tiled seq | Auto: draws par/seq |
|---|---|---:|---:|---:|---:|---:|---:|---|
| f3 | 320x180 | 0.357 | 0.347 | 0.297 | 0.349 | 1.20x | 1.17x | 0.00/19.00 |
| f3 | 640x360 | 1.176 | 1.196 | 0.716 | 1.191 | 1.64x | 1.67x | 0.00/19.00 |
| f3 | 1280x720 | 4.235 | 4.284 | 1.741 | 1.978 | 2.43x | 2.46x | 1.00/18.00 |
| geometry | 320x180 | 8.812 | 8.401 | 7.697 | 8.094 | 1.14x | 1.09x | 0.00/1.00 |
| geometry | 640x360 | 11.091 | 9.528 | 7.691 | 9.415 | 1.44x | 1.24x | 0.00/1.00 |
| geometry | 1280x720 | 15.456 | 14.367 | 9.040 | 13.944 | 1.71x | 1.59x | 0.00/1.00 |
| overdraw | 320x180 | 8.334 | 4.820 | 2.452 | 2.497 | 3.40x | 1.97x | 1.00/0.00 |
| overdraw | 640x360 | 30.767 | 14.750 | 4.436 | 4.523 | 6.94x | 3.33x | 1.00/0.00 |
| overdraw | 1280x720 | 109.422 | 53.829 | 12.571 | 11.722 | 8.70x | 4.28x | 1.00/0.00 |

Memoria y tiempos del paralelo forzado (los cuatro modos completos, con P95,
Min y Max, permanecen en el CSV):

| Escena | Resolución | Workers observados | Scratch KiB | Diferencia vs original KiB | Binning ms | Bandas ms |
|---|---|---:|---:|---:|---:|---:|
| f3 | 320x180 | 6 | 26.9 | +5.1 | 0.029 | 0.194 |
| f3 | 640x360 | 8 | 49.2 | -23.4 | 0.074 | 0.557 |
| f3 | 1280x720 | 8 | 133.2 | -143.7 | 0.251 | 1.357 |
| geometry | 320x180 | 6 | 4431.9 | +3527.7 | 0.527 | 0.875 |
| geometry | 640x360 | 8 | 4454.4 | +3550.1 | 0.592 | 1.023 |
| geometry | 1280x720 | 8 | 4613.4 | +3708.3 | 0.825 | 1.718 |
| overdraw | 320x180 | 6 | 667.8 | +521.0 | 0.131 | 1.453 |
| overdraw | 640x360 | 8 | 842.3 | +644.8 | 0.278 | 3.183 |
| overdraw | 1280x720 | 8 | 1394.0 | +992.2 | 0.943 | 10.408 |

Las 36 comparaciones dieron cero píxeles RGBA distintos y cero valores de
profundidad distintos. Se inspeccionaron además los PNG F3 original y paralelo
a 640x360: mantienen terreno, siluetas, triángulos y oclusión.

El paralelo mejoró el promedio en las nueve combinaciones, pero no todas las
métricas mejoraron. En F3 320x180, P95 paralelo fue 0.4142 ms frente a
0.3638 ms tiled secuencial: mayor variabilidad pese al menor promedio.
Auto en F3 640x360 fue 1.1906 ms frente a 1.1760 ms original (speedup 0.9877x).
Auto sigue siendo conservador en geometría elevada y puede dejar rendimiento
disponible sin aprovechar. Sus umbrales no garantizan elegir el modo más rápido.

La memoria temporal adicional no es cero: en geometría elevada, el paralelo
necesitó hasta unos 3.62 MiB adicionales frente al original por los vertices de
pantalla y los bins. Las cifras de scratch no incluyen el pool ni sus pilas.
Auto y paralelo usan el mismo algoritmo cuando seleccionan paralelo; diferencias
entre sus tiempos de ejecuciónes separadas se interpretan como ruido y efectos
de caché/planificación, no como una ventaja geométrica de Auto.

## Validación y alcance cerrado

| Comando | Código de salida |
|---|---:|
| `cargo fmt -p render-cpu -- --check` | 0 |
| `cargo check -p render-cpu --all-targets --offline` | 0 |
| `cargo test -p render-cpu --offline` | 0 |
| `cargo test --workspace --offline` | 0 |
| `cargo clippy -p render-cpu --all-targets --offline -- -D warnings` | 0 |
| `cargo bench -p render-cpu --offline` | 0 |

Las salidas completas de esta sesión permanecen en
`target/f42-validation/final-*.txt`, fuera de los commits de producción.
El lanzador de Rust emite `could not canonicalize path C:\Users\sarah`;
los comandos anteriores terminaron con los códigos indicados y sin warnings
de rustc/Clippy.

F4.2 queda cerrada con ejecución paralela real, selección Auto, equivalencia
verificada, benchmarks y tests. F4 completo sigue pendiente: **no se inicia
F4.3 ni F5**. Los respaldos previos `tiles_orig.rs` y `tiles_backup.rs` dejaron de estar en
sus rutas iniciales durante la sesión, sin operaciones de borrado del agente.
Por indicación del usuario, se conservan solo sus copias iniciales en
`target/f42-validation/baseline/`, con hashes verificados, y no se incluyen
en commits de producción.
