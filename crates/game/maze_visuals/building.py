from building_obstacles import BuildingObstacles
import tkinter as tk
import random
from pathlib import Path

# ---------------------------------------------------------------
#  ASTRAL ECHO | Laberinto industrial cyberpunk (vista cenital 2D)
#  Contenedores, maquinaria, tuberias y calles con luces neon.
# ---------------------------------------------------------------

MAP = Path("/tmp/astral_echo_mazes/building.txt")

CELL = 34
SIZE = 21
WIDTH = CELL * SIZE
HEIGHT = CELL * SIZE
SEED = 9002

# Paleta (tomada de la imagen: verde azulado + naranja oxidado)
BG = "#060C12"
FLOOR = "#12262C"
FLOOR_PLATE = "#173038"
FLOOR_EDGE = "#0C1B20"
GRIME = "#0A161B"
RIVET = "#2C5057"
CYAN = "#19E6E0"
ORANGE = "#FF8138"
TEAL = "#1F6F73"
TEAL_DARK = "#0F3A40"
TEAL_LIGHT = "#3FA6A4"
METAL = "#24424A"
METAL_LIGHT = "#4F7F88"
OUTLINE = "#050B0F"
YELLOW = "#E3B52A"

# Azoteas de edificios: (techo, techo_claro, borde, acento)
ROOFS = [
    ("#2A474F", "#4E7A84", "#0D1E24", CYAN),      # concreto verde azulado
    ("#26333D", "#4A5E6B", "#0A141A", ORANGE),    # pizarra
    ("#3A3532", "#6B5A52", "#140F0E", ORANGE),    # oxido oscuro
    ("#1F4A52", "#3FA6A4", "#071A1F", CYAN),      # cobertura teal
]

CONTAINER_COLORS = [
    ("#C8741E", "#E39A47", "#7E4310"),   # naranja
    ("#A8452B", "#CB6C4D", "#6A2415"),   # oxido
    ("#B8902A", "#DDB852", "#745A14"),   # mostaza
    ("#2C7478", "#4DA5A6", "#16474A"),   # verde azulado
]


# ---------------------------------------------------------------
#  Utilidades de dibujo
# ---------------------------------------------------------------

def rect(canvas, x1, y1, x2, y2, fill, outline="", width=1, stipple=""):
    options = dict(fill=fill, outline=outline, width=width)
    if stipple:
        options["stipple"] = stipple
    canvas.create_rectangle(x1, y1, x2, y2, **options)


def circle(canvas, cx, cy, r, fill, outline="", width=1, stipple=""):
    options = dict(fill=fill, outline=outline, width=width)
    if stipple:
        options["stipple"] = stipple
    canvas.create_oval(cx - r, cy - r, cx + r, cy + r, **options)


def is_wall(lines, x, y):
    if 0 <= y < len(lines) and 0 <= x < len(lines[y]):
        return lines[y][x] == "#"
    return True


# ---------------------------------------------------------------
#  Generacion de mapa (solo si no existe el archivo)
# ---------------------------------------------------------------

def generate_map(seed=SEED):
    """Laberinto 21x21 con entrada E arriba a la izquierda y salida S abajo a la derecha."""
    r = random.Random(seed)
    grid = [["#"] * SIZE for _ in range(SIZE)]

    def carve(x, y):
        grid[y][x] = " "
        dirs = [(2, 0), (-2, 0), (0, 2), (0, -2)]
        r.shuffle(dirs)
        for dx, dy in dirs:
            nx, ny = x + dx, y + dy
            if 1 <= nx < SIZE - 1 and 1 <= ny < SIZE - 1 and grid[ny][nx] == "#":
                grid[y + dy // 2][x + dx // 2] = " "
                carve(nx, ny)

    carve(1, 1)

    # Abrir algunos muros extra para crear rutas alternas
    for _ in range(18):
        x = r.randrange(2, SIZE - 2)
        y = r.randrange(2, SIZE - 2)
        if grid[y][x] == "#" and (
            (grid[y][x - 1] == " " and grid[y][x + 1] == " ")
            or (grid[y - 1][x] == " " and grid[y + 1][x] == " ")
        ):
            grid[y][x] = " "

    grid[1][1] = "E"
    grid[SIZE - 2][SIZE - 2] = "S"
    return ["".join(row) for row in grid]


def load_lines():
    if MAP.exists():
        return MAP.read_text().splitlines()
    lines = generate_map()
    MAP.parent.mkdir(parents=True, exist_ok=True)
    MAP.write_text("\n".join(lines) + "\n")
    return lines


# ---------------------------------------------------------------
#  Suelo
# ---------------------------------------------------------------

def draw_floor(canvas, x, y, rng):
    px, py = x * CELL, y * CELL

    rect(canvas, px, py, px + CELL, py + CELL, FLOOR)
    rect(canvas, px + 2, py + 2, px + CELL - 2, py + CELL - 2,
         FLOOR_PLATE, FLOOR_EDGE)

    # Remaches en las esquinas
    for rx, ry in ((6, 6), (CELL - 6, 6), (6, CELL - 6), (CELL - 6, CELL - 6)):
        circle(canvas, px + rx, py + ry, 1.5, RIVET)

    # Manchas de suciedad
    if rng.random() < 0.35:
        gx = px + rng.randint(6, CELL - 14)
        gy = py + rng.randint(6, CELL - 14)
        rect(canvas, gx, gy, gx + rng.randint(6, 12), gy + rng.randint(4, 9),
             GRIME, stipple="gray50")

    # Grietas
    if rng.random() < 0.22:
        canvas.create_line(
            px + 7, py + 12,
            px + 16, py + 17,
            px + 22, py + 14,
            px + 28, py + 20,
            fill=GRIME, width=2,
        )

    # Cables en el suelo
    if rng.random() < 0.10:
        y0 = py + rng.randint(10, CELL - 10)
        canvas.create_line(px + 2, y0, px + CELL // 2, y0 + 4, px + CELL - 2, y0,
                           fill="#0B1417", width=3, smooth=True)

    # Franja de luz de calle
    roll = rng.random()
    if roll < 0.08:
        canvas.create_line(px + 8, py + CELL - 6, px + CELL - 8, py + CELL - 6,
                           fill=CYAN, width=2)
    elif roll < 0.13:
        canvas.create_line(px + 6, py + 6, px + 6, py + CELL - 6,
                           fill=ORANGE, width=2)

    # Rejilla de drenaje
    if rng.random() < 0.06:
        rect(canvas, px + 9, py + 9, px + CELL - 9, py + CELL - 9, "#081317", METAL)
        for i in range(12, CELL - 9, 4):
            canvas.create_line(px + 9, py + i, px + CELL - 9, py + i,
                               fill="#1B3A42")


# ---------------------------------------------------------------
#  Clasificacion de muros: contenedores (2-3 celdas) y maquinaria
# ---------------------------------------------------------------

def plan_walls(lines, rng):
    """Devuelve (segmentos, singles).
    segmentos: lista de (x, y, largo, orientacion, tipo, estilo)
        tipo = "container" | "building"
    singles: celdas de muro sueltas -> maquinaria / tanques
    """
    used = set()
    segments = []

    def split_run(length):
        parts = []
        while length > 0:
            if length <= 5:
                parts.append(length)
                break
            n = rng.choice((2, 3, 4, 5))
            if length - n == 1:
                n = n + 1 if n < 5 else n - 1
            parts.append(n)
            length -= n
        return parts

    def make_segment(x, y, n, orient):
        if n <= 3 and rng.random() < 0.40:
            return (x, y, n, orient, "container", rng.randrange(len(CONTAINER_COLORS)))
        return (x, y, n, orient, "building", rng.randrange(len(ROOFS)))

    # Tramos horizontales
    for y in range(SIZE):
        x = 0
        while x < SIZE:
            if lines[y][x] == "#" and (x, y) not in used:
                start = x
                while x < SIZE and lines[y][x] == "#" and (x, y) not in used:
                    x += 1
                run = x - start
                if run >= 2:
                    pos = start
                    for part in split_run(run):
                        if part >= 2 and rng.random() < 0.85:
                            segments.append(make_segment(pos, y, part, "h"))
                            for i in range(part):
                                used.add((pos + i, y))
                        pos += part
            else:
                x += 1

    # Tramos verticales con las celdas que sobran
    for x in range(SIZE):
        y = 0
        while y < SIZE:
            if lines[y][x] == "#" and (x, y) not in used:
                start = y
                while y < SIZE and lines[y][x] == "#" and (x, y) not in used:
                    y += 1
                run = y - start
                if run >= 2:
                    pos = start
                    for part in split_run(run):
                        if part >= 2 and rng.random() < 0.8:
                            segments.append(make_segment(x, pos, part, "v"))
                            for i in range(part):
                                used.add((x, pos + i))
                        pos += part
            else:
                y += 1

    singles = [(x, y) for y in range(SIZE) for x in range(SIZE)
               if lines[y][x] == "#" and (x, y) not in used]
    return segments, singles, used


# ---------------------------------------------------------------
#  Muros
# ---------------------------------------------------------------

def draw_shadow(canvas, x1, y1, x2, y2):
    rect(canvas, x1 + 5, y1 + 7, x2 + 5, y2 + 7, "#000000", stipple="gray50")


def draw_container(canvas, x, y, length, orient, color_idx):
    base, light, dark = CONTAINER_COLORS[color_idx]
    x1 = x * CELL + 2
    y1 = y * CELL + 2
    if orient == "h":
        x2 = (x + length) * CELL - 2
        y2 = (y + 1) * CELL - 2
    else:
        x2 = (x + 1) * CELL - 2
        y2 = (y + length) * CELL - 2

    draw_shadow(canvas, x1, y1, x2, y2)
    rect(canvas, x1, y1, x2, y2, dark, OUTLINE, 2)
    rect(canvas, x1 + 3, y1 + 3, x2 - 3, y2 - 3, base)

    # Costillas metalicas (perpendiculares al lado largo)
    if orient == "h":
        for cx in range(int(x1) + 8, int(x2) - 4, 6):
            canvas.create_line(cx, y1 + 4, cx, y2 - 4, fill=dark, width=2)
            canvas.create_line(cx + 2, y1 + 4, cx + 2, y2 - 4, fill=light)
        # Cierre de puertas en un extremo
        rect(canvas, x2 - 7, y1 + 4, x2 - 3, y2 - 4, METAL, OUTLINE)
        canvas.create_line(x1 + 4, y1 + 5, x2 - 8, y1 + 5, fill=light, width=2)
    else:
        for cy in range(int(y1) + 8, int(y2) - 4, 6):
            canvas.create_line(x1 + 4, cy, x2 - 4, cy, fill=dark, width=2)
            canvas.create_line(x1 + 4, cy + 2, x2 - 4, cy + 2, fill=light)
        rect(canvas, x1 + 4, y2 - 7, x2 - 4, y2 - 3, METAL, OUTLINE)
        canvas.create_line(x1 + 5, y1 + 4, x1 + 5, y2 - 8, fill=light, width=2)

    # Etiqueta desgastada
    rect(canvas, (x1 + x2) / 2 - 5, (y1 + y2) / 2 - 3,
         (x1 + x2) / 2 + 5, (y1 + y2) / 2 + 3, "#E7D6A5", stipple="gray50")


def draw_machine(canvas, x, y, lines, rng):
    px, py = x * CELL, y * CELL
    x1, y1, x2, y2 = px + 2, py + 2, px + CELL - 2, py + CELL - 2

    draw_shadow(canvas, x1, y1, x2, y2)
    rect(canvas, x1, y1, x2, y2, TEAL_DARK, OUTLINE, 2)
    rect(canvas, x1 + 3, y1 + 3, x2 - 3, y2 - 3, TEAL)
    rect(canvas, x1 + 3, y1 + 3, x2 - 3, y1 + 7, TEAL_LIGHT)

    kind = rng.randrange(4)

    if kind == 0:
        # Panel de control con luces
        rect(canvas, x1 + 7, y1 + 10, x2 - 7, y2 - 7, "#0B2A30", METAL_LIGHT)
        for i, col in enumerate((CYAN, ORANGE, "#36F0A4")):
            circle(canvas, x1 + 12 + i * 6, y1 + 16, 2, col)
        canvas.create_line(x1 + 10, y2 - 11, x2 - 10, y2 - 11, fill=CYAN, width=2)
    elif kind == 1:
        # Ventilador / turbina
        cx, cy = (x1 + x2) / 2, (y1 + y2) / 2 + 2
        circle(canvas, cx, cy, 9, "#0B2A30", METAL_LIGHT, 2)
        for dx, dy in ((0, -6), (5, 3), (-5, 3)):
            canvas.create_line(cx, cy, cx + dx, cy + dy, fill=CYAN, width=2)
        circle(canvas, cx, cy, 2, ORANGE)
    elif kind == 2:
        # Rejillas de ventilacion
        for i in range(11, CELL - 9, 4):
            canvas.create_line(x1 + 7, py + i, x2 - 7, py + i,
                               fill=TEAL_DARK, width=2)
        rect(canvas, x2 - 11, y1 + 8, x2 - 6, y1 + 12, ORANGE)
    else:
        # Franja de peligro amarilla y negra
        for i in range(0, CELL - 12, 6):
            rect(canvas, x1 + 5 + i, y2 - 11, x1 + 8 + i, y2 - 5,
                 YELLOW if (i // 6) % 2 == 0 else OUTLINE)
        circle(canvas, x1 + 11, y1 + 14, 4, "#0B2A30", METAL_LIGHT, 1)
        circle(canvas, x1 + 11, y1 + 14, 1.5, ORANGE)

    # Tuberias hacia muros vecinos tipo maquinaria
    cx, cy = px + CELL / 2, py + CELL / 2
    for dx, dy in ((1, 0), (0, 1)):
        nx, ny = x + dx, y + dy
        if is_wall(lines, nx, ny) and 0 <= nx < SIZE and 0 <= ny < SIZE:
            if rng.random() < 0.6:
                ex, ey = cx + dx * CELL, cy + dy * CELL
                canvas.create_line(cx, cy, ex, ey, fill=OUTLINE, width=8)
                canvas.create_line(cx, cy, ex, ey, fill=METAL, width=5)
                canvas.create_line(cx, cy - 1, ex, ey - 1, fill=METAL_LIGHT, width=1)


def segment_bounds(x, y, length, orient):
    x1, y1 = x * CELL + 2, y * CELL + 2
    if orient == "h":
        x2, y2 = (x + length) * CELL - 2, (y + 1) * CELL - 2
    else:
        x2, y2 = (x + 1) * CELL - 2, (y + length) * CELL - 2
    return x1, y1, x2, y2


def roof_feature(canvas, kind, cx, cy, accent, rng):
    """Equipo sobre la azotea, centrado en (cx, cy)."""
    if kind == "ac":
        rect(canvas, cx - 10, cy - 8, cx + 10, cy + 8, "#16343B", OUTLINE, 2)
        circle(canvas, cx - 4, cy, 5, "#0A1D22", METAL_LIGHT, 1)
        for dx, dy in ((-4, -3), (-1, 1), (-7, 1)):
            canvas.create_line(cx - 4, cy, cx - 4 + dx // 1, cy + dy, fill=accent)
        for i in range(3):
            canvas.create_line(cx + 3, cy - 5 + i * 5, cx + 8, cy - 5 + i * 5,
                               fill=METAL_LIGHT)
    elif kind == "skylight":
        rect(canvas, cx - 11, cy - 8, cx + 11, cy + 8, "#071A1F", METAL_LIGHT, 2)
        rect(canvas, cx - 9, cy - 6, cx + 9, cy + 6, accent, stipple="gray25")
        canvas.create_line(cx - 7, cy + 5, cx - 1, cy - 5, fill="white")
        canvas.create_line(cx, cy + 5, cx + 6, cy - 5, fill="white")
    elif kind == "vent":
        circle(canvas, cx, cy, 8, "#0A1D22", METAL_LIGHT, 2)
        canvas.create_line(cx - 6, cy, cx + 6, cy, fill=METAL_LIGHT, width=2)
        canvas.create_line(cx, cy - 6, cx, cy + 6, fill=METAL_LIGHT, width=2)
        circle(canvas, cx, cy, 2, accent)
    elif kind == "antenna":
        circle(canvas, cx, cy + 5, 4, METAL, OUTLINE, 1)
        canvas.create_line(cx, cy + 5, cx, cy - 9, fill=METAL_LIGHT, width=2)
        canvas.create_line(cx - 6, cy - 4, cx + 6, cy - 4, fill=METAL_LIGHT)
        circle(canvas, cx, cy - 10, 5, "#FF3B3B", stipple="gray25")
        circle(canvas, cx, cy - 10, 2, "#FF3B3B")
    elif kind == "tank":
        circle(canvas, cx + 2, cy + 3, 10, "#000000", stipple="gray50")
        circle(canvas, cx, cy, 10, METAL, OUTLINE, 2)
        circle(canvas, cx, cy, 6, "#2E5760", METAL_LIGHT, 1)
        circle(canvas, cx, cy, 2, accent)
    elif kind == "hatch":
        rect(canvas, cx - 8, cy - 8, cx + 8, cy + 8, "#2C4650", OUTLINE, 2)
        rect(canvas, cx - 5, cy - 5, cx + 5, cy + 5, "#0F2329", METAL_LIGHT)
        canvas.create_line(cx - 3, cy, cx + 3, cy, fill=YELLOW, width=2)
    elif kind == "solar":
        for i in range(2):
            for j in range(2):
                rx, ry = cx - 11 + i * 12, cy - 9 + j * 10
                rect(canvas, rx, ry, rx + 10, ry + 8, "#0B2C45", "#4FA3D1")
                canvas.create_line(rx + 5, ry, rx + 5, ry + 8, fill="#1E5F8C")
    elif kind == "pipes":
        for dy in (-5, 0, 5):
            canvas.create_line(cx - 13, cy + dy, cx + 13, cy + dy,
                               fill=OUTLINE, width=5)
            canvas.create_line(cx - 13, cy + dy, cx + 13, cy + dy,
                               fill=METAL, width=3)
            canvas.create_line(cx - 13, cy + dy - 1, cx + 13, cy + dy - 1,
                               fill=METAL_LIGHT)
        rect(canvas, cx - 3, cy - 8, cx + 3, cy + 8, "#16343B", OUTLINE)


ROOF_FEATURES = ["ac", "skylight", "vent", "antenna", "tank", "hatch", "solar", "pipes"]


def draw_building(canvas, x, y, length, orient, style, rng):
    roof, roof_light, trim, accent = ROOFS[style]
    x1, y1, x2, y2 = segment_bounds(x, y, length, orient)

    draw_shadow(canvas, x1, y1, x2, y2)

    # Estructura: borde oscuro, parapeto claro, azotea
    rect(canvas, x1, y1, x2, y2, trim, OUTLINE, 2)
    rect(canvas, x1 + 2, y1 + 2, x2 - 2, y2 - 2, roof_light)
    rect(canvas, x1 + 4, y1 + 4, x2 - 4, y2 - 4, roof)
    rect(canvas, x1 + 7, y1 + 7, x2 - 7, y2 - 7, "", trim, 1)

    # Manchas de desgaste en la azotea
    for _ in range(length):
        gx = rng.uniform(x1 + 6, x2 - 16)
        gy = rng.uniform(y1 + 6, y2 - 14)
        rect(canvas, gx, gy, gx + rng.randint(6, 14), gy + rng.randint(4, 8),
             "#000000", stipple="gray12")

    # Centros de cada celda del edificio
    cells = []
    for i in range(length):
        if orient == "h":
            cells.append(((x + i) * CELL + CELL / 2, y * CELL + CELL / 2))
        else:
            cells.append((x * CELL + CELL / 2, (y + i) * CELL + CELL / 2))

    # Juntas entre modulos
    for i in range(1, length):
        if orient == "h":
            jx = (x + i) * CELL
            canvas.create_line(jx, y1 + 4, jx, y2 - 4, fill=trim, width=2)
        else:
            jy = (y + i) * CELL
            canvas.create_line(x1 + 4, jy, x2 - 4, jy, fill=trim, width=2)

    # Equipos en la azotea (sin repetir el vecino)
    previous = None
    for (cx, cy) in cells:
        options = [f for f in ROOF_FEATURES if f != previous]
        feature = rng.choice(options)
        roof_feature(canvas, feature, cx, cy, accent, rng)
        previous = feature

    # Franja de neon en el borde largo + balizas en las esquinas
    if rng.random() < 0.6:
        if orient == "h":
            canvas.create_line(x1 + 6, y1 + 3, x2 - 6, y1 + 3,
                               fill=accent, width=5, stipple="gray25")
            canvas.create_line(x1 + 6, y1 + 3, x2 - 6, y1 + 3, fill=accent, width=1)
        else:
            canvas.create_line(x1 + 3, y1 + 6, x1 + 3, y2 - 6,
                               fill=accent, width=5, stipple="gray25")
            canvas.create_line(x1 + 3, y1 + 6, x1 + 3, y2 - 6, fill=accent, width=1)

    for bx, by in ((x1 + 5, y1 + 5), (x2 - 5, y2 - 5)):
        circle(canvas, bx, by, 2, "#FF5A3C")


def draw_special(canvas, x, y, kind, rng):
    """Tanques, radares y generadores para celdas de muro sueltas."""
    px, py = x * CELL, y * CELL
    x1, y1, x2, y2 = px + 2, py + 2, px + CELL - 2, py + CELL - 2
    cx, cy = px + CELL / 2, py + CELL / 2

    draw_shadow(canvas, x1, y1, x2, y2)
    rect(canvas, x1, y1, x2, y2, "#0C1A20", OUTLINE, 2)

    if kind == "silo":
        circle(canvas, cx, cy, 14, METAL, OUTLINE, 2)
        circle(canvas, cx, cy, 10, "#2E5760", METAL_LIGHT, 2)
        circle(canvas, cx, cy, 5, "#0F2329", METAL_LIGHT, 1)
        circle(canvas, cx, cy, 2, ORANGE)
        for ang in ((0, -13), (13, 0), (0, 13), (-13, 0)):
            canvas.create_line(cx, cy, cx + ang[0] * 0.75, cy + ang[1] * 0.75,
                               fill=METAL_LIGHT)
    elif kind == "radar":
        circle(canvas, cx, cy, 13, "#16343B", OUTLINE, 2)
        canvas.create_arc(cx - 11, cy - 11, cx + 11, cy + 11, start=30, extent=240,
                          style="chord", fill="#2B5560", outline=METAL_LIGHT)
        canvas.create_line(cx, cy, cx + 10, cy - 6, fill=CYAN, width=2)
        circle(canvas, cx + 10, cy - 6, 2, CYAN)
        circle(canvas, cx, cy, 3, ORANGE)
    else:  # generador
        rect(canvas, x1 + 4, y1 + 4, x2 - 4, y2 - 4, "#2E5760", METAL_LIGHT, 1)
        rect(canvas, x1 + 4, cy - 3, x2 - 4, cy + 3, ORANGE)
        for i in range(3):
            circle(canvas, x1 + 9 + i * 7, y1 + 10, 2, CYAN if i != 1 else "#36F0A4")
        for i in range(4):
            canvas.create_line(x1 + 8 + i * 5, y2 - 11, x1 + 8 + i * 5, y2 - 6,
                               fill=OUTLINE, width=2)


# ---------------------------------------------------------------
#  Iluminacion y efectos
# ---------------------------------------------------------------

def draw_glows(canvas, lines, rng):
    roads = [(x, y) for y, line in enumerate(lines)
             for x, c in enumerate(line) if c != "#"]
    rng.shuffle(roads)

    for i, (x, y) in enumerate(roads[:9]):
        color = ORANGE if i % 3 == 0 else CYAN
        cx, cy = x * CELL + CELL / 2, y * CELL + CELL / 2
        circle(canvas, cx, cy, 52, color, stipple="gray12")
        circle(canvas, cx, cy, 34, color, stipple="gray25")
        circle(canvas, cx, cy, 5, color)
        circle(canvas, cx, cy, 2, "white")


def draw_vignette(canvas):
    for i, stipple in enumerate(("gray50", "gray25", "gray12")):
        d = 26 * (i + 1)
        rect(canvas, 0, 0, WIDTH, d, "#000000", stipple=stipple)
        rect(canvas, 0, HEIGHT - d, WIDTH, HEIGHT, "#000000", stipple=stipple)
        rect(canvas, 0, 0, d, HEIGHT, "#000000", stipple=stipple)
        rect(canvas, WIDTH - d, 0, WIDTH, HEIGHT, "#000000", stipple=stipple)


def draw_markers(canvas, lines):
    for y, line in enumerate(lines):
        for x, char in enumerate(line):
            if char not in ("E", "S"):
                continue

            cx = x * CELL + CELL / 2
            cy = y * CELL + CELL / 2
            color = "#36F0A4" if char == "E" else "#FF526E"

            circle(canvas, cx, cy, 20, color, stipple="gray25")
            circle(canvas, cx, cy, 14, color, stipple="gray50")
            circle(canvas, cx, cy, 11, color, "white", 2)
            canvas.create_text(cx, cy, text=char, fill="#071018",
                               font=("DejaVu Sans", 12, "bold"))


# ---------------------------------------------------------------
#  Ciudad completa
# ---------------------------------------------------------------

def draw_city(canvas, lines):
    canvas.delete("all")
    rng = random.Random(SEED)

    rect(canvas, 0, 0, WIDTH, HEIGHT, BG)

    # 1) Suelo
    for y, line in enumerate(lines):
        for x, char in enumerate(line):
            if char != "#":
                draw_floor(canvas, x, y, rng)

    # 2) Muros: contenedores y maquinaria
    segments, singles, _ = plan_walls(lines, rng)
    for (x, y, length, orient, kind, style) in segments:
        if kind == "container":
            draw_container(canvas, x, y, length, orient, style)
        else:
            draw_building(canvas, x, y, length, orient, style, rng)
    for (x, y) in singles:
        roll = rng.random()
        if roll < 0.20:
            draw_special(canvas, x, y, "silo", rng)
        elif roll < 0.32:
            draw_special(canvas, x, y, "radar", rng)
        elif roll < 0.42:
            draw_special(canvas, x, y, "generator", rng)
        else:
            draw_machine(canvas, x, y, lines, rng)

    # 3) Luces, vineta y marcadores
    draw_glows(canvas, lines, rng)
    draw_vignette(canvas)
    draw_markers(canvas, lines)


# ---------------------------------------------------------------
#  Ventana
# ---------------------------------------------------------------



class MazeController:
    def __init__(self, canvas, lines, status):
        self.canvas = canvas
        self.lines = lines
        self.status = status

        self.start = self.find_symbol("E")
        self.goal = self.find_symbol("S")

        self.x, self.y = self.start
        self.steps = 0
        self.completed = False

        self.obstacles = BuildingObstacles(
            canvas, lines, CELL, status
        )

        self.draw_player()

    def find_symbol(self, symbol):
        for y, row in enumerate(self.lines):
            for x, value in enumerate(row):
                if value == symbol:
                    return (x, y)
        raise ValueError(f"No se encontro {symbol}")

    def draw_player(self):
        self.canvas.delete("maze_player")

        cx = self.x * CELL + CELL / 2
        cy = self.y * CELL + CELL / 2

        self.canvas.create_oval(
            cx - 12, cy - 12,
            cx + 12, cy + 12,
            fill="#071A22",
            outline="#19E6E0",
            width=3,
            tags="maze_player",
        )

        self.canvas.create_polygon(
            cx, cy - 9,
            cx - 8, cy + 7,
            cx, cy + 3,
            cx + 8, cy + 7,
            fill="#F0FAFF",
            outline="#19E6E0",
            tags="maze_player",
        )

        self.canvas.tag_raise("maze_player")

    def move(self, dx, dy):
        if self.completed:
            return

        nx = self.x + dx
        ny = self.y + dy

        if not (0 <= nx < SIZE and 0 <= ny < SIZE):
            return

        if self.lines[ny][nx] == "#":
            self.update_status("EDIFICIO BLOQUEANDO EL PASO")
            return

        allowed, message = self.obstacles.can_enter((nx, ny))

        if not allowed:
            self.update_status(message)
            return

        self.steps += 1

        position, messages = self.obstacles.enter((nx, ny))
        self.x, self.y = position

        self.draw_player()

        if (self.x, self.y) == self.goal:
            self.completed = True
            self.obstacles.running = False
            self.status.config(
                text=(
                    f"BUILDING COMPLETADO | "
                    f"{self.steps} PASOS | "
                    f"ENERGIA {self.obstacles.energy}"
                ),
                fg="#36F0A4",
            )
            return

        self.update_status(" | ".join(messages) if messages else "")

    def update_status(self, message=""):
        text = (
            f"ENERGIA: {self.obstacles.energy} | "
            f"PASOS: {self.steps} | "
            f"TARJETA: {'SI' if self.obstacles.has_key else 'NO'}"
        )

        if message:
            text += f" | {message}"

        self.status.config(
            text=text,
            fg="#B8C9D0",
        )

    def reset(self):
        self.x, self.y = self.start
        self.steps = 0
        self.completed = False

        if not self.obstacles.running:
            self.obstacles.running = True
            self.canvas.after(700, self.obstacles.update)

        self.obstacles.reset()
        self.draw_player()
        self.update_status("LABERINTO REINICIADO")

    def handle_key(self, event):
        key = event.keysym.lower()

        directions = {
            "w": (0, -1),
            "up": (0, -1),
            "s": (0, 1),
            "down": (0, 1),
            "a": (-1, 0),
            "left": (-1, 0),
            "d": (1, 0),
            "right": (1, 0),
        }

        if key == "r":
            self.reset()
        elif key in directions:
            self.move(*directions[key])



def main():
    lines = load_lines()

    if len(lines) != SIZE or any(len(row) != SIZE for row in lines):
        raise ValueError("El mapa debe ser de 21x21")

    window = tk.Tk()
    window.title("Astral Echo | Building")
    window.configure(bg=BG)
    window.resizable(False, False)

    # Dimensiones constantes, incluso con mensajes largos.
    WINDOW_WIDTH = WIDTH + 340
    WINDOW_HEIGHT = HEIGHT + 155
    window.geometry(f"{WINDOW_WIDTH}x{WINDOW_HEIGHT}")

    header = tk.Frame(window, bg=BG)
    header.pack(fill="x", pady=(8, 5))

    tk.Label(
        header,
        text="BUILDING | ZONA INDUSTRIAL",
        bg=BG,
        fg=ORANGE,
        font=("DejaVu Sans", 18, "bold"),
    ).pack()

    body = tk.Frame(window, bg=BG)
    body.pack(fill="both", expand=True, padx=10)

    game_area = tk.Frame(body, bg=BG)
    game_area.pack(side="left", anchor="n")

    canvas = tk.Canvas(
        game_area,
        width=WIDTH,
        height=HEIGHT,
        bg=BG,
        highlightthickness=1,
        highlightbackground=TEAL,
    )
    canvas.pack()

    draw_city(canvas, lines)

    sidebar_container = tk.Frame(
        body, bg="#101D27",
        width=300,
        height=HEIGHT,
    )
    sidebar_container.pack(
        side="left", fill="y", padx=(10, 0)
    )
    sidebar_container.pack_propagate(False)

    sidebar = tk.Frame(
        sidebar_container,
        bg="#101D27",
    )
    sidebar.pack(fill="both", expand=True, padx=12)

    tk.Label(
        sidebar,
        text="INSTRUCCIONES",
        bg="#101D27",
        fg=CYAN,
        font=("DejaVu Sans", 14, "bold"),
    ).pack(anchor="w", pady=(15, 12))


    def legend_icon(parent, kind):
        icon = tk.Canvas(
            parent,
            width=38,
            height=38,
            bg="#101D27",
            highlightthickness=0,
        )
        icon.pack(side="left", padx=(5, 9))

        x, y = 19, 19

        if kind in ("entrance", "exit"):
            color = "#32E7A1" if kind == "entrance" else "#FF536B"
            letter = "E" if kind == "entrance" else "S"

            icon.create_oval(
                x-11, y-11, x+11, y+11,
                fill=color, outline="white", width=2
            )
            icon.create_text(
                x, y, text=letter,
                fill="#071018",
                font=("DejaVu Sans", 11, "bold")
            )

        elif kind == "key":
            icon.create_rectangle(
                x-10, y-7, x+10, y+7,
                fill="#FFD447", outline="white", width=2
            )
            icon.create_text(
                x, y, text="K", fill="#151515",
                font=("DejaVu Sans", 9, "bold")
            )

        elif kind == "door":
            icon.create_rectangle(
                x-13, y-13, x+13, y+13,
                fill="#62202D", outline="#FF365D", width=3
            )
            icon.create_text(
                x, y, text="D", fill="white",
                font=("DejaVu Sans", 10, "bold")
            )

        elif kind == "laser":
            color = "#FF365D"
            icon.create_line(
                x-12, y-12, x+12, y+12,
                fill=color, width=4
            )
            icon.create_line(
                x+12, y-12, x-12, y+12,
                fill=color, width=3
            )

        elif kind == "steam":
            icon.create_oval(
                x-11, y-11, x+11, y+11,
                fill="#A9D8DF", outline="#19E6E0", width=2
            )
            icon.create_text(
                x, y, text="V", fill="#071018",
                font=("DejaVu Sans", 10, "bold")
            )

        elif kind == "drone":
            icon.create_oval(
                x-10, y-10, x+10, y+10,
                fill="#36102C", outline="#FF365D", width=3
            )
            icon.create_oval(
                x-3, y-3, x+3, y+3,
                fill="#FF365D"
            )

        elif kind == "checkpoint":
            icon.create_oval(
                x-9, y-9, x+9, y+9,
                fill="#287DCA", outline="white", width=2
            )

        return icon

    legend = [
        ("entrance", "ENTRADA", "Aqui comienza el recorrido."),
        ("exit", "SALIDA", "Llega aqui para completar Building."),
        ("key", "TARJETA K", "Recogela para abrir la puerta."),
        ("door", "PUERTA D", "No puedes cruzarla sin la tarjeta."),
        ("laser", "LASER", "Activo: bloquea el paso. Espera."),
        ("steam", "VAPOR V", "Activo al entrar: -20 de energia."),
        ("drone", "DRON", "Al entrar en su posicion: -30 de energia."),
        ("checkpoint", "CONTROL", "Guarda tu punto de reaparicion."),
    ]

    for kind, name, description in legend:
        row = tk.Frame(sidebar, bg="#101D27")
        row.pack(fill="x", pady=3)

        legend_icon(row, kind)

        information = tk.Frame(row, bg="#101D27")
        information.pack(side="left", fill="x", expand=True)

        tk.Label(
            information,
            text=name,
            bg="#101D27",
            fg="#FF8138",
            font=("DejaVu Sans", 9, "bold"),
        ).pack(anchor="w")

        tk.Label(
            information,
            text=description,
            bg="#101D27",
            fg="#B8C9D0",
            font=("DejaVu Sans", 9),
            wraplength=185,
            justify="left",
        ).pack(anchor="w")

    tk.Label(
        sidebar,
        text="WASD / FLECHAS: MOVER   |   R: REINICIAR",
        bg="#101D27",
        fg="#19E6E0",
        font=("DejaVu Sans", 8, "bold"),
        wraplength=260,
        justify="center",
    ).pack(pady=(12, 4))

    sidebar_visible = [True]

    def toggle_sidebar():
        sidebar_visible[0] = not sidebar_visible[0]

        if sidebar_visible[0]:
            sidebar_container.pack(
                side="left", fill="y", padx=(10, 0)
            )
            toggle.config(text="OCULTAR INSTRUCCIONES")
        else:
            sidebar_container.pack_forget()
            toggle.config(text="MOSTRAR INSTRUCCIONES")

    footer = tk.Frame(window, bg=BG)
    footer.pack(fill="x", padx=10, pady=(4, 8))

    toggle = tk.Button(
        footer,
        text="OCULTAR INSTRUCCIONES",
        command=toggle_sidebar,
        bg="#173038",
        fg="white",
        activebackground="#24515B",
    )
    toggle.pack(anchor="w")

    # Reservar un ancho fijo para los mensajes.
    status = tk.Label(
        footer,
        text="EXPLORANDO BUILDING | PASOS: 0",
        bg=BG,
        fg="#B8C9D0",
        font=("DejaVu Sans", 10, "bold"),
        anchor="w",
        justify="left",
        wraplength=WINDOW_WIDTH - 35,
        height=2,
    )
    status.pack(fill="x")

    controller = MazeController(canvas, lines, status)
    window.bind("<KeyPress>", controller.handle_key)

    window.mainloop()


if __name__ == "__main__":
    main()