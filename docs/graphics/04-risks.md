# 04 — Riesgos

| # | Riesgo | Impacto | Prob. | Mitigación / estado F1 |
|---|---|---|---|---|
| 1 | Acoplamiento Bevy↔gameplay | Alto | Media | `visual.rs` intacto; F1 solo lee vía bridge |
| 2 | Duplicación math | Medio | Alta | `glam` directo + única conversión `from_engine/to_engine` |
| 3 | Raster CPU lento | Alto | Alta | tiles + rayon + early-z en F4; meta LOW 30fps 640×360 |
| 4 | Memoria excesiva | Medio | Media | pools reutilizados; sin MSAA en CPU |
| 5 | Transferencias CPU→GPU | Medio | Media | `MeshCache` estático + uniforms dinámicos (F5) |
| 6 | Divergencia visual CPU/GPU | Medio | Alta | goldens con tolerancia (F2+) |
| 7 | Hard antiguo sin Vulkan | Alto | Media | perfil LOW = CPU `softbuffer`; auto-fallback |
| 8 | Assets `models/*.glb` ausentes | Medio | Alta | placeholders procedurales (`quad/cube`) |
| 9 | FrameGraph sobrediseñado | Medio | Media | `Vec<Pass>` simple, sin grafo genérico |
| 10 | Multijugador futuro | Alto | Baja | snapshot inmutable + `tick`; interp en F9 |
