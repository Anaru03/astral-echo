# 12 — F5A: backend GPU propio

Implementación validada el 21 de octubre de 2026 en Windows x86_64, Rust/Cargo
1.97.0. Se completa exclusivamente F5A. Bevy, gameplay, networking y simulación
permanecen intactos. F5B y F6 no se han iniciado.

## Arquitectura y contrato

`render-gpu` utiliza wgpu **29.0.4**, winit **0.30.13**, bytemuck **1.25.2** y
png **0.18**, versiones inspeccionadas en Cargo.lock y en las fuentes locales
antes de implementar. Todas las dependencias se resolvieron offline.

La única definición de la escena F3 y sus helpers está en
`render-core/src/demo.rs`. `RasterMesh` y `RasterVertex` también pertenecen a core.
CPU conserva sus imports mediante reexports y su wrapper de cámara conserva el
contrato de errores anterior. Dos tests adicionales verifican esa compatibilidad.
La escena compartida tiene 19 draws y 278 triángulos. No se duplicó su geometría.

`GpuContext` selecciona un adaptador compatible, crea device/queue y devuelve
errores tipados. La ausencia de adaptador es `GpuError::Unavailable`; los ejemplos
imprimen `SKIP GPU` y terminan limpiamente. Los tests dependientes de GPU indican
el mismo motivo con `--nocapture`. Otros errores son fallos reales, no skips.
Una máscara vacía de backends prueba la ausencia de forma determinista y comprueba
que CPU sigue renderizando. La ausencia física de todos los drivers no se probó
en este equipo. No hay selección automática CPU/GPU ni dependencia de GPU desde CPU.

`GpuRenderer` empaqueta las mallas en vertex/index buffers persistentes, con
índices u32 reajustados a offsets globales. El uniform buffer de MVP usa offsets
dinámicos alineados y se actualiza por frame; no se vuelven a subir las mallas.
`set_transform` permite actualizar un modelo sin reconstruir los recursos.
`ResourceStats` informa los tamaños de los buffers. La escena F3 utiliza
10 836 bytes de vértices, 3 336 de índices y 4 864 de uniforms: **19 036 bytes**
de payload GPU persistente, excluyendo overhead del driver y command buffers.
Existe además un scratch CPU de uniforms reutilizado.

WGSL recibe posición/color y aplica `projection * view * model` en columnas,
con clip depth 0..1 de core. Los colores se interpolan en perspectiva por hardware.
El pipeline es triangle-list indexado, sin backface culling, con Depth32Float,
comparación `Less` y escritura de profundidad. No incluye iluminación, blending,
PBR, sombras, postprocesado ni carga glTF. F3 es opaca.

Offscreen utiliza RGBA8 **Unorm**, clear `[8,10,18,255]` y depth clear 1.0,
para conservar la convención de color del CPU sin introducir una conversión sRGB.
Los targets color/depth y staging buffers son persistentes. Las filas de readback
se alinean a 256 bytes y se desempaquetan antes de exportar PNG RGBA8. La lectura
de profundidad también está disponible en `GpuImage`. Un resize inválido conserva
el target anterior; el resize válido recrea únicamente los recursos dependientes
de la resolución. Se rechazan índices, matrices, formatos y dimensiones inválidos.

## Ejecución

Desde la raíz del workspace:

```powershell
cargo run -p render-gpu --example gpu_probe --offline
cargo run -p render-gpu --example gpu_offscreen --offline
cargo run -p render-gpu --example gpu_compare --offline
cargo run -p render-gpu --example gpu_window --offline
cargo run -p render-gpu --example gpu_window --offline -- --frames 20
cargo test -p render-gpu --offline -- --nocapture
cargo bench -p render-gpu --bench gpu_bench --offline
```

Los PNG CPU/GPU se guardan en `target/f5a-output/`. CPU conserva su exportador
PNG y sus demos previas. GPU no depende de render-cpu en runtime: la dependencia
CPU se limita a pruebas, ejemplos comparativos y benchmarks.

## Ventana y recuperación

`SurfaceRenderer` crea una surface compatible con el adaptador y conserva los
buffers de la escena. Prefiere BGRA8/RGBA8 Unorm; si solo existe sRGB, una entrada
de fragment shader aplica la transferencia inversa para mantener el color legacy.
La ventana usa FIFO y un límite de 60 Hz mediante `WaitUntil`, sin busy waiting.
Resize/DPI se basan en el tamaño físico; la cámara actualiza su aspect ratio y
se recrea depth. Tamaño cero y oclusión suspenden los frames con `Wait`.

Se manejan los estados de wgpu 29 explícitamente:

| Estado | Acción |
| --- | --- |
| Success | Renderizar y presentar |
| Suboptimal | Presentar y reconfigurar después |
| Outdated | Reconfigurar y omitir el frame |
| Lost | Recrear surface, verificar compatibilidad y reconfigurar |
| Timeout / Occluded | Omitir y esperar la siguiente oportunidad |
| Validation | Devolver error y cerrar limpiamente |

Los errores no capturados y la pérdida del device se conservan en el contexto y
se propagan como errores. No se intenta reconstruir un device perdido ni cambiar
automáticamente a CPU. No se inyectaron fallos de driver, OOM o surface Lost;
sus rutas están implementadas pero no verificadas contra esos fallos reales.

Smoke tests reales sobre **NVIDIA GeForce RTX 4060 Laptop GPU**, driver **616.56**,
**Vulkan**, formato de surface **Bgra8Unorm**:

- Ejemplo limitado a 20 frames: presentó 20 y terminó con código **0**.
- Sesión nativa con resize a 801×601, minimizar, restaurar y cierre por evento:
  2 293 frames, 47,993 s, código **0**. La duración incluye inicialización;
  no es un benchmark de FPS sostenido.
- UI Automation confirmó tamaño y estados Minimized/Normal. La captura directa
  del escritorio con CopyFromScreen falló con «Controlador no válido».
  Se inspeccionaron visualmente los PNG CPU/GPU; no se afirma inspección visual
  directa de la ventana. El primer script de automatización no obtuvo ExitCode
  de Start-Process; la repetición nativa verificó el código real.

## Comparación CPU/GPU y tolerancia

Se comparan **todos** los píxeles RGBA y todos los valores depth, sin descartar
zonas de la escena. Tolerancia definida por el comparador: error medio absoluto
por canal de como máximo **0,25 niveles de 255**; como máximo **0,5 %** de píxeles
con algún canal diferente en más de **2**; como máximo **0,5 %** de valores depth
con diferencia superior a **1e-5**. Depth debe ser finito. Estos márgenes permiten
redondeo UNorm y cobertura de bordes por la precisión subpíxel del hardware;
no se exige igualdad binaria entre rasterizadores diferentes.

| Resolución | Error medio/canal | Píxeles color >2 | Depth >1e-5 | Máximo error depth |
| --- | ---: | ---: | ---: | ---: |
| 320×180 | 0,007856 | 2 / 57 600 | 2 | 0,002659 |
| 640×360 | 0,004538 | 0 / 230 400 | 0 | 4,17e-7 |
| 1280×720 | 0,004758 | 2 / 921 600 | 2 | 0,001179 |
| 321×181 | 0,004742 | 0 / 58 101 | 0 | 2,98e-7 |

Los dos outliers de las resoluciones afectadas son compatibles con diferencias de
cobertura en bordes; no se ocultan los máximos depth. Los PNG a 640×360 muestran
la misma escena al inspeccionarlos. El ejemplo `gpu_compare` guarda ambos PNG y
falla si se supera la tolerancia.

Los **12 tests nuevos** de GPU cubren: ausencia explícita y CPU independiente;
padding/overflow de readback; layouts inválidos; PNG round-trip RGBA y tamaños;
empaquetado/indexación y datos inválidos; F3 a cuatro resoluciones; determinismo,
resize y actualizaciones con recursos persistentes; clear sin draws; formatos y
transformaciones inválidos sin envenenar el device; perspectiva/winding;
oclusión de mallas near/far independiente del orden; clipping del near plane.
En este equipo las pruebas GPU se ejecutaron realmente: no hubo skips.

## Benchmark reproducible

Escena F3 compartida; release optimizado; **5 warmups + 50 muestras** por
resolución. CPU: AMD Ryzen 7 8845HS, 16 procesadores lógicos, rasterizador
secuencial original con framebuffer persistente. GPU: RTX 4060 Laptop Vulkan.
La comparación es contra ese CPU secuencial, no contra Rayon/Auto de F4.2.

CPU y GPU excluyen inicialización, PNG y readback del tiempo de render. GPU mide
tiempo de host desde actualización de MVP/submit hasta `Device::poll` completado,
no solo enqueue ni timestamps internos del hardware. CPU incluye clear y todas
sus etapas por frame. Se alternan mediciones CPU/GPU; readback se mide aparte.
Inicialización de contexto, empaquetado y pipeline: **3 266,487 ms**, fuera de las
muestras. No se mide presentación FIFO. Las cifras dependen de energía, carga y
driver del portátil; no implican rendimiento general de otras escenas/adaptadores.

Todos los tiempos siguientes están en milisegundos, corrida final:

| Resolución | CPU Avg / P95 / Min / Max | GPU Avg / P95 / Min / Max | Speedup render | Readback Avg |
| --- | --- | --- | ---: | ---: |
| 320×180 | 0,6051 / 1,1276 / 0,4138 / 1,3696 | 0,2129 / 0,2939 / 0,1171 / 0,3655 | 2,84× | 0,2434 |
| 640×360 | 1,6736 / 1,9133 / 1,5209 / 2,5604 | 0,3080 / 0,4215 / 0,2226 / 0,4905 | 5,43× | 1,4385 |
| 1280×720 | 6,4671 / 7,9722 / 5,6468 / 8,9109 | 0,4104 / 0,5255 / 0,3093 / 0,5925 | 15,76× | 3,7497 |

Readback incluye copia GPU→staging, sincronización, desempaquetado RGBA y depth,
y asignación de la imagen CPU. Sumándolo al render, los speedups aproximados son
**1,33× / 0,96× / 1,55×**: a 640×360 la exportación GPU resulta ligeramente más
lenta que el render CPU antes de codificar PNG. No se presenta el speedup de
render como si fuera el de exportación completa.

Payload persistente adicional color+depth+dos staging buffers:
**921 600 / 3 686 400 / 14 745 600 bytes**, respectivamente, además de los
19 036 bytes de buffers de escena. No incluye alignment interno del driver,
command buffers ni imágenes CPU temporales de readback.

## Validación y evidencia

Resultados reales; logs locales bajo `target/f5a-validation/`, excluidos de Git:

| Comando | Exit |
| --- | ---: |
| `cargo fmt -p render-core -p render-cpu -p render-present -p render-gpu -- --check` | 0 |
| `cargo check --workspace --all-targets --offline` | 0 |
| `cargo test --workspace --offline` | 0 |
| `cargo clippy -p render-core -p render-cpu -p render-present -p render-gpu --all-targets --offline -- -D warnings` | 0 |
| `cargo test -p render-gpu --offline -- --nocapture` | 0 |
| `cargo bench -p render-gpu --bench gpu_bench --offline` | 0 |
| `cargo run -p render-gpu --example gpu_compare --offline` | 0 |
| `cargo run -p render-gpu --example gpu_offscreen --offline` | 0 |
| `cargo run -p render-gpu --example gpu_window --offline -- --frames 20` | 0 |

Workspace: **166 tests**, ninguno eliminado: core **8**, CPU **129** (127
anteriores + 2 de compatibilidad), presentación CPU **17**, GPU **12**.
Clippy estricto se validó para todos los crates gráficos afectados; no se afirma
un nuevo resultado de Clippy global sobre gameplay. Cargo emite un aviso ambiental
de canonicalización de `C:\Users\sarah`; no impidió los comandos ni se ocultaron
diagnósticos Rust con atributos allow.

Quedan sin validar otros sistemas operativos, adaptadores, DX12/Metal, cambios
reales entre monitores con distinto DPI, captura visual directa del escritorio y
fallos de driver/surface forzados. No bloquean la ejecución Windows/Vulkan
demostrada, pero limitan la cobertura de portabilidad y recuperación. No hay
PBR, sombras, postprocesado, selector CPU/GPU ni integración nueva en Bevy.

## Commits locales

- `b6497b0` — tipos coloreados y escena F3 compartidos en core.
- `bd8df30` — GPU offscreen, buffers persistentes, WGSL y PNG.
- `df4b921` — ventana, resize y recuperación de surface.
- `ec42a2f` — tests y comparación CPU/GPU.
- `a6267b6` — benchmark reproducible de F3.
- Este documento se registra en un commit Conventional Commit separado;
  consultar `git log` para su hash.

No se realizó push. `AGENTS.md` preexistente y sin seguimiento se preserva fuera
de los commits. No se tocaron respaldos ni evidencias previas en target.
