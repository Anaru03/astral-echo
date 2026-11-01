# Astral Echo — Multijugador TCP

## Descripción

Astral Echo incluye un sistema multijugador basado en TCP, implementado en Rust, que permite conectar hasta tres jugadores simultáneamente.

El servidor administra las conexiones, valida los mensajes y movimientos, mantiene identificadores únicos y sincroniza las actualizaciones autorizadas por gameplay.

## Requisitos

- Rust y Cargo instalados.
- Código fuente de Astral Echo.
- Acceso a una terminal.
- Conectividad TCP entre el servidor y los clientes.

## Iniciar el servidor

Desde la raíz del proyecto:

```bash
cargo run -p game -- --multiplayer 0.0.0.0:5000
```

El servidor escuchará en el puerto TCP 5000, utilizando todas las interfaces de red disponibles.

Para pruebas únicamente en la misma computadora, se puede utilizar `127.0.0.1:5000`.

## Conectar jugadores

Cada jugador debe ejecutar el cliente desde su propia terminal o computadora.

```bash
cargo run -p networking --example multiplayer_client -- <IP_SERVIDOR>:5000 <NOMBRE>
```

Ejemplo para una prueba local:

```bash
cargo run -p networking --example multiplayer_client -- 127.0.0.1:5000 Ruth
```

Para conectarse desde otra computadora, se debe reemplazar `127.0.0.1` por una dirección IP accesible del servidor.

## Comandos disponibles

| Comando | Descripción |
|---|---|
| `help` | Muestra los comandos disponibles. |
| `players` | Consulta los jugadores remotos y sus últimas posiciones recibidas. |
| `move x y z` | Envía una dirección de movimiento válida. |
| `rotate yaw pitch` | Modifica la orientación local que se enviará con el siguiente movimiento. |
| `ping` | Comprueba que el servidor responde. |
| `quit` | Abandona la partida. |

Ejemplo:

```text
move 1 0 0
rotate 90 15
move 0 0 1
players
```

## Límite de jugadores

El servidor permite un máximo de tres jugadores conectados simultáneamente.

Si un cuarto cliente intenta ingresar cuando la partida está llena, su conexión es rechazada.

Cuando un jugador se desconecta, su espacio puede ser ocupado por otro cliente. El servidor mantiene identificadores de red únicos y gameplay puede reutilizar entidades inactivas.

## Sincronización de movimientos

Los clientes envían mensajes `PlayerMove` con una secuencia, posición, orientación y velocidad.

El servidor valida los datos, rechaza actualizaciones antiguas o inválidas y entrega los movimientos autorizados a los demás jugadores mediante `PlayerMoved`.

El cliente conserva el estado remoto mediante `RemotePlayers`, incluyendo la última transformación y secuencia recibida.

## Pruebas

Para ejecutar las pruebas de networking:

```bash
cargo test -p networking
```

Para ejecutar las pruebas de integración con gameplay:

```bash
cargo test -p game multiplayer::tests
```

Para validar todo el proyecto:

```bash
cargo test --workspace
cargo check --workspace
```

También se realizaron pruebas manuales con tres clientes independientes en una computadora Debian, comprobando conexiones, movimientos, notificaciones de entrada y salida, reutilización de espacios y rechazo de un cuarto cliente simultáneo.

## Pruebas entre computadoras

Las computadoras deben tener conectividad hacia el servidor en el puerto TCP 5000.

En una red local, puede utilizarse la dirección IP privada del equipo que ejecuta el servidor. Algunas redes institucionales pueden restringir la comunicación directa entre dispositivos.

Para computadoras ubicadas en redes diferentes, se requiere una conexión enrutable, por ejemplo mediante una VPN privada configurada por los participantes.

## Alcance

El cliente interactivo permite verificar la comunicación y sincronización por terminal. No constituye por sí mismo una interfaz gráfica multijugador.

La representación visual simultánea de jugadores remotos dentro del motor gráfico requiere integración adicional si forma parte de los requisitos de la entrega.