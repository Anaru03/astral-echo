# 03 — Dependencias

Base lock: `bevy 0.19.1`, `glam 0.32.1` (transitiva de Bevy, reutilizada).

| Crate | Uso | Dónde | Estado F1 |
|---|---|---|---|
| `glam 0.32` | `Mat4/Quat/Vec` | `render-core` | agregada, verificada en `cargo tree` |
| `bytemuck` | `Pod` para GPU | ninguno | **no agregada** (sin necesidad concreta en F1) |
| `rayon` | tiles paralelos CPU | `render-cpu` | F4, no ahora |
| `image` | texturas/export PNG | `render-cpu/gpu` | F2, no ahora |
| `gltf` | cargar `.glb` | `render-gpu` | F5, no ahora |
| `wgpu 29` | backend GPU | `render-gpu` (feature) | F5, excluida de CPU-only |
| `winit` | ventana | `render-present` | F4, no ahora |
| `softbuffer` | presentar CPU | `render-present` | F4, no ahora |
| `tracing` | spans por etapa | todos | posterior, no ahora |

`cargo tree -p render-core --depth 1` (F1 cierre): solo
`engine + glam v0.32.1`. `bevy/wgpu/winit` no resuelven contra el paquete.
