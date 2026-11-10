# Astral Echo

## Ejecutar el juego en Windows

### Instalar Rust

Si `cargo` no existe, instala Rust desde <https://www.rust-lang.org/tools/install>. Después de instalarlo, reinicia PowerShell y comprueba:

```powershell
rustc --version
cargo --version
```

### Ejecutar la ventana visual

Desde la raíz del repositorio:

```powershell
cargo run --manifest-path .\crates\game\Cargo.toml --example gameplay_visual -- --window --backend cpu --graphics-validate
```

Para dejar que el programa elija entre GPU y CPU:

```powershell
cargo run --manifest-path .\crates\game\Cargo.toml --example gameplay_visual -- --window --backend auto --graphics-validate
```

Usa `cpu` si el backend GPU falla o funciona peor en tu equipo.

### Otros modos

Regresiones headless:

```powershell
cargo run -p game
```

Consola interactiva:

```powershell
cargo run -p game -- --runtime
```

En la consola interactiva aparecerá `astral>`. Allí puedes usar comandos como `status`, `help` y `quit`; los comandos de PowerShell deben ejecutarse fuera de esa consola.

### Ejecutar sin ventana

```powershell
cargo run --manifest-path .\crates\game\Cargo.toml --example gameplay_visual -- --backend cpu --frames 1 --output target/visual-cpu.png
```

`--output` solo se usa sin `--window`.

## Controles visuales

El panel `F1` muestra los controles del modo actual. `F2` oculta o muestra el HUD y `F3` abre o cierra el inventario. Si el HUD está visible, las acciones urgentes aparecen en pantalla cuando corresponden.

### Inicio y exploración interior

- `Enter`: comenzar la partida.
- `W/A/S/D`: mover al personaje dentro de ECHO-7.
- Flechas: mirar con el personaje.
- `I`: interactuar con el elemento contextual cercano.
- `M`: iniciar la misión disponible.
- `R`: reparar el sistema eléctrico cuando el panel y la condición de gameplay lo permiten.
- `Y`: restaurar el soporte vital cuando el panel y la condición de gameplay lo permiten.
- `V`: equipar el arma de energía.
- `B`: equipar el arma de plasma si ya fue encontrada.
- `C`: equipar el arma cinética.
- `E`: recoger un coleccionable activo.

### Exterior, observación y navegación

- `Q`: escanear señales.
- `P`: iniciar o invertir un viaje/autopiloto válido.
- `O`: iniciar o invertir la observación planetaria.
- `[` / `]`: cambiar el planeta observado mientras la observación está activa.
- `L`: solicitar aterrizaje cuando el planeta está dentro de la distancia válida.
- `F`: aterrizar si hay un planeta válido; en otro contexto ataca al enemigo seleccionado.
- `T`: reintentar el contenido cooperativo cuando la misión lo permite.
- `Escape`: cancelar un viaje de navegación libre en curso desde el pilotaje.

### Pilotaje de ECHO-7

- `W/S`: avanzar o retroceder.
- `A/D`: girar horizontalmente.
- Flechas izquierda/derecha: girar horizontalmente.
- Flechas arriba/abajo: inclinar la nave.
- `Espacio` / `Ctrl`: subir o bajar.
- `G`: activar o desactivar el escudo.
- `F`: disparar mientras se pilota.
- `Shift`: mantener pulsado para activar el boost.
- `X`: frenar.
- `N`: abrir o cerrar el selector de destino libre.
- En el selector: flechas izquierda/derecha cambian de planeta y `Enter` confirma.
- `U`: usar una unidad de combustible del inventario.
- `B` / `V` / `C`: equipar plasma, energía o armamento cinético.
- `P`: activar o invertir el autopiloto configurado.
- `I`: abandonar el pilotaje y devolver el control al personaje.
- `PageUp` / `PageDown` o `=` / `-`: acercar o alejar la cámara de pilotaje.
- La cámara sigue automáticamente a ECHO-7 durante el pilotaje.

### Exploración planetaria

- `W/A/S/D`: mover al personaje.
- Flechas: mirar.
- `I`: iniciar el despegue al acercarse al modelo de ECHO-7.

### Entrada contextual

Cuando el HUD muestra una acción contextual, puede activarse con la tecla indicada o haciendo clic izquierdo sobre el botón de acción. El clic también permite abrir el panel `F1` y los botones de controles que aparezcan en el HUD.

## Renderizado y rendimiento

- `G`: en la ventana de juego activa o desactiva el escudo durante el pilotaje. En hosts de hot-swap de pruebas con los controles de rasterizado habilitados también puede solicitar el cambio entre CPU y GPU.
- `Space`: avanzar manualmente 100 ms de tiempo solo en herramientas de prueba; durante el pilotaje significa subir.
- Teclas `1`, `2` y `3`: cambiar entre los presets de resolución bajo, medio y alto cuando el host de renderizado permite cambiar presets.
- Teclas `S`, `P` y `A`: seleccionar rasterizado secuencial, paralelo o automático únicamente cuando los controles de rasterizado están habilitados.

El juego usa resolución baja por defecto, selección automática del rasterizador y LOD para reducir el coste de modelos lejanos. Si el equipo tiene problemas de rendimiento, usa `--backend cpu`, mantén el preset bajo y evita capturas gráficas durante el juego.

## Problemas frecuentes

- `cargo no se reconoce`: Rust no está instalado o debes reiniciar la terminal.
- `could not find Cargo.toml`: no estás en la raíz del repositorio; usa `cd` o `Set-Location`.
- `comando desconocido ...`: estás escribiendo en `astral>`; escribe `quit` y ejecuta el comando en PowerShell.
- El backend GPU falla: ejecuta el juego con `--backend cpu`.
