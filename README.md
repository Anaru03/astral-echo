# astral-echo

Durante la aproximación planetaria, `F` confirma el aterrizaje cuando la distancia y la velocidad son válidas; fuera de esa ventana conserva la acción de ataque. La gravedad y la colisión se procesan en gameplay usando la misma posición orbital que el puente visual. La atmósfera se representa mediante vibración determinista de cámara y un efecto espacial; el proyecto no dispone de audio ni de un HUD textual dedicado para alarmas.

## Ejecución visual

Ventana interactiva con el renderer propio y selección explícita de backend:

```text
cargo run -p game --example gameplay_visual -- --window --backend cpu --graphics-validate
cargo run -p game --example gameplay_visual -- --window --backend gpu --graphics-validate
cargo run -p game --example gameplay_visual -- --window --backend auto --graphics-validate
```

La ejecución sin ventana genera una imagen verificable:

```text
cargo run -p game --example gameplay_visual -- --backend cpu --frames 1 --output target/visual-cpu.png
cargo run -p game --example gameplay_visual -- --backend auto --frames 1 --output target/visual-auto.png
```

Controles de la experiencia visual: `W/A/S/D` mueve al jugador, `Space` avanza el tiempo, `P` inicia o retira el viaje de la nave, `O` inicia o invierte la observación del planeta, las flechas desplazan la cámara alrededor de su superficie, `Q` escanea, `E` recoge, `F` aterriza cuando corresponde y ataca fuera de esa ventana, y `G` cambia entre CPU y GPU cuando el host de hot-swap está activo.

La órbita y rotación del planeta dependen del tiempo transcurrido. La cámara puede acercarse, observar y retirarse sin cargar otra escena. `--output` es para modo offscreen y no debe combinarse con `--window`.