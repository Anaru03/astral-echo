# 07 — Raster CPU básico (F2, cierre)

Crate nuevo `crates/render-cpu`, sin ventana, sin GPU, sin hilos. Deps:
`render-core` + `glam 0.32` + `png 0.18` (esta última solo para escribir los
goldens).

## Arquitectura

```
Malla (RasterMesh: vértices + índices)
   │  mvp = P · V · M           (render_core::Camera + Transform)
   ▼
Clip-space homogeneo (ClipVertex: clip: Vec4, color: Vec4)
   │  clip_polygon_near  — Sutherland–Hodgman contra w > 1e-6
   ▼
Polígono recortado (3..n vértices) → abanico de triángulos
   │  to_screen — división por w → NDC → viewport → píxeles
   ▼
Rasterizado por edge functions (muestra en centro de píxel)
   │  barycéntricas → profundidad (z de NDC) + color (perspectivo)
   ▼
Framebuffer (color RGBA8 + depth f32)  ──►  write_png → triangle.png / cube.png
```

Módulos: `clip`, `color`, `error`, `framebuffer`, `geometry`, `image`,
`rasterizer`, `viewport`. API pública re-exportada en `src/lib.rs`.

## Decisiones de diseño

- **Reutiliza `render-core`, no lo duplica.** `Camera` (view + projection),
  `Transform` (modelo) y `perspective_rh_wgpu` son los del core; el crate solo
  añade la parte que falta: framebuffer, clip, raster y escritura PNG. El
  `mvp` se arma como `projection * view * model`.
- **Profundidad 0..1**, coherente con `perspective_rh_wgpu` y con wgpu/WGSL.
  El framebuffer se inicializa y se limpia a `1.0`; el test de profundidad
  rechaza cuando `depth >= existing` (igualdad = no pasa, evita z-fighting
  determinista entre triángulos coplanares).
- **Near plane en espacio clip**, no en espacio vista: se recorta contra
  `w > 1e-6`. Es el único recorte implementado; el far plane y los laterales
  quedan para F3.
- **Interpolación de color perspectiva-correcta**: se interpola `1/w` con las
  barycéntricas y se renormaliza, en vez de interpolar el color linealmente.
- **Winding configurable** (`Both` / `Front` / `Back`): la cara frontal se
  determina por el signo del área de la edge function, no por la normal
  (el demo usa `Both` porque la cámara puede ver la cara trasera del cubo).
- **Muestreo en centro de píxel** (`px + 0.5`), con bounding box del triángulo
  recortado a la ventana como recorrido previo.

## API pública

| Función / tipo | Rol |
| --- | --- |
| `Framebuffer::new(w, h)` / `clear` / `resize` | color RGBA8 + depth f32 |
| `Viewport::from_size` / `ndc_to_pixel` / `pixel_to_ndc` | NDC ↔ píxeles (Y invertido) |
| `clip_polygon_near` / `clip_triangle_near` | recorte del near plane |
| `Rasterizer::draw_mesh` / `draw_triangle` | rasterizado + `RasterStats` |
| `render_mesh(fb, viewport, config, mesh, camera, transform)` | atajo de una llamada |
| `write_png(path, fb)` / `write_png_bytes(fb)` | salida PNG RGBA8 |
| `RasterMesh::triangle` / `RasterMesh::cube` | geometría de prueba |

Errores: `RasterError` (tamaño de framebuffer, viewport, vértice no finito,
clip inválido) e `ImageError` (codificación PNG / I/O). La API que puede fallar
devuelve `Result`; el código de dibujo que no puede fallar no lo hace.


## Validación real

- `cargo fmt -p render-cpu -- --check` → exit 0.
- `cargo check --workspace --offline` → exit 0.
- `cargo test --workspace --offline` → exit 0. Suites nuevos de `render-cpu`:
  - `transform_viewport` (13): MVP, proyección/viewport, inversión NDC↔píxel,
    viewport con offset, viewport inválido.
  - `rasterization` (10): cobertura de píxeles, triángulo fuera de pantalla,
    degenerado y colineal, ambos windings, culling por winding, interpolación
    de color, vértice no finito, vértices detrás de la cámara, cruce del near
    plane.
  - `depth_order` (5): quad cercano gana, quad lejano no pisa al cercano,
    escritura de profundidad, `depth_test` desactivado, `depth_write`
    desactivado.
  - `framebuffer_image` (12): dimensiones inválidas, estado inicial, acceso
    fuera de rango, clear, resize, conversiones de color, PNG reproducible y
    decodificable.
  - `clip` (6): polígono delante/detrás/cruzando el near plane, interpolación
    de color en el vértice recortado, no finito, polígono degenerado.
- `cargo run --release -p render-cpu --example render_demo` → exit 0.
  Genera `target/render-output/triangle.png` (640×480, 12 271 px con color) y
  `target/render-output/cube.png` (640×480, 54 065 px escritos, 39 381
  rechazados por profundidad → oclusión correcta). Ambos verificados a ojo:
  el triángulo muestra el gradiente barycéntrico y el cubo muestra la
  perspectiva y la oclusión de caras.

## Errores encontrados y corregidos

1. `png::Compression::Default` no existe en `png 0.18` (es `Default::default()`
   sobre la variante `Default`): el enum se llama `Compression` y su variante
   es `Default`, pero se accede como `png::Compression::default()`.
2. `Framebuffer` no podía derivar `Eq` porque contiene `Vec<f32>`: se quitó
   `Eq` de la derivación, dejando `PartialEq`.
3. Prestamo doble en `set_depth`: `self.depth[self.index(x, y)]` pedía `&self`
   y `&mut self` a la vez; se calculó el índice en una variable local.
4. `RasterError2` quedó como resto de una refactorización: se eliminó.
5. `RasterStats` marcaba **todo** triángulo como `triangles_clipped`. Se
   separó el caso "recortado" (`clipped.len() != clip.len()`) del caso
   "descartado por quedar fuera de la ventana" (`triangles_offscreen`).
6. Test ajeno de `render-core` (`mvp_composition`) fallaba: el vértice de
   prueba estaba en `z = +52` mientras la cámara del test mira hacia `-Z` desde
   el origen, es decir, **detrás** de la cámara; `clip.w` negativo era la
   respuesta correcta. Se corrigió el test (a `z = -5`), no el código de
   producción.

## Riesgos pendientes / alcance de F3

- Solo se recorta el near plane: falta far plane, laterales, y el frustum
  culling por objeto (`Frustum` ya existe en `render-core`).
- Sin ensamblado de índices ni strip/fan: `RasterMesh` solo expone
  triángulos sueltos.
- El cubo del demo es un sólido opaco de un solo color; no hay iluminación,
  ni materiales, ni texturas, ni antialiasing.
- La iluminación (F8) y el pipeline por etapas instrumentado (F3) quedan
  fuera de este crate.
- `render-cpu` no tiene todavía `Cargo.lock` propio: usa el lock del
  workspace.

## Estado definitivo

F2 lista: crate compilado y formateado, 46 tests propios en verde, el
workspace completo en verde, demo en release con exit 0 y los dos goldens
`triangle.png` / `cube.png` generados y verificados. **No se inició F3.**
