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

En PowerShell, reemplaza la ruta si el repositorio está en otra carpeta:

```powershell
Set-Location "C:\Users\Tiffa\Desktop\UVG\GRAFICAS\PROYECTO3\astral-echo"
```

En CMD:

```bat
cd /d C:\Users\Tiffa\Desktop\UVG\GRAFICAS\PROYECTO3\astral-echo
```

Comprueba que estás en la raíz correcta:

```powershell
Get-ChildItem Cargo.toml
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

- `W/A/S/D`: mover al jugador.
- `Space`: avanzar el tiempo.
- `P`: iniciar o retirar el viaje de la nave.
- `O`: iniciar o invertir la observación del planeta.
- Flechas: mover la cámara alrededor de la superficie.
- `Q`: escanear.
- `E`: recoger.
- `F`: aterrizar cuando corresponda o atacar fuera de esa ventana.
- `G`: cambiar entre CPU y GPU cuando el host de hot-swap esté activo.

## Problemas frecuentes

- `cargo no se reconoce`: Rust no está instalado o la terminal debe reiniciarse.
- `could not find Cargo.toml`: no estás en la raíz del repositorio; ejecuta `Set-Location` o `cd /d`.
- `comando desconocido ...`: estás escribiendo en `astral>`; escribe `quit` y ejecuta el comando en PowerShell o CMD.
- El backend GPU falla: vuelve a ejecutar con `--backend cpu`.