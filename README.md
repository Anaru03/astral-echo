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

### Multijugador por LAN (experimental)

En el equipo que hará de servidor, inicia la sesión escuchando en todas las interfaces:

```powershell
cargo run -p game -- --multiplayer 0.0.0.0:4000
```

En cada equipo cliente, usa la dirección IPv4 LAN del servidor:

```powershell
cargo run -p game -- --visual-client 192.168.1.20:4000 Ana
```

Reemplaza `192.168.1.20` por la dirección que muestre `ipconfig` en el equipo servidor. Permite el puerto TCP `4000` en el firewall de la red privada. El servidor admite hasta tres jugadores. Todos deben usar la misma versión del protocolo y abrir su propia ventana `--visual-client`.

El servidor concede el control de la nave a un solo jugador a la vez. Los demás pueden entrar al modo piloto para observar la cámara, pero no controlar la nave. La posición y orientación de la nave se transmiten a todos; los avatares de los pasajeros se representan junto a ella también en la vista exterior. El servidor separa las posiciones iniciales y corrige solapamientos entre jugadores.

La sincronización de combate, inventario por jugador y el resto de acciones cooperativas aún no está completa. El movimiento de personajes y la nave sí se replican entre las ventanas conectadas.

### Ejecutar sin ventana

```powershell
cargo run --manifest-path .\crates\game\Cargo.toml --example gameplay_visual -- --backend cpu --frames 1 --output target/visual-cpu.png
```

`--output` solo se usa sin `--window`.

## Controles visuales

- `F4`: pausar o reanudar la música.
- `F5`: activar o desactivar el mute.
- `F6`: pasar a la siguiente canción.

Para agregar música, coloca archivos `.mp3` en `assets/audio/music`. El juego los ordena por nombre al iniciar y los reproduce en secuencia, pasando automáticamente a la siguiente canción al terminar cada una. `F6` vuelve a leer la carpeta para detectar canciones agregadas mientras el juego está abierto.

El panel `F1` muestra los controles del modo actual. `F2` oculta o muestra el HUD y `F3` abre o cierra el inventario. Si el HUD está visible, las acciones urgentes aparecen en pantalla cuando corresponden.

### Recursos iniciales y balance

La nave comienza con el tanque de combustible lleno (100) y 200 unidades de reserva en el inventario, equivalente a tres tanques completos en total. Los saltos visuales cuestan entre 10 y 22 unidades según el destino; los restos recuperables dan 12 unidades de combustible. El inventario deja espacio para recoger recursos durante la exploración.

El jugador comienza con 5 unidades de oxígeno, suficientes para restaurar el soporte vital actual. El oxígeno no se consume con el paso del tiempo, así que acumular 15 unidades al inicio no tendría un uso adicional en el sistema actual. También hay 3 materiales de reparación, 8 municiones cinéticas y 4 de energía.

### Inicio y exploración interior

- `Enter`: comenzar la partida.
- `W/A/S/D`: mover al personaje dentro de ECHO-7.
- Flechas: mirar con el personaje.
- `I`: interactuar con el elemento contextual cercano.
- `M`: iniciar la misión disponible.
- `R`: reparar el sistema eléctrico cuando el panel y la condición de gameplay lo permiten.
- `Y`: restaurar el soporte vital cuando el panel y la condición de gameplay lo permiten.
- `Tab`: cambiar al siguiente arma disponible (cinética, energía y plasma cuando se haya encontrado).
- `E`: recoger un coleccionable activo.

### Exterior, observación y navegación

- `Q`: escanear señales.
- `P`: iniciar el viaje/autopiloto válido o reanudarlo si se pausó por un encuentro.
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
- En el selector: flechas izquierda/derecha cambian de planeta; `P` elige el planeta resaltado e inicia el autopiloto en un solo paso. No hace falta pulsar `Enter`.
- Si el planeta se movió mientras elegías, el juego recalcula la ruta al iniciar y muestra el resultado en el HUD.
- `U`: rellenar el tanque con la reserva disponible en una sola pulsación.
- `Tab`: cambiar al siguiente arma disponible; el plasma entra en el ciclo después de recuperarlo.
- `P`: iniciar o reanudar el autopiloto; si se pausa por una amenaza, vuelve a pulsarlo para continuar.
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

## Música del juego

Coloca aquí las canciones en formato MP3 (`.mp3`). El juego las reproduce en orden alfabético, continúa con la siguiente cuando termina una y permite saltar con `F6`.

## Efectos de sonido adicionales

| Archivo | Uso sugerido |
|---|---|
| `destination_select.wav` | Confirmar un destino |
| `autopilot_engage.wav` | Iniciar el piloto automático |
| `autopilot_arrival.wav` | Completar una ruta automática |
| `ship_boost.wav` | Activar el impulso |
| `ship_brake.wav` | Frenar la nave |
| `shield_on.wav` | Activar el escudo |
| `shield_off.wav` | Desactivar el escudo |
| `shield_hit.wav` | Impacto absorbido por el escudo |
| `warning_alert.wav` | Aviso urgente |
| `takeoff.wav` | Despegue |
| `turn.wav` | Giro breve de la nave |