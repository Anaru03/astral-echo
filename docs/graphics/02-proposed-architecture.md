# 02 — Arquitectura CPU/GPU

```text
GameplayWorld / Networking (futuro)
          |
  RenderSnapshot (POD lógico, sin Bevy/wgpu)
          |
     render-core (math, camera, mesh, material, scene, snapshot)
          |
   trait Renderer (F2+)
          |
     +----+----+
     |         |
 render-cpu   render-gpu (feature gpu, opcional)
     |         |
 CPU Buffer   wgpu
     +----+----+
          |
 render-present (winit+softbuffer / wgpu surface)
```

- Dirección: `game → present → {cpu|gpu} → render-core → engine`.
  `gameplay` nunca depende de render; `render-core` nunca depende de
  `bevy/wgpu/winit`. Sin ciclos.
- F1 implementa solo `render-core` + `game::render_bridge`.
- `FrameGraph` futuro como `Vec<Pass>` simple, sin grafo genérico.
