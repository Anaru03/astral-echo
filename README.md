# Astral Echo

## Ejecutar el juego en Windows

### 1. Instalar Rust

Si `cargo` no existe, instala Rust desde:

```text
https://www.rust-lang.org/tools/install
```

Cierra y vuelve a abrir PowerShell después de instalarlo. Comprueba la instalación:

```powershell
rustc --version
cargo --version
```

### 2. Abrir una terminal del sistema

Abre PowerShell o CMD. Debes ver un prompt parecido a `PS C:\...>` o `C:\...>`. No debes ver `astral>`: ese prompt pertenece a la consola interna del juego.

### 3. Entrar al repositorio

```powershell
cd astral-echo
```

### 4. Abrir la ventana del juego

Este es el comando recomendado y probado:

```powershell
cargo run --manifest-path .\crates\game\Cargo.toml --example gameplay_visual -- --window --backend cpu --graphics-validate
```

La primera ejecución puede tardar mientras Cargo compila. Debe abrir una ventana titulada `Astral Echo - CPU/GPU Hot-Swap` y mostrar frames del backend CPU.

También puedes solicitar selección automática del backend:

```powershell
cargo run --manifest-path .\crates\game\Cargo.toml --example gameplay_visual -- --window --backend auto --graphics-validate
```

Usa `cpu` si `auto` o `gpu` no funcionan en tu equipo.

## Modos adicionales

Regresiones headless:

```powershell
cargo run -p game
```

Consola interactiva de gameplay:

```powershell
cargo run -p game -- --runtime
```

En la consola interactiva aparecerá `astral>`. Allí solo se aceptan comandos del juego, por ejemplo `status`, `help` o `quit`. No escribas comandos PowerShell como `cargo`, `Set-Location` o `cd` dentro de `astral>`.

## Ejecutar sin ventana

Para generar una imagen:

```powershell
cargo run --manifest-path .\crates\game\Cargo.toml --example gameplay_visual -- --backend cpu --frames 1 --output target/visual-cpu.png
```

La opción `--output` es para modo sin ventana y no debe combinarse con `--window`.

## Controles visuales

### Inicio y exploración interior

- `Enter`: comenzar la partida desde la pantalla inicial.
- `W/A/S/D`: mover al personaje dentro de ECHO-7 o controlar el desplazamiento de la nave durante el pilotaje.
- Flechas: orientar la cámara del personaje dentro de ECHO-7.
- `I`: interactuar con el elemento contextual seleccionado dentro del alcance. En la consola de pilotaje, entra o sale del modo de pilotaje.
- `M`: iniciar la misión disponible. En una partida nueva inicia Mission1; las misiones sucesoras requieren que estén disponibles.
- `R`: reparar el sistema eléctrico cuando el panel y la condición de gameplay lo permiten.
- `Y`: reparar el soporte vital cuando el panel y la condición de gameplay lo permiten.

### Exterior, observación y misiones

- `Q`: escanear señales cuando la acción está disponible.
- `P`: iniciar o invertir el viaje/autopiloto cuando el jugador está en exterior o pilotando ECHO-7 y existe una ruta válida.
- `O`: iniciar o invertir la observación del planeta.
- Flechas: orbitar la cámara durante la observación planetaria; durante el pilotaje ajustan el rumbo de ECHO-7.
- `F`: aterrizar cuando la ventana de aterrizaje es válida; en otro contexto intenta atacar al enemigo seleccionado.
- `E`: recoger un coleccionable activo cuando está disponible.
- `T`: reintentar la preparación del contenido cooperativo de Mission5 cuando la misión está activa y la recuperación corresponde.
- `V`: equipar el arma de energía.
- `C`: equipar el arma cinética.

### Pilotaje de ECHO-7

- `W/A/S/D`: acelerar la nave en la dirección correspondiente mientras el modo de pilotaje está activo.
- Flechas: cambiar el rumbo de ECHO-7.
- `P`: activar o invertir el autopiloto configurado.
- `I`: abandonar el pilotaje y devolver el control al personaje.

### Renderizado

- `G`: cambiar entre CPU y GPU cuando el host de hot-swap de la ventana visual está activo.
- `Space`: avanzar manualmente 100 ms de tiempo de gameplay. El modo visual también avanza el tiempo automáticamente mientras la partida está activa.

## Problemas frecuentes

- `cargo no se reconoce`: Rust no está instalado o la terminal debe reiniciarse.
- `could not find Cargo.toml`: no estás en la raíz del repositorio; ejecuta `Set-Location` o `cd /d`.
- `comando desconocido ...`: estás escribiendo en `astral>`; escribe `quit` y ejecuta el comando en PowerShell o CMD.
- El backend GPU falla: vuelve a ejecutar con `--backend cpu`.