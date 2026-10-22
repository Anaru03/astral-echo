# F8B — Sombras direccionales y configuración de luces

## Contrato e integración

F8B añade shadow mapping GPU real sobre `PreparedScene`. No cambia gameplay, networking, Bevy ni el estado del snapshot. `casts_shadow` pertenece a `RenderLight`, no a `SnapshotNode`: se respeta la primera luz direccional admitida por F8A cuando su flag está activo. Las luces puntuales conservan su iluminación y no generan mapas. No se inventa un flag por entidad.

`ShadowConfig` vive en `render-core`, dentro de `LightingEnvironment`. Por defecto está desactivado; `ShadowConfig::low()` también lo desactiva y selecciona 512. Los perfiles LOW existentes no habilitan sombras. CPU conserva su rasterizador y omite esta configuración, incluso si recibe una escena con sombras habilitadas. Unlit no crea ni ejecuta recursos de sombra nuevos. Recursos ya creados se retienen al apagar sombras para poder reactivarlas.

Ejemplo con las APIs reales, antes de preparar la escena:

```rust
scene.set_lighting_mode(render_core::LightingMode::Lambert);
scene.set_shadow_config(render_core::ShadowConfig {
    enabled: true,
    resolution: render_core::ShadowResolution::R1024,
    pcf: true,
    center: render_core::Vec3::ZERO,
    half_extent: 10.0,
    depth: 40.0,
    depth_bias: 0.001,
    slope_bias: 0.002,
    normal_bias: 0.0,
});
let prepared = scene.prepare(render_core::CullingConfig::default());
renderer.sync_prepared(&prepared)?;
```

El snapshot debe contener una direccional con `casts_shadow = true`. Para escenas construidas con `PreparedScene::from_draws`, configurar `prepared.lighting.shadows` directamente. La ruta moderna es `sync_prepared`/`render_prepared`; las rutas `sync_snapshot` y `sync_prepared_reference` mantienen su semántica de referencia Unlit y no añaden sombras.

La proyección ortográfica usa profundidad wgpu [0,1], centro explícito, extensión lateral `±half_extent`, profundidad `depth` y near 0,1. La dirección normalizada proviene de las luces saneadas de F8A. Hay una alternativa de eje up para direcciones verticales. No hay ajuste automático a cámara ni cascadas: la aplicación visual debe configurar un volumen que cubra los receptores y emisores relevantes. Fuera del mapa el receptor permanece iluminado. Extensión/profundidad/bias no finitos o inválidos producen `GpuError::InvalidInput` al sincronizar, sin enviar comandos inválidos a wgpu.

## Pasadas, recursos y compatibilidad

1. `SnapshotScene::prepare` conserva el culling/LOD de cámara para color y construye una lista adicional de emisores cuyo AABB transformado interseca el frustum de luz. Excluye entidades ocultas. Incluye emisores fuera de cámara que pueden sombrear receptores visibles, sin modificar IDs, handles, snapshots ni historial LOD de cámara. Para esos emisores reutiliza el último LOD compatible; si no existe, usa HIGH conservadoramente. Las escenas manuales utilizan sus draws como emisores.
2. Una pasada depth-only instanciada escribe un `Depth32Float` mediante light VP × model. Reutiliza los mismos buffers de geometría y caché `(MeshHandle, LOD)` que la pasada de color. Agrupa secuencias compatibles sin reordenarlas. No vuelve a subir mallas por mover luces, cambiar bias, PCF, visibilidad o tamaño de ventana.
3. La pasada de color consulta la textura de profundidad con sampler de comparación. Atenúa exclusivamente el término direccional; ambiente, emissive y luces puntuales conservan sus contribuciones. Mantiene profundidad, materiales, transformaciones y normales por inversa transpuesta de F8A.

La textura, vista, sampler, bind groups, pipelines y buffer de instancias son persistentes. Solo cambiar resolución recrea el mapa y su bind group; el mapa anterior se libera cuando wgpu termina de usarlo. El buffer crece a potencias de dos; no se reduce al ocultar objetos. La sincronización compara emisores y matriz de luz para evitar recalcular matrices estáticas; las escrituras abarcan únicamente el intervalo dinámico modificado. Cambiar luz/bias/PCF actualiza el uniform de 96 bytes; cambiar dirección también actualiza las matrices de emisores. Resize de presentación no recrea el mapa.

La ruta OFF mantiene los shaders y la iluminación por vértice básica de F8A. La ruta ON evalúa Lambert/Blinn-Phong por fragmento para poder consultar sombras; GGX ya era por fragmento. Con luces puntuales o normales interpoladas, ON puede cambiar también la interpolación de la iluminación básica. No se promete igualdad de color OFF/ON; sí se comprueba igualdad exacta de profundidad. Unlit es idéntico con ambos valores de la configuración. CPU/GPU mantienen la equivalencia geométrica y las tolerancias de color anteriores cuando las sombras están desactivadas.

La semántica de alpha sigue siendo la actual: sin blending ni alpha-test, todas las superficies enviadas escriben profundidad y proyectan su silueta completa. No se introducen transparencias físicas, sombras puntuales, HDR ni postprocesado.

## Bias y PCF

`depth_bias` resta una constante a la profundidad normalizada consultada. `slope_bias` añade `bias × (1 - max(N·L,0))`. `normal_bias` desplaza el receptor en unidades de mundo según su normal transformada. No son enteros de rasterización: el shadow pass conserva bias de rasterización cero. Los valores adecuados dependen de la escala y del volumen de luz. Un bias grande evita acne pero puede separar o eliminar sombras de contacto.

Sin PCF se realiza una comparación; PCF activo promedia 3×3 comparaciones con offsets de un texel. El sampler usa filtrado nearest y clamp-to-edge, sin requerir filtrado lineal de profundidad. No hay sombras suaves físicas; PCF reduce dientes en el borde y añade hasta nueve consultas por fragmento.

## Validación

Fixtures procedurales: `render_core::shadow_fixture()` crea un plano y un cubo con normales, luz direccional y cámara; `shadows_bench` añade 1.000 cubos compartidos y una variante con aparición/desaparición y cambios LOW/HIGH. No usa assets externos.

Pruebas nuevas:

- Valores por defecto/LOW, validación de configuración y proyección near/far, luz vertical, flags direccionales y puntuales.
- AABB de emisores fuera de cámara, ocultos y fuera del volumen de luz; decisiones de color/LOD y snapshots estables.
- Oclusión real, PCF, 512/1024/2048, resize, cambio de dirección y de `casts_shadow`, activación de modos y Unlit exacto.
- Aparición/desaparición, LOD, caché, buffers/mapas persistentes, setter directo de transformaciones con escala negativa/no uniforme frente a sincronización de escena.
- Igualdad exacta de profundidad de cámara OFF/ON. CPU sin GPU conserva color y profundidad con ambos valores del flag.
- Fixture de acne: 7.963 píxeles indebidamente oscuros con bias cero se corrigen con 0,001/0,002; el plano resultante coincide con OFF. Fixture de contacto: bias 0,2 elimina 297 píxeles de sombra respecto del bias razonable. La fixture de oclusión detecta 295 píxeles oscurecidos. Estas pruebas acotan regresiones en las fixtures, no garantizan bias universal.

Comandos ejecutados con salida exitosa:

```text
cargo fmt -p render-core -p render-gpu
cargo check -p render-core -p render-cpu -p render-gpu -p render-present --all-targets
cargo test -p render-core -p render-cpu -p render-gpu -p render-present
cargo clippy -p render-core -p render-cpu -p render-gpu -p render-present --all-targets -- -D warnings
cargo bench -p render-gpu --bench lighting_bench
cargo bench -p render-gpu --bench instancing_bench
cargo bench -p render-gpu --bench shadows_bench
cargo bench -p render-gpu --bench shadows_bench -- --cpu-only
```

Suite conjunta: 203 tests aprobados. Después del ajuste de reutilización se repitió la suite completa de render-gpu. Las pruebas GPU omiten únicamente `GpuError::Unavailable` e imprimen el motivo; errores de backend fallan. La simulación sin GPU usa backends vacíos y ejecuta el rasterizador software; no equivale a haber probado todos los adaptadores reales de gama baja. Cargo emite el aviso externo preexistente `could not canonicalize path C:\Users\sarah`; Clippy no tiene warnings de código. Imágenes de inspección local en `target/f8b-validation/`.

## Mediciones de sombras

2026-10-22, Windows x86_64, RTX 4060 Laptop, Vulkan, driver NVIDIA 616.56, release. Target 320×240, 8 frames de calentamiento + 40 muestras por configuración, sin VSync. Tiempo host = preparación/sync + encoding/submission + espera GPU. No incluye readback de imagen/timestamps ni rasterización CPU intercalada. `prepare_us` incluye construir la lista dinámica en `mixed_churn`; en escenas estáticas mide sync. Los timestamps miden las pasadas GPU, no el tiempo de espera del host. Son opcionales: `enable_gpu_timing()` devuelve false si falta `TIMESTAMP_QUERY`; el render sigue funcionando. Sin activar esta API no se crean queries ni staging de métricas.

La escena visible contiene plano + 1.000 cubos (1.001 objetos); la mixta conserva 750 cubos por frame, alterna visibilidad y una variante LOW. Ambas mantienen 2 draws de color y, con sombras, 2 draws adicionales de profundidad.

| Escena | Resolución/PCF | Preparación µs | Frame host ms | Shadow GPU ms | Color GPU ms |
|---|---|---:|---:|---:|---:|
| Oclusión | OFF | 1,510 | 0,328 | 0 | 0,0035 |
| Oclusión | 512 / OFF | 1,950 | 0,225 | 0,0017 | 0,0040 |
| Oclusión | 1024 / OFF | 2,145 | 0,243 | 0,0037 | 0,0045 |
| Oclusión | 2048 / OFF | 1,960 | 0,243 | 0,0147 | 0,0044 |
| Oclusión | 512 / ON | 1,637 | 0,195 | 0,0018 | 0,0051 |
| Oclusión | 1024 / ON | 1,747 | 0,208 | 0,0038 | 0,0064 |
| Oclusión | 2048 / ON | 1,730 | 0,231 | 0,0146 | 0,0081 |
| 1.000 cubos | OFF | 58,135 | 0,290 | 0 | 0,0232 |
| 1.000 cubos | 512 / OFF | 95,905 | 0,454 | 0,0094 | 0,0234 |
| 1.000 cubos | 1024 / OFF | 89,667 | 0,343 | 0,0207 | 0,0237 |
| 1.000 cubos | 2048 / OFF | 94,835 | 0,407 | 0,0638 | 0,0238 |
| 1.000 cubos | 512 / ON | 85,163 | 0,303 | 0,0093 | 0,0238 |
| 1.000 cubos | 1024 / ON | 90,745 | 0,328 | 0,0205 | 0,0241 |
| 1.000 cubos | 2048 / ON | 100,943 | 0,486 | 0,0636 | 0,0253 |
| Mixta/churn | OFF | 82,440 | 0,484 | 0 | 0,0184 |
| Mixta/churn | 512 / OFF | 157,925 | 0,530 | 0,0072 | 0,0187 |
| Mixta/churn | 1024 / OFF | 186,315 | 0,636 | 0,0165 | 0,0190 |
| Mixta/churn | 2048 / OFF | 182,995 | 0,668 | 0,0472 | 0,0189 |
| Mixta/churn | 512 / ON | 178,870 | 0,668 | 0,0071 | 0,0190 |
| Mixta/churn | 1024 / ON | 194,905 | 0,648 | 0,0165 | 0,0193 |
| Mixta/churn | 2048 / ON | 162,663 | 0,629 | 0,0472 | 0,0204 |

El coste GPU de sombras aumenta con resolución. Que alguna muestra host ON sea inferior a OFF no demuestra una mejora: los tiempos host cortos tienen variación considerable. No se extrapolan estos números a 1080p ni a hardware integrado; PCF y shading por fragmento dependerán de cobertura/overdraw.

Memoria lógica del mapa: 512 = 1.048.576 bytes, 1024 = 4.194.304, 2048 = 16.777.216 (1/4/16 MiB). Los buffers de la escena de 1.000 cubos pasan de 280.368 a 346.000 bytes; mixta de 281.472 a 347.104; oclusión de 2.384 a 2.608. No incluyen targets de cámara/staging, pipelines, asignaciones internas del driver ni RAM de estructuras host: no representan VRAM física total. Tras calentamiento hubo **0 reconstrucciones, 0 subidas de geometría y 0 nuevas asignaciones de mapa** en todas las configuraciones. Estáticas: 0 bytes dinámicos. Mixta, 40 frames: 6.227.520 bytes OFF frente a 8.143.680 ON, por matrices adicionales de emisores. Cambiar resolución produjo una nueva asignación por cambio; PCF, luz, bias y resize no la producen.

CPU, mismo target y 8+40 frames, comparación exacta color/depth:

| Escena | Flag OFF ms | Flag ON ms | Framebuffer bytes |
|---|---:|---:|---:|
| Oclusión | 0,796 | 0,768 | 614.400 |
| 1.000 cubos | 12,024 | 12,902 | 614.400 |

El flag ON se ignora en software; estas variaciones no prueban aceleración ni coste de sombras software. No se añadieron mapas ni buffers de sombra CPU.

## Referencias antes/después y regresiones

Se midieron `lighting_bench` e `instancing_bench` **antes de modificar producción**, sobre F8A validada, y después con sombras OFF. Misma máquina, escenas y parámetros originales. Las rutas de referencia siguen disponibles. Logs locales: `target/f8b-before-lighting.log`, `f8b-before-instancing.log`, `f8b-after-lighting.log`, `f8b-after-instancing.log`, `f8b-shadows-final.log` y `f8b-cpu.log`.

`instancing_bench`, preparación completa incluyendo snapshot/culling/LOD; frame host, 5+30 muestras:

| Escena instanciada | Prep antes/después µs | Frame antes/después ms | CPU antes/después ms |
|---|---|---|---|
| Pequeña | 6,480 / 5,843 | 0,225 / 0,212 | 0,045 / 0,050 |
| 1.000 visibles | 831,237 / 898,723 | 1,304 / 1,382 | 8,318 / 8,147 |
| Mixta | 428,713 / 454,473 | 0,746 / 0,750 | 1,498 / 1,667 |
| Unlit churn | 684,453 / 835,020 | 1,182 / 1,502 | 16,452 / 21,417 |

Unlit churn presenta una regresión observada de ~27% en esta pareja de ejecuciones; no está resuelta ni se atribuye automáticamente a las sombras. Se mantienen 1 draw, 69.208 bytes de buffers, 0 reconstrucciones/subidas, 1.440.000 bytes dinámicos por 30 frames y 0 outliers de color/depth. La ruta no instanciada churn pasí de 5,600 a 6,786 ms: también varió, sin probar una causa común. No se declara mejora general.

`lighting_bench`, 1.000 objetos, frame GPU host antes/después: Unlit 0,574/0,472 ms; Lambert 0,569/0,586; Blinn-Phong 0,560/0,662; GGX 0,508/0,558. Preparación: 55,190/45,140; 94,980/83,915; 106,510/117,900; 90,955/101,500 µs respectivamente. Buffers y draws permanecen iguales (66.616 bytes Unlit, 280.184 iluminados, 1 draw). CPU antes/después: 7,995/7,289; 11,264/11,257; 11,668/13,709; 12,574/12,819 ms. Las diferencias de color CPU/GPU conservan 0 outliers Unlit y 4 básicos en esa escena, iguales a la línea base; no se exige equivalencia de color GGX CPU.

### Investigación Unlit churn

El documento F8A registraba 1,843 frente a 2,289 ms entre fases. La nueva línea base de 1,182 ms no reprodujo ese valor absoluto; por eso se conservan ambos antecedentes y se instrumentan etapas, sin afirmar que la regresión haya desaparecido.

El diagnóstico separa update de snapshot, preparación, sync GPU, encode/submit y espera; hace tres repeticiones sin rasterizador CPU intercalado y tres con él. Son mediciones nuevas, no sustituyen la comparación anterior:

| Intercalar CPU | Frames host ms (3 repeticiones) | Color GPU ms |
|---|---|---|
| No | 0,969 / 0,960 / 1,054 | 0,0141–0,0144 |
| Sí | 1,365 / 2,313 / 1,362 | 0,0143–0,0144 |

En la repetición de 2,313 ms: update 371,373 µs, preparación 298,247, sync 28,607, encode/submit 1.333,957 y espera 280,667. Sin CPU intercalada, encode/submit fue 151,603–179,407 µs. Todas mantienen 0 reconstrucciones y 1.440.000 bytes escritos. Otra ejecución diagnóstica previa produjo 2,292 ms y 1.312,450 µs en encode/submit, con GPU igualmente ~0,0143 ms.

**Confirmado:** la variación observada no está en la ejecución de la pasada GPU; en los picos medidos se concentra en encode/submit y, en menor grado, espera. Update/preparación de snapshots constituyen una parte grande del coste host. No hubo reconstrucción ni crecimiento de draws/buffers. **Hipótesis pendientes:** pacing al intercalar trabajo CPU, scheduling del sistema, estados de energía o bloqueos internos del driver/queue. El experimento muestra asociación con la intercalación y variación; no identifica cuál llamada interna del driver causa la demora ni demuestra causalidad. No se introdujo una refactorización de snapshots/colas fuera de alcance.

### Activación fría de iluminación y sombras

Se instrumentó `LightingPass::new` por etapa. En la ejecución final, tres renderers nuevos:

| Escena | Primera iluminación total ms | Shader module ms | Create pipeline ms | Pop validation ms |
|---|---:|---:|---:|---:|
| Oclusión | 77,449 | 1,907 | 69,877 | 0,009 |
| 1.000 cubos | 71,497 | 1,401 | 68,185 | 0,006 |
| Mixta | 70,267 | 1,454 | 66,738 | 0,009 |

**Confirmado:** el camino lazy crea shader/pipeline al primer modo iluminado; `create_render_pipeline` domina el coste medido. También se crean normales/buffers y se envía/espera el primer frame, incluidos en el total. El shader y pipeline se reutilizan entre Lambert/Blinn-Phong/GGX y posteriores activaciones del mismo renderer. Cada renderer nuevo conserva su propia compilación; no se añadió una caché global. La línea base ya mostraba 66,827–81,199 ms al activar Lambert; después `lighting_bench` obtuvo 74,107–79,713 ms. No se promete arreglar el hitch frío con F8B.

Primera activación de sombras: 82,884–87,214 ms en el ensayo final; incluye los dos pipelines nuevos, mapa y primer frame. Cambios de resolución posteriores: 1,116–2,729 ms en esas muestras (incluyen primer uso del mapa). Permanecen como limitaciones; pueden calentarse usando la ruta real de render durante carga. No se implementó calentamiento automático ni compilación asíncrona.

## Límites pendientes

Un mapa direccional, volumen manual y resolución fija; sin cascadas ni ajuste temporal, puede haber aliasing y cambios al mover luz o volumen. El bias debe calibrarse por escena; no existe garantía universal contra acne/peter-panning. Recursos retenidos OFF consumen memoria tras su primer uso. PCF y mayor resolución añaden coste demostrado, y hay regresiones host registradas. Solo se midió Vulkan en esta GPU, además de la ruta CPU y ausencia simulada de GPU; no se validaron Metal/DX12 ni hardware integrado real. No se inició F8C.
