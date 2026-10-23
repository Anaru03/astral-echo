# F8C — HDR, tone mapping, bloom y antialiasing

F8C añade una etapa GPU de postprocesado optativa alrededor de `OffscreenTarget`. La ruta por defecto sigue siendo la ruta LDR de F8B: renderiza directamente al `Rgba8Unorm` de salida, no crea targets adicionales y no añade trabajo a CPU ni a Unlit. La CPU no importa este módulo.

## API y presets

```rust
target.set_postprocess(render_gpu::PostProcessConfig::preset(
    render_gpu::PostProcessPreset::High,
))?;
```

`Low` equivale a la ruta existente (`enabled = false`). `Medium` activa exposición/tone mapping ACES equivalente y bloom. `High` añade FXAA y un bloom algo más intenso. La configuración permite cambiar `exposure`, `bloom_threshold`, `bloom_strength`, `bloom` y `fxaa`; valores no finitos o negativos son rechazados. `postprocess_config()` y `postprocess_resources()` sirven para inspección y telemetría.

La composición usa una textura fuente persistente, un sampler no filtrado y un pipeline fullscreen de un triángulo. El target se recrea únicamente al cambiar el tamaño; `resize` conserva la configuración y crea exactamente una nueva asignación para el tamaño nuevo. La fuente y el bind group se reutilizan entre frames. La profundidad de cámara se mantiene en el target de F8B y no participa en la composición.

## Flujo de color y alcance actual

La fórmula ACES equivalente trabaja en valores lineales y devuelve [0,1]. Bloom extrae el máximo canal sobre `bloom_threshold`, suma la contribución y después aplica tone mapping. FXAA usa una muestra horizontal vecina como suavizado ligero. No se hace conversión sRGB manual adicional en esta etapa; el target actual es `Rgba8Unorm` lineal y la presentación existente conserva sus reglas de formato.

El renderer de F8B fija sus pipelines de geometría al attachment LDR al crearse. Por eso esta primera integración mantiene una fuente LDR persistente y aplica la composición fullscreen sin declarar un framebuffer HDR nativo: la exposición puede redistribuir el rango ya representado, pero no recupera energía que el attachment LDR haya saturado. `Rgba16Float` y extracción/downsample/blur multinivel quedan como la siguiente extensión del contrato de renderer; no se oculta esta limitación bajo el nombre HDR. No se implementan TAA, HDR de presentación, bloom temporal, sombras nuevas ni efectos de gameplay.

## Integración

`render_prepared` y `render_snapshot` pasan por el mismo target. Sombras, PBR GGX, instancing, caché, culling y LOD terminan primero en la fuente; el postprocesado solo modifica color. El camino apagado sigue llamando directamente a `GpuRenderer::render_to_view_matrix`. La presentación de ventana conserva el comportamiento LDR porque no cambia configuración automáticamente; los consumidores de `OffscreenTarget` pueden seleccionar el preset explícitamente. CPU y ausencia de GPU siguen funcionando sin esta etapa.

## Fixtures y pruebas

Se reutiliza `shadow_fixture`, que contiene un plano, un cubo y material emisivo disponible desde F8A. Las pruebas nuevas verifican monotonicidad de exposición, límites ACES, presets, validación, diferencia visual al activar composición, igualdad de profundidad, persistencia de asignaciones y resize conservando configuración. La prueba no compara color CPU/GPU del postprocesado: CPU no tiene esta ruta y el objetivo es conservar la ruta de rasterización existente.

## Medición

Se añadió `cargo bench -p render-gpu --bench postprocess_bench`, con 1280×720 y 1920×1080, 4 frames de calentamiento y 20 muestras. Reporta tiempo host (sync, submit y espera), asignaciones persistentes, bytes lógicos de la fuente y draws de geometría. La medición debe ejecutarse en la máquina objetivo; no se inventan números aquí porque el attachment sigue dependiendo de la GPU y del driver. La telemetría de timestamps GPU existente en F8B permanece disponible para separar coste de pass cuando el renderer consumidor la habilita.

Limitaciones confirmadas: la ruta no es aún HDR nativa por el attachment LDR fijo; bloom es extracción local, no cadena de downsample/blur; FXAA es opcional y deliberadamente sencillo; no se han medido Metal, DX12 ni hardware integrado. No se inició F9.
