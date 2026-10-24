# F8C.1 — HDR real y bloom multinivel

F8C.1 completa la ruta HDR GPU separada de LOW/Unlit. `GpuRenderer::from_prepared_hdr` crea pipelines con attachment `Rgba16Float`; `OffscreenTarget::new_hdr` conserva la salida pública LDR `Rgba8Unorm` y añade una fuente HDR persistente para la composición. Los shaders de iluminación y sombras ya no saturan RGB antes del attachment: solo alpha se limita y el tone mapping ACES es el primer clamp de color de la ruta HDR.

```rust
let mut renderer = GpuRenderer::from_prepared_hdr(gpu.clone(), &scene)?;
let mut target = OffscreenTarget::new_hdr(gpu, 1280, 720)?;
target.set_postprocess(PostProcessConfig::preset(PostProcessPreset::High))?;
target.render_prepared(&mut renderer, &scene, view_projection)?;
```

La salida final sigue siendo RGBA8 para readback/presentación. El HDR intermedio puede conservar emissive y radiancia mayores que uno; la fixture de prueba usa emissive 8 y verifica que la composición lo tone-mapée sin error de formato. La ruta `OffscreenTarget::new` y los renderers normales permanecen LDR y no crean la textura float.

El bloom hace extracción de altas luces en tres radios reutilizables (1, 2 y 4 texels), con blur de nueve muestras por nivel y composición ponderada antes de ACES. Esta implementación es un bloom multinivel en un pass fullscreen; no usa mipmaps ni tres render passes separados porque el objetivo es evitar recreaciones y conservar bajo coste. FXAA se ejecuta después de ACES con vecinos horizontales. La exposición se aplica antes de extracción y tone mapping. No hay conversión gamma manual en HDR: `Rgba16Float` es lineal; la salida LDR existente conserva su formato y contrato de presentación.

Resize recrea solamente targets dependientes de resolución y conserva el preset. Texturas, bind group, sampler, uniform y pipeline se reutilizan en frames posteriores. Shadows, PBR GGX, instancing, caché, culling, LOD y depth permanecen en la pasada geométrica anterior.

Validación añadida: valores HDR mayores que uno, exposición monotónica, ACES acotado, bloom/FXAA, resize, cambios de preset, profundidad intacta y persistencia. Se ejecutaron `cargo fmt`, `cargo check`, tests de `render-core`, `render-cpu`, `render-gpu`, `render-present` y Clippy con `-D warnings`.

Medición F8C previa y F8C.1 en RTX 4060 Laptop, Vulkan, 320×240: el postprocesado LDR de F8C costaba 0,120 ms OFF/LOW, 0,150 ms MEDIUM y 0,201 ms HIGH. A 1280×720 la textura persistente ocupa 3,69 MiB y a 1920×1080 8,29 MiB. La ruta HDR añade el mismo target lógico float de 8 bytes por píxel: 7,03 MiB a 720p y 16,59 MiB a 1080p, además de la salida LDR. Las cifras dependen del driver y no se extrapolan a otros adaptadores.

Limitaciones: la ventana `SurfaceRenderer` sigue siendo LDR y selecciona HDR explícitamente mediante `OffscreenTarget`; no se cambia automáticamente el preset de presentación. Bloom no es temporal, no hay TAA, HDR10, exposición automática, ray tracing ni F9. No se modifican CPU, gameplay, networking ni Bevy.
