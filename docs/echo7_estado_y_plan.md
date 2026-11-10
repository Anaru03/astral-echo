# Auditoría de ECHO-7 y plan por fases

Fecha de auditoría: 2026-11-09. Alcance: inspección del árbol actual, código, assets y comprobaciones ejecutadas en este entorno. No se eliminaron ni modificaron assets. El único archivo nuevo de esta auditoría es este informe.

## A. Resumen del estado real

El flujo de juego registra y carga el montaje `assets/models/echo_7/montajes/ECHO7_Completa_DosCubiertas.obj` como `MeshHandle::SHIP_PLACEHOLDER`, mediante `crates/game/src/model_registry.rs`. La representación visual de la nave sí está conectada al snapshot CPU; el interior lógico, las interactuables y la cámara son representaciones separadas creadas por `crates/game/src/render_bridge.rs`.

La escalera es visible en el montaje y también existe una rampa lógica. La evidencia geométrica inspeccionada en la OBJ muestra 24 peldaños, con extensión local aproximada `x=-4.2..3.0`, ancho local `y=-7.8..-6.2` y altura `z=0..4.2`. La lógica histórica usaba límites mundiales estrechos en X y avance en Z sin derivarlos del modelo. El árbol actual contiene una corrección local que intenta invertir la transformación visual, pero su prueba exacta de superficies aún falla (`2.7267327` frente a `2.73`), por lo que no se puede declarar cerrada ni visualmente validada.

El backend CPU está implementado y conectado: carga OBJ asíncrona, mallas persistentes, materiales/UV, clipping, rasterización, fragmentos y profundidad. Hay métricas por etapa y rutas de tiles/paralelismo. No hay evidencia de una validación visual de ventana en esta auditoría.

## B. Comandos y modos reproducibles

```text
cargo run -p game -- --visual
cargo run -p render-present --example cpu_window
cargo run -p render-cpu --example render_demo
cargo test -p game
cargo test -p render-core
cargo test -p render-cpu
cargo test -p render-present
cargo check -p game
```

Diagnóstico de escalera detectado en `crates/game/src/visual.rs`: `ASTRAL_ECHO_STAIR_DIAGNOSTIC=1` o `true`; muestra pies, apoyo y bounds en el feedback HUD. El informe no afirma que el dibujo coincida visualmente: el modo dibuja texto/estado y no sustituye una captura comparativa.

El montaje visual es el modelo indicado por `model_registry.rs`; el arranque de `VisualBridge` inicializa nave y jugador con la misma posición de nave. La prueba de ventana no se ejecutó en esta auditoría.

## C. Estado por sistema

| Sistema | Estado | Evidencia | Prueba | Pendiente |
|---|---|---|---|---|
| Registro de modelo ECHO-7 | Implementado y conectado | `game/src/model_registry.rs::registered_models` | `cargo check -p game` previo correcto | Verificar carga real hasta `CpuReady` en ventana |
| Montaje OBJ CPU | Implementado | `render-core/src/cpu_model_loader.rs`, `Verificacion_Archivos.json` | Fixtures del loader; no prueba del montaje completo | Medir memoria/tiempo del montaje real |
| Transformación nave | Implementada | `render_bridge.rs`: escala `0.65`, centro, yaw visual `-90°`, conversión Z-up | `echo7_z_up_conversion_maps_to_world_y_without_reflection` | Captura con ejes y vértices de referencia |
| Materiales y texturas | Parcialmente conectado | `model_registry.rs`, `render-core` material/texture; `Materiales.json` | Tests de textured model y pipeline | Confirmar todas las rutas `map_Kd` del montaje en runtime |
| Interior lógico | Implementado separado del modelo | `render_bridge.rs::interior_nodes`, `interior_camera` | Tests de snapshot/cámara | Alinear paredes, muebles y límites con geometría |
| Movimiento | Implementado | `visual.rs::update_interior_motion`, `gameplay/src/systems.rs::move_player` | Tests de movimiento existentes | Probar esquinas, obstáculos, frame time y yaw no cero |
| Escalera | En corrección, no cerrada | `render_bridge.rs::echo7_walkable_height` y objetos `ECHO7_28...Peldano` | La regresión visual pasó en una ejecución anterior; la prueba `echo7` actual falló por tolerancia exacta | Corregir tolerancia/contrato y ejecutar ventana |
| Descanso/balcón | Lógica provisional | Constantes de upper deck y `interior_nodes` | Cobertura parcial | Derivar bordes del OBJ y probar lateral elevado |
| Interacciones | Implementadas en gameplay | `visual.rs`, `render_bridge.rs`, crates `gameplay` | Muchas pruebas de gameplay | Revisar coincidencia con piezas visibles |
| Luces | Implementadas en snapshot/CPU | `render_bridge.rs` luces interiores, `render-core/src/lighting.rs` | `render-core` lighting tests | Separar y verificar visualmente interior/exterior |
| Sombras/transparencia/emisión | No demostradas para ECHO-7 | APIs existen parcialmente en render-core | No hay evidencia específica del montaje | Confirmar material por material |
| Exterior/transiciones | Parcialmente implementado | cámaras de vuelo/pilotaje/observación en `render_bridge.rs` | Tests de cámaras y navegación | Capturas de transición, aterrizaje y retorno |
| Rendimiento | Instrumentación disponible, baseline incompleto | `StageMetrics`, ejemplos de benchmark y CSV histórico | No se midió el montaje actual | Medir FPS/CPU/RAM y comparar montaje contra componentes |

## D. Transformaciones y coordenadas

La OBJ declara `# ... meters; Z up`. La cadena usada por la nave es, conceptualmente:

```text
v OBJ local
  -> escala uniforme 0.65 y corrección de centro ECHO7_MODEL_CENTER
  -> rotación X -90° (Z local pasa a Y mundial)
  -> yaw visual -90° más orientación de la nave
  -> traslación a ship_position
  -> view/projection de Camera
  -> clipping, rasterización y framebuffer CPU
```

El transform de escena se construye en `render_bridge.rs` alrededor de `model_rotation` y `transform.translation`. Las normales se procesan en el pipeline CPU; la auditoría confirma la existencia del camino de normal/shading, pero no una captura que demuestre cada normal del montaje.

Para la escalera, los vértices de los objetos `ECHO7_28_Escalera_Entre_Cubiertas__Peldano_01..24` dan 24 peldaños y el rango citado arriba. Esto demuestra que el ancho está en el eje local Y y el avance en el eje local X; después de la rotación, el apoyo debe consultar localmente, no comparar constantes mundiales independientes. Una prueba que solo recorra las mismas constantes de implementación no sería suficiente.

El jugador usa posición autoritativa de gameplay. El ajuste de apoyo ocurre después de `MovePlayer`; la cámara interior toma esa posición y suma `ECHO7_INTERIOR_EYE_HEIGHT`. Esto deja un punto sensible: ningún paso posterior debe sobrescribir la altura corregida ni mezclar el movimiento vertical con el contrato horizontal de `MovePlayer`.

## E. Assets y rutas canónicas

### Montaje y componentes

- Montaje cargado por el juego: `assets/models/echo_7/montajes/ECHO7_Completa_DosCubiertas.obj` y `.mtl`.
- Editables/escenas Blender: el mismo montaje en `.blend`, además de `montajes/ECHO7_Maestro_Vinculado.blend` y `ECHO7_Montaje_Inspeccion.blend`.
- Componentes: `componentes/ECHO7_Exterior.obj`, `ECHO7_Interior_Inferior.obj`, `ECHO7_Interior_Superior.obj`, `ECHO7_Nucleo_Entre_Cubiertas.obj`.
- Exportaciones separadas: `separados/ECHO7_01...ECHO7_30...` y `modelos_separados/` con OBJ/BLEND por pieza.
- Escalera aislada: `separados/ECHO7_28_Escalera_Entre_Cubiertas.obj`; ascensor: `ECHO7_29_Ascensor_Carga.obj`; exterior unificado: `ECHO7_30_Exterior_Unificado.obj`.
- Reparables/variantes: `repuestos/` y `kit_dano_reparacion/`, con sano/dañado.

`Verificacion_Archivos.json` reporta para el montaje 5.533 objetos, 142.720 vértices y 263.388 triángulos; para la escalera separada, 45 objetos, 1.480 vértices y 2.780 triángulos. Es metadato de exportación, no una medición de draws CPU por cuadro.

Los MTL referenciados deben revisarse junto al montaje. `Materiales.json` clasifica `Panel_Verde` como pintura original; `Casco_Marfil`, metales, vidrio, pantallas, tela, vegetación, coral y panel solar como bases provisionales con PNG/RGB. La auditoría no elimina duplicados ni decide cuál exportación es fuente de autoridad.

## F. Renderizado CPU e iluminación

El loader asíncrono crea un `RasterMesh` persistente en `Arc`; el registro evita recargar una ruta ya solicitada. El snapshot aporta nodos, transform, material, cámara y luces. El pipeline enumera `Vertex -> Assembly -> Clipping -> Raster -> Fragment -> Depth`, con interpolación de atributos/UV y estadísticas de fragmentos, descarte y píxeles. El depth test/write es configurable y está activo por defecto. Existen rutas tiled/paralelas y pruebas específicas, pero no se midió el montaje completo aquí.

`LightingEnvironment` admite un directional y hasta cuatro point lights, con Lambert/Blinn-Phong/PBR en la API. Las luces no son automáticamente las mallas de lámpara: deben distinguirse en el snapshot. `render_bridge.rs` contiene luces interiores; no se hizo una captura para confirmar rango, atenuación y normales. Sombras aparecen como infraestructura de render-core, pero no están demostradas como efecto visible de ECHO-7. Transparencia, emisión y materiales no texturados deben verificarse por material.

## G. Fallos confirmados, hipótesis y datos faltantes

### Confirmados

1. El modelo de juego es el montaje completo, no una combinación explícita de componentes separados.
2. La geometría de escalera usa X local para avanzar y Y local como ancho; la rampa mundial anterior no estaba demostrablemente ligada a esos vértices.
3. La prueba actual `cargo test -p game echo7 -- --nocapture` falló en la aserción de altura por diferencia numérica (`2.7267327` contra `2.73`) durante esta auditoría.
4. Hay muchos cambios locales ajenos: `git status --short` reportó 141 entradas medidas previamente, incluyendo gameplay, render y assets; no se atribuyen a esta auditoría.

### Hipótesis

- El fallo de ventana puede incluir además un desacuerdo entre centro/transform del montaje y el shell interior lógico.
- La escalera podría verse atravesable aunque el apoyo lógico sea correcto si el jugador entra por lateral, por debajo del balcón o cruza demasiada distancia en un frame.
- El montaje completo puede tener coste innecesario durante vuelo si no existe LOD/culling específico por interior/exterior.

### Datos faltantes

- Captura CPU de ventana con overlay de rampa y ejes.
- Estado `CpuReady` y error de carga del montaje en una ejecución real.
- Conteo de draws/triángulos realmente enviados por frame y memoria residente.
- Verificación material por material de MTL, `map_Kd`, orientación PNG y sombras/emisión.
- Contrato acordado para ascensor, puertas y colisiones de muebles.

## H. Plan por fases

### Fase 1 — Contrato geométrico de escalera

Objetivo: derivar bounds, sentido, ancho, niveles y transformación desde la OBJ real. Dependencias: ninguna; usar el montaje canónico y la transformación existente. Archivos probables: `game/src/render_bridge.rs`, `game/src/visual.rs`, fixture o herramienta de auditoría. Fuera: gameplay nuevo, ascensor y materiales. Pruebas: fixture independiente que parsea o consume datos extraídos de la OBJ, ascenso/descenso/lateral/bajo balcón durante varios frames, nave trasladada y yaw. Aceptación: prueba pasa y overlay/captura muestra coincidencia visual. Commit independiente posible.

### Fase 2 — Circulación y colisiones interiores

Objetivo: continuidad suelo-escalera-descanso y límites con paredes, techo, muebles y puertas. Dependencia: Fase 1. Archivos: `visual.rs`, adaptadores de solo lectura y fixtures gráficos. Fuera: cambiar reglas autoritativas de gameplay. Aceptación: recorridos deterministas con distintos delta, sin atravesamiento ni salto lateral.

### Fase 3 — Interacciones y puertas

Objetivo: alinear estaciones, reparables, puertas y piezas visibles. Dependencia: circulación estable. Archivos: `render_bridge.rs`, `visual.rs`, contratos públicos de `gameplay`. Fuera: nuevas reglas de misión/economía. Aceptación: radios y obstrucciones verificadas con fixtures.

### Fase 4 — Iluminación interior/exterior

Objetivo: documentar y estabilizar fuentes reales, espacios, normales y atenuación. Dependencia: geometría y materiales identificados. Aceptación: capturas CPU reproducibles interior/exterior y tests de snapshot.

### Fase 5 — Pintura y materiales

Objetivo: sustituir bases provisionales donde existan texturas originales y validar MTL/PNG. Dependencia: pipeline y luces estables. Fuera: cambiar gameplay. Aceptación: materiales esperados en snapshot y comparación visual.

### Fase 6 — Exterior, transiciones y rendimiento

Objetivo: validar pilotaje, aterrizaje, cámaras, paneles, propulsión, rampa/tren y coste del montaje. Dependencia: fases anteriores. Aceptación: pruebas de transición, benchmark con métricas reales y criterio CPU académico documentado.

## I. Próxima tarea concreta

Crear un fixture de auditoría de escalera que lea los vértices de `ECHO7_28_Escalera_Entre_Cubiertas.obj`, calcule bounds y pruebe la transformación para al menos dos traslaciones y dos yaw. Luego corregir la tolerancia/contrato de la prueba actual y ejecutar una captura CPU con `ASTRAL_ECHO_STAIR_DIAGNOSTIC=1`. No avanzar a iluminación ni pintura hasta que esa captura y la prueba geométrica pasen.

## Separación académica/opcional

Requisitos académicos: backend CPU obligatorio, OBJ/MTL/PNG, UV, transformaciones, clipping, rasterización, profundidad, luces y pruebas reproducibles. Mejoras opcionales: GPU/wgpu, LOD avanzado, sombras completas, animaciones decorativas y mayor fidelidad de materiales. Ninguna mejora opcional debe bloquear la corrección geométrica ni sustituir la validación CPU.

## J. Cierre de la etapa de circulación

Validación manual: el usuario confirmó en ventana que puede subir y recorrer el balcón, que sus bordes bloquean y que la escalera no permite salida lateral ni suspensión. Esta comprobación no sustituye una captura automatizada: la ventana no fue ejecutada en esta sesión.

Validación automatizada ejecutada:

- `cargo test -p game interior_motion_supports_real_stair_ascent_and_descent -- --nocapture`: pasa.
- `cargo test -p game echo7 -- --nocapture`: pasa para las tres pruebas ECHO-7 de transformación, superficies y balcón.
- `cargo check -p game`: pasa; permanecen warnings preexistentes no relacionados.
- `visual_runtime_starts_echo7_in_clear_space`: sigue fallando en una aserción de distancia de detección de enemigos/Nareth (línea 4175/actualizada por el árbol local), no en movimiento, apoyo, escalera, balcón ni cámara interior. No se desactivó ni se modificó.

La validación de la escalera subdivide ahora según distancia horizontal: cada muestra está separada como máximo `0.05` unidades. Con el límite actual de `delta_ms <= 50` ms y `max_speed=8`, el peor desplazamiento es `0.4` y produce ocho muestras; con pasos menores produce menos, y si el límite cambia la subdivisión se adapta. La separación lateral transitable mínima de la geometría de escalera, después del radio del jugador, es aproximadamente `0.14` unidades, superior al paso máximo de muestreo. La comprobación conserva la posición válida y recalcula el apoyo.

Archivos/hunks de esta etapa: `crates/game/src/render_bridge.rs` (clasificación explícita de superficies y límites laterales locales de escalera) y `crates/game/src/visual.rs` (revalidación del trayecto, conservación de posición y subdivisión por distancia). No se modificaron assets, iluminación, texturas, controles ni el test ajeno.

## K. Colisiones de la cubierta inferior

La cubierta inferior conserva paredes estáticas por segmentos en coordenadas locales de ECHO-7, con el hueco de la escalera exento y radio del jugador aplicado. El trayecto se subdivide con el mismo límite de `0.05` usado para la escalera; una muestra inválida conserva la última posición válida y permite el deslizamiento que no cruza la pared. La altura local limita el obstáculo a la cubierta inferior.

Gameplay expone estado autoritativo de puertas (`Closed`, `Open`, `Locked`), pero el runtime visual solo registra actualmente la puerta del puente; no existen puertas interiores inferiores registradas. La colisión consulta ese estado real y no inventa estados: una puerta abierta conserva su hueco y una cerrada no lo abre. Las pruebas automáticas cubren pared frontal, diagonal, esquina, traslado/yaw y puerta abierta/cerrada mediante la transformación existente. La comprobación en ventana queda pendiente.

## L. Mobiliario grande de la cubierta inferior

Se añadieron volúmenes separados para las familias grandes identificadas en `Asignaciones_Materiales.csv`: mesas, cápsulas, armarios y jardineras. No se incluyen hojas, botones, cables ni decoración pequeña. Cada volumen se consulta dentro de la validación de paredes existente, usa el mismo radio y transformación local, y solo bloquea hasta su altura; las superficies superiores y los corredores laterales permanecen transitables. Las posiciones de interacción de las consolas no se convierten en una caja global, por lo que conservan acceso lateral. La regresión cubre choque por familia, espacio libre, traslación y yaw. La validación visual en ventana sigue pendiente.

