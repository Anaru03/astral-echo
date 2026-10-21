# F4.3 — Presentación CPU en ventana

Validación: 20 de octubre de 2026, Windows x86_64. Alcance exclusivo F4.3;
F4.2 y sus benchmarks permanecen intactos. No se inicia F5.

## Arquitectura y ejecución

`crates/render-present` es miembro del workspace y consume `render-cpu`.
Usa `winit 0.30.13` para eventos y ventanas y `softbuffer 0.4.8` para presentar
píxeles por la API nativa. Su árbol de dependencias no incluye Bevy ni wgpu.
Los otros miembros históricos del workspace conservan sus dependencias.
En esta sesión fue necesario descargar softbuffer; winit ya estaba en caché.
Las validaciones posteriores se ejecutan offline. En una máquina nueva,
compilar primero sin `--offline` permite obtener las dependencias requeridas.

```powershell
cargo run -p render-present --release --offline --example cpu_window -- --preset low --raster auto
```

La escena es la referencia F3 de `render-cpu`. La demo prepara la escena una
sola vez y reutiliza el framebuffer entre frames. Cada draw usa el pipeline
tiled de F4.2 y el modo seleccionado, sin implementar otro rasterizador.

| Preset | Resolución CPU | Límite de FPS |
|---|---|---:|
| LOW | 640×360 | 30 |
| MEDIUM | 960×540 | 60 |
| HIGH | 1280×720 | 60 |

`--preset low|medium|high` y `--raster sequential|parallel|auto` seleccionan
ambas opciones; LOW y Auto son los valores iniciales. Durante la ejecución,
las teclas `1/2/3` cambian de preset y `S/P/A` seleccionan el modo de raster.
Escape o el botón de cierre terminan limpiamente la aplicación. La barra de
título muestra preset, resolución CPU, modo, tiempos de render/presentación y
límite de FPS. El rendimiento real puede ser inferior al límite en máquinas
lentas, especialmente usando el perfil debug.

## Resize, DPI y formato de píxeles

La resolución de render depende del preset, no del tamaño de la ventana.
Un resize solo cambia la superficie de presentación; cambiar de preset sí
redimensiona el framebuffer CPU y ajusta el limitador. La ventana se solicita
en unidades lógicas y `softbuffer` se dimensiona con los píxeles físicos que
reporta `winit`. `ScaleFactorChanged` y `Resized` actualizan ese estado sin
multiplicar dos veces por el factor de DPI.

El escalado conserva la proporción de la imagen y centra el resultado con
bandas negras. Se escribe directamente en el buffer de `softbuffer`, sin
crear una imagen escalada intermedia. Para cada píxel de destino se utiliza
`floor(coordenada_destino * tamaño_origen / tamaño_destino)`: nearest-neighbor
sin filtrado ni interpolación de colores. Se admiten aumentos fraccionarios,
reducciones y dimensiones físicas no divisibles.

RGBA8 se convierte al entero `0x00RRGGBB` que espera softbuffer. El canal alfa
no forma parte del formato de presentación: se conserva RGB y se pone a cero
el byte superior. Esto no modifica el framebuffer CPU ni sus datos alfa o de
profundidad, y no agrega una operación nueva de blending.

Una superficie de ancho o alto cero queda suspendida: no se llama a
`Surface::resize` con valores nulos ni se renderiza. Las ventanas ocluidas y
la aplicación suspendida también quedan en espera. Al restaurar o reanudar,
se recrean los recursos necesarios. Los préstamos de `Surface` pertenecen a
handles `Arc<Window>`; no hay lifetimes artificiales, `unsafe` ni referencias
a una ventana destruida. La superficie se libera antes del contexto y de
la última referencia de la ventana.

## Limitador y métricas

El event loop utiliza `ControlFlow::WaitUntil` para el próximo deadline y
`ControlFlow::Wait` cuando está pausado o espera un redraw ya solicitado.
`FramePacer` filtra también los redraws adelantados que solicite el sistema.
No usa polling continuo ni busy waiting. Después de un frame tardío, el
siguiente deadline se calcula desde su inicio real, sin ráfagas para recuperar
frames perdidos. Los timers del sistema pueden introducir variación en la
cadencia; el límite no equivale a sincronización vertical ni garantiza FPS.

`FrameMetrics` registra por separado:

- Render: tiempo del callback que produce el framebuffer CPU.
- Presentación: adquisición del buffer, escalado/conversión y `present()`.
- Frames presentados, último tiempo y promedio de cada categoría.

Las esperas del limitador, cambios de título, eventos de resize y exportación
PNG no se suman a esas dos categorías. El tiempo de `present()` mide la
llamada nativa, no la latencia hasta que el monitor muestra el píxel.

`run_window(WindowConfig, callback)` devuelve `RunReport`, con métricas,
tiempo de sesión y el framebuffer final. Los errores de render, superficie o
event loop se propagan al llamador después de liberar los recursos.

## PNG y ejecución acotada

```powershell
cargo run -p render-present --release --offline --example cpu_window -- --preset high --raster parallel --frames 120 --png target/render-output/window-high.png
cargo run -p render-cpu --offline --example render_demo
```

`--frames` limita la cantidad de presentaciones y permite smoke tests sin
dejar una ventana abierta indefinidamente. `--png` escribe el framebuffer CPU
final después de cerrar el event loop: no mide el disco como render ni
presentación. Mantiene la resolución del preset, sin las bandas del escalado.
La demo PNG anterior de `render-cpu` sigue disponible y sin cambios.

## Pruebas y validación

Los 17 tests de `tests/headless.rs` no crean ventanas. Cubren el contrato de
presets y modos, packing RGB, orientación, escalado entero/fraccionario y
reducción, letterbox/pillarbox, tamaños impares, destino de un píxel, buffers
inválidos y overflow, resize nulo/restauración, DPI fraccionario y errores de
DPI, deadlines y frames tardíos, y métricas independientes.

| Comando | Código | Resultado |
|---|---:|---|
| `cargo fmt -p render-present -- --check` | 0 | Formato del crate nuevo |
| `cargo check --workspace --all-targets --offline` | 0 | Workspace completo |
| `cargo test --workspace --offline` | 0 | 152 aprobados: 127 CPU + 17 present + 8 core |
| `cargo clippy -p render-present -p render-cpu --all-targets --offline -- -D warnings` | 0 | Crates afectados y dependencias |
| `cargo clippy --workspace --all-targets --offline -- -D warnings` | 101 | 14 diagnósticos en gameplay intacto |
| `cargo clippy -p gameplay --all-targets --offline -- -D warnings` | 101 | Reproduce el bloqueo de forma aislada |
| `cargo run -p render-cpu --example render_demo --offline` | 0 | Exportación PNG anterior |
| `cargo test -p render-present --offline -- --list` | 0 | 17 tests headless descubiertos |
| `cargo build -p render-present --example cpu_window --release --offline` | 0 | Demo release |

El bloqueo de Clippy global está en código que no se modificó:
`encounter.rs:73`, `navigation.rs:904`, `systems.rs:48` y once diagnósticos en
`world.rs` (comparaciones, ramas duplicadas, búsquedas, manejo de Option y
complejidad de tipos). La compilación y los tests del workspace pasan; no se
presenta Clippy global como aprobado. La regla de `AGENTS.md` que excluye
cambios en gameplay, networking y Bevy sin autorización impide corregir esos
archivos dentro de está entrega. No se agregan allows para ocultar el bloqueo.


## Prueba nativa y límite visual del entorno

La demo LOW/Auto presentó 1450 frames y terminó con código 0 al recibir el
cierre nativo. Windows UI Automation confirmó resize exterior a 801×601,
minimizar, restaurar y cerrar. La sesión midió 2,562 ms de render promedio y
1,353 ms de presentación promedio durante 51,413 s, incluidos cambios de
tamaño y pausa. Es telemetría de smoke test, no un benchmark estadístico ni
una comparación con F4.2.

La captura de escritorio con `Graphics.CopyFromScreen` falló con
«Controlador no válido» y la ventana no pudo recibir foco mediante UI
Automation en esta sesión. **No se certifica inspección visual de los píxeles
de la ventana.** La exportación PNG y las comparaciones headless sí están
disponibles; una captura fallida no se trata como evidencia visual.

Tres smoke tests adicionales ejecutaron 30 frames cada uno, con cierre por
límite y exportación PNG, todos con código 0:

| Preset / modo | Render promedio ms | Presentación promedio ms | Sesión s |
|---|---:|---:|---:|
| LOW / Sequential | 2.367 | 1.211 | 1.033 |
| MEDIUM / Parallel | 3.887 | 2.582 | 0.550 |
| HIGH / Auto | 3.836 | 4.466 | 0.547 |

Las dimensiones de los tres PNG coinciden con sus presets. El PNG LOW de la
demo nueva es idéntico byte a byte a la referencia F3 640x360 del benchmark
F4.2 conservado en target. Esto verifica que el render CPU no cambió; no
sustituye la inspección de la ventana ni mide latencia de pantalla.


Antes de certificar el comportamiento visual en un escritorio interactivo,
ejecutar la demo en release, probar `1/2/3` y `S/P/A`, redimensionar a ventanas
anchas/altas y pequeñas, minimizar/restaurar, arrastrar entre monitores con
DPI distintos y cerrar con Escape y con el botón de ventana. Comprobar que
RGB mantiene su orientación, las bandas quedan negras y no aparecen restos
del buffer anterior. Los tests headless cubren la aritmética de DPI; no
sustituyen una prueba física con dos monitores. Linux/macOS no se ejecutaron
en esta validación Windows.

Las salidas de validación y los PNG temporales se guardan en
`target/f43-validation/`, fuera de los commits. `AGENTS.md`, que ya estaba sin
seguimiento al comenzar, se conserva y no se incluye en los commits.

## Referencias de las APIs

- [Context y propiedad del display](https://docs.rs/softbuffer/0.4.8/softbuffer/struct.Context.html).
- [Surface, resize físico y buffer](https://docs.rs/softbuffer/0.4.8/softbuffer/struct.Surface.html).
- [Eventos de ventana de winit](https://docs.rs/winit/0.30.13/winit/event/enum.WindowEvent.html).
