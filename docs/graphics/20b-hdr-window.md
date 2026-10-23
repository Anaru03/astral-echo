# F8C.2 — Presentación GPU HDR sobre surface SDR

`SurfaceRenderer` ahora expone `set_postprocess_preset` y `set_postprocess`. LOW conserva el pipeline LDR existente y renderiza directamente al texture view de la surface. MEDIUM/HIGH crean una segunda ruta persistente: geometría e iluminación escriben `Rgba16Float`, `PostProcessPass` reutiliza el shader ACES/bloom/FXAA de F8C.1 y la composición escribe sobre el formato SDR real de la surface (`Rgba8Unorm`, `Bgra8Unorm` o sus variantes sRGB soportadas).

```rust
surface.set_postprocess_preset(render_gpu::PostProcessPreset::High)?;
```

El renderer HDR se crea una vez al activar el preset; la lista de geometría, caché, instancing, sombras, culling, LOD y depth siguen compartidas conceptualmente con la escena preparada. En snapshots posteriores se sincronizan ambos renderers. El HDR target, su view y la fuente del postprocesado se recrean únicamente cuando cambia la resolución. Cambiar exposición, bloom, FXAA o preset no reconstruye recursos por frame.

La surface mantiene su formato SDR. En un formato sRGB, la composición entrega valores lineales al attachment y el hardware hace la conversión sRGB una sola vez. En unorm lineal no se añade gamma manual. LOW no crea target float ni ejecuta composición.

La API y el pipeline se validaron con `cargo check`, Clippy y las pruebas de F8C.1. La prueba headless de HDR offscreen comprueba emissive 8, `Rgba16Float`, ACES y profundidad. No se pudo abrir una ventana real en esta ejecución; por ello la equivalencia visual y el coste de presentación de `SurfaceRenderer` quedan pendientes de validar en un entorno con display. No se declara la ventana HDR validada sin esa ejecución.

El benchmark offscreen de referencia en RTX 4060 Laptop/Vulkan (target 1280×720 y 1920×1080) permanece: LOW 0,116/0,126 ms, MEDIUM 0,215/0,246 ms y HIGH 0,194/0,257 ms. Esas cifras no son mediciones de surface/present; el coste de adquisición y presentación real depende del compositor, VSync y formato SDR.

No se implementan HDR10, TAA, ray tracing, gameplay, networking, Bevy ni F9.
## Hot-swap de presets HDR

La escena preparada conservaba nodos visibles (`5`) y triángulos (`48`) durante los
cambios LOW→MEDIUM/HIGH; el problema estaba en el primer frame HDR después de activar
el preset o redimensionar. `SurfaceRenderer::present_snapshot` ahora verifica que un
postprocesado habilitado tenga target HDR y vista válidos, y recrea los recursos
dependientes de resolución antes de sincronizar `PreparedScene` y presentar.

Esto conserva depth, culling, cámara y la composición ACES/bloom/FXAA. Las pruebas
headless GPU pasan salvo una regresión preexistente de equivalencia Lambert (`904`
outliers frente al umbral `30`); no se atribuye a este cambio. La validación visual
Windows/Vulkan sigue pendiente sin display.
