import tkinter as tk
import random
import math
from pathlib import Path

# ---------------------------------------------------------------
#  KHARVEX | MECANIC PLANET  -  red de tuberias y reactores
#
#  Los muros NO son edificios: son una red continua de tuberias
#  de energia (violeta, lima y ambar) que se conectan en nodos:
#  reactores, valvulas, bombas y manometros. El suelo es una
#  placa de circuitos con cintas transportadoras.
# ---------------------------------------------------------------

MAP = Path("/tmp/astral_echo_mazes/kharvex.txt")

CELL = 34
SIZE = 21
WIDTH = CELL * SIZE
HEIGHT = CELL * SIZE
SEED = 7331

# Paleta propia de Kharvex: indigo profundo + violeta / lima / ambar
BG = "#07060F"
FLOOR = "#12112A"
FLOOR_PLATE = "#1A1838"
FLOOR_SEAM = "#0A0918"
TRACE = "#3B2E7A"
VOID = "#0C0B1E"
MESH = "#16153A"
LEDGE = "#2E3358"
LEDGE_LIGHT = "#5A63A0"
STEEL = "#2B3050"
STEEL_LIGHT = "#4B5384"
STEEL_DARK = "#171A30"
CHANNEL = "#0A0A18"
OUTLINE = "#040309"

VIOLET = "#B04BFF"
LIME = "#9DFF3C"
AMBER = "#FFB020"
PINK = "#FF3DA5"
ZONE_COLORS = [VIOLET, LIME, AMBER, PINK]

# Colores de iconos (iguales a los de tu panel de instrucciones)
CYAN = "#19E6E0"
ORANGE = "#FF8138"
RED = "#FF4D6A"
GREEN = "#36F0A4"
YELLOW = "#E3B52A"

# Simbolos del mapa (edita las letras para que coincidan con tu juego)
#   E entrada | S salida | N nucleo | G compuerta | F campo electrico
#   H calor   | D dron   | C control
MARKERS = {
    "E": "entrada",
    "S": "salida",
    "N": "nucleo",
    "G": "compuerta",
    "F": "electrico",
    "H": "calor",
    "D": "dron",
    "C": "control",
}


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


def neon_line(canvas, pts, color, width=2):
    canvas.create_line(*pts, fill=color, width=width + 5, stipple="gray25")
    canvas.create_line(*pts, fill=color, width=width)


def is_wall(lines, x, y):
    if 0 <= y < len(lines) and 0 <= x < len(lines[y]):
        return lines[y][x] == "#"
    return True


def zone_color(x, y):
    return ZONE_COLORS[(x // 6 + 2 * (y // 6)) % len(ZONE_COLORS)]


def center(x, y):
    return x * CELL + CELL / 2, y * CELL + CELL / 2


# ---------------------------------------------------------------
#  Mapa (solo se genera si no existe el archivo)
# ---------------------------------------------------------------

def generate_map(seed=SEED):
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

    # Objetos de muestra para previsualizar los iconos
    free = [(x, y) for y in range(SIZE) for x in range(SIZE)
            if grid[y][x] == " " and (x + y) > 6]
    r.shuffle(free)
    for char in "NGFFHHDDCC":
        if free:
            x, y = free.pop()
            grid[y][x] = char

    return ["".join(row) for row in grid]


def load_lines():
    if MAP.exists():
        return MAP.read_text().splitlines()
    lines = generate_map()
    MAP.parent.mkdir(parents=True, exist_ok=True)
    MAP.write_text("\n".join(lines) + "\n")
    return lines


# ---------------------------------------------------------------
#  Suelo: placa de circuitos y cintas transportadoras
# ---------------------------------------------------------------

def draw_floor(canvas, x, y, lines, rng):
    px, py = x * CELL, y * CELL
    cx, cy = px + CELL / 2, py + CELL / 2

    rect(canvas, px, py, px + CELL, py + CELL, FLOOR_SEAM)
    shade = rng.choice((FLOOR, FLOOR_PLATE, FLOOR_PLATE))
    rect(canvas, px + 1, py + 1, px + CELL - 1, py + CELL - 1, shade)
    canvas.create_line(px + 2, py + 2, px + CELL - 2, py + 2, fill="#2A2766")

    # Textura de placa antideslizante (cruces pequenas)
    for i in range(3):
        for j in range(3):
            tx = px + 7 + i * 10
            ty = py + 7 + j * 10
            canvas.create_line(tx - 1, ty, tx + 1, ty, fill="#24214F")
            canvas.create_line(tx, ty - 1, tx, ty + 1, fill="#24214F")

    roll = rng.random()
    if roll < 0.20:
        # Pista de circuito con un quiebre de 45 grados
        sides = [(px, cy), (px + CELL, cy), (cx, py), (cx, py + CELL)]
        a, b = rng.sample(sides, 2)
        mid = (cx + rng.choice((-6, 6)), cy + rng.choice((-6, 6)))
        canvas.create_line(a[0], a[1], mid[0], mid[1], b[0], b[1],
                           fill=TRACE, width=2)
        if rng.random() < 0.6:
            circle(canvas, mid[0], mid[1], 3, "#0A0918", LIME, 1)
            circle(canvas, mid[0], mid[1], 1, LIME)
    elif roll < 0.29:
        # Cinta transportadora (chevrones ambar apagados)
        d = rng.choice(((1, 0), (-1, 0), (0, 1), (0, -1)))
        for k in (-7, 3):
            if d[0] != 0:
                bx = cx + k * d[0]
                canvas.create_line(bx - 3 * d[0], cy - 7, bx + 3 * d[0], cy,
                                   bx - 3 * d[0], cy + 7, fill="#8A5A10", width=3)
            else:
                by = cy + k * d[1]
                canvas.create_line(cx - 7, by - 3 * d[1], cx, by + 3 * d[1],
                                   cx + 7, by - 3 * d[1], fill="#8A5A10", width=3)
    elif roll < 0.34:
        # Pad luminoso
        circle(canvas, cx, cy, 5, "#0A0918", VIOLET, 1)
        circle(canvas, cx, cy, 2, VIOLET)

    # Sombra de los muros vecinos
    if is_wall(lines, x, y - 1):
        rect(canvas, px, py, px + CELL, py + 8, "#000000", stipple="gray50")
    if is_wall(lines, x - 1, y):
        rect(canvas, px, py, px + 7, py + CELL, "#000000", stipple="gray50")


# ---------------------------------------------------------------
#  Muros: fondo oscuro con borde (cornisa) hacia los pasillos
# ---------------------------------------------------------------

def draw_wall_base(canvas, x, y, lines, rng):
    px, py = x * CELL, y * CELL

    rect(canvas, px, py, px + CELL, py + CELL, VOID)
    for k in range(0, CELL, 8):
        canvas.create_line(px + k, py, px + k, py + CELL, fill=MESH)
        canvas.create_line(px, py + k, px + CELL, py + k, fill=MESH)

    # Cornisa metalica en los lados que dan al pasillo
    sides = (
        (0, -1, px, py, px + CELL, py + 4),
        (0, 1, px, py + CELL - 4, px + CELL, py + CELL),
        (-1, 0, px, py, px + 4, py + CELL),
        (1, 0, px + CELL - 4, py, px + CELL, py + CELL),
    )
    for dx, dy, x1, y1, x2, y2 in sides:
        if not is_wall(lines, x + dx, y + dy):
            rect(canvas, x1, y1, x2, y2, LEDGE, OUTLINE, 1)
            if dy == -1:
                canvas.create_line(x1 + 1, y1 + 1, x2 - 1, y1 + 1, fill=LEDGE_LIGHT)
            elif dx == -1:
                canvas.create_line(x1 + 1, y1 + 1, x1 + 1, y2 - 1, fill=LEDGE_LIGHT)


# ---------------------------------------------------------------
#  Tuberias
# ---------------------------------------------------------------

def draw_tube(canvas, ax, ay, bx, by, color, rng):
    horizontal = abs(by - ay) < 1

    # Sombra, casco, brillo, canal y energia
    canvas.create_line(ax + 3, ay + 4, bx + 3, by + 4, fill="#000000",
                       width=14, stipple="gray50")
    canvas.create_line(ax, ay, bx, by, fill=OUTLINE, width=14)
    canvas.create_line(ax, ay, bx, by, fill=STEEL, width=12)
    if horizontal:
        canvas.create_line(ax, ay - 4, bx, by - 4, fill=STEEL_LIGHT, width=2)
    else:
        canvas.create_line(ax - 4, ay, bx - 4, by, fill=STEEL_LIGHT, width=2)
    canvas.create_line(ax, ay, bx, by, fill=CHANNEL, width=6)
    canvas.create_line(ax, ay, bx, by, fill=color, width=8, stipple="gray25")
    canvas.create_line(ax, ay, bx, by, fill=color, width=2)

    # Pulso de energia viajando por la tuberia
    if rng.random() < 0.55:
        t = rng.uniform(0.25, 0.75)
        px_, py_ = ax + (bx - ax) * t, ay + (by - ay) * t
        circle(canvas, px_, py_, 4, color, stipple="gray50")
        circle(canvas, px_, py_, 2, "white")

    # Brida en la union entre celdas
    mx, my = (ax + bx) / 2, (ay + by) / 2
    if horizontal:
        rect(canvas, mx - 2, my - 9, mx + 2, my + 9, STEEL_DARK, OUTLINE, 1)
        circle(canvas, mx, my - 6, 1.2, STEEL_LIGHT)
        circle(canvas, mx, my + 6, 1.2, STEEL_LIGHT)
    else:
        rect(canvas, mx - 9, my - 2, mx + 9, my + 2, STEEL_DARK, OUTLINE, 1)
        circle(canvas, mx - 6, my, 1.2, STEEL_LIGHT)
        circle(canvas, mx + 6, my, 1.2, STEEL_LIGHT)


# ---------------------------------------------------------------
#  Nodos: reactores, valvulas, bombas, manometros
# ---------------------------------------------------------------

def node_hub(canvas, cx, cy, color, rng):
    circle(canvas, cx, cy, 17, color, stipple="gray12")
    rect(canvas, cx - 14, cy - 14, cx + 14, cy + 14, STEEL_DARK, OUTLINE, 2)
    for bx, by in ((-11, -11), (11, -11), (-11, 11), (11, 11)):
        circle(canvas, cx + bx, cy + by, 1.5, STEEL_LIGHT)
    circle(canvas, cx, cy, 12, STEEL, OUTLINE, 2)
    circle(canvas, cx, cy, 9, "", color, 2)
    circle(canvas, cx, cy, 7, CHANNEL)
    circle(canvas, cx, cy, 4.5, color)
    circle(canvas, cx, cy, 1.8, "white")


def node_cap(canvas, cx, cy, color, rng):
    circle(canvas, cx, cy, 15, color, stipple="gray12")
    circle(canvas, cx, cy, 12, STEEL, OUTLINE, 2)
    for i in range(8):
        a = i * math.pi / 4
        canvas.create_line(cx + math.cos(a) * 8, cy + math.sin(a) * 8,
                           cx + math.cos(a) * 12, cy + math.sin(a) * 12,
                           fill=STEEL_LIGHT)
    circle(canvas, cx, cy, 8, CHANNEL, color, 1)
    circle(canvas, cx, cy, 4, color)
    canvas.create_arc(cx - 11, cy - 11, cx + 11, cy + 11, start=110, extent=60,
                      style="arc", outline=STEEL_LIGHT, width=2)


def node_tower(canvas, cx, cy, color, rng):
    rect(canvas, cx - 15, cy - 15, cx + 15, cy + 15, STEEL_DARK, OUTLINE, 2)
    for r_ in (12, 8, 4):
        circle(canvas, cx, cy, r_, STEEL if r_ == 12 else "", color if r_ < 12 else OUTLINE, 2)
    circle(canvas, cx, cy, 2, "white")


def node_elbow(canvas, cx, cy, color, rng):
    circle(canvas, cx, cy, 9, STEEL, OUTLINE, 2)
    circle(canvas, cx, cy, 5, CHANNEL, STEEL_LIGHT, 1)
    circle(canvas, cx, cy, 2.5, color)
    for a in (0.8, 2.4, 4.0, 5.5):
        circle(canvas, cx + math.cos(a) * 7, cy + math.sin(a) * 7, 1, STEEL_LIGHT)


def node_valve(canvas, cx, cy, color, rng):
    circle(canvas, cx, cy, 10, STEEL, OUTLINE, 2)
    circle(canvas, cx, cy, 8, "", STEEL_LIGHT, 1)
    for a in (0, math.pi / 2):
        canvas.create_line(cx - math.cos(a) * 8, cy - math.sin(a) * 8,
                           cx + math.cos(a) * 8, cy + math.sin(a) * 8,
                           fill=STEEL_LIGHT, width=2)
    circle(canvas, cx, cy, 3, color, OUTLINE, 1)


def node_gauge(canvas, cx, cy, color, rng):
    circle(canvas, cx, cy, 10, STEEL_DARK, OUTLINE, 2)
    circle(canvas, cx, cy, 8, "#0E0D22", "#8F98D6", 1)
    canvas.create_arc(cx - 6, cy - 6, cx + 6, cy + 6, start=-30, extent=240,
                      style="arc", outline=color, width=2)
    canvas.create_arc(cx - 6, cy - 6, cx + 6, cy + 6, start=-30, extent=45,
                      style="arc", outline=RED, width=2)
    a = rng.uniform(-0.4, 3.5)
    canvas.create_line(cx, cy, cx + math.cos(a) * 6, cy + math.sin(a) * 6,
                       fill="white", width=2)


def node_pump(canvas, cx, cy, color, rng):
    rect(canvas, cx - 10, cy - 9, cx + 10, cy + 9, STEEL, OUTLINE, 2)
    for i in (-4, 0, 4):
        canvas.create_line(cx - 7, cy + i, cx + 3, cy + i, fill=STEEL_DARK, width=2)
    circle(canvas, cx + 6, cy - 4, 2, color)
    rect(canvas, cx + 3, cy + 1, cx + 8, cy + 6, color, stipple="gray50")


def node_capacitor(canvas, cx, cy, color, rng):
    for dx in (-6, 6):
        rect(canvas, cx + dx - 4, cy - 9, cx + dx + 4, cy + 9, STEEL, OUTLINE, 2)
        rect(canvas, cx + dx - 4, cy - 2, cx + dx + 4, cy + 2, color)
    canvas.create_line(cx - 2, cy - 7, cx + 2, cy - 7, fill=STEEL_LIGHT)


INLINE_DEVICES = [node_valve, node_gauge, node_pump, node_capacitor]


def draw_node(canvas, x, y, lines, rng):
    cx, cy = center(x, y)
    color = zone_color(x, y)

    sides = {
        "L": is_wall(lines, x - 1, y) and x - 1 >= 0,
        "R": is_wall(lines, x + 1, y) and x + 1 < SIZE,
        "U": is_wall(lines, x, y - 1) and y - 1 >= 0,
        "D": is_wall(lines, x, y + 1) and y + 1 < SIZE,
    }
    degree = sum(sides.values())

    if degree == 0:
        node_tower(canvas, cx, cy, color, rng)
    elif degree == 1:
        node_cap(canvas, cx, cy, color, rng)
    elif degree >= 3:
        node_hub(canvas, cx, cy, color, rng)
    else:
        straight = (sides["L"] and sides["R"]) or (sides["U"] and sides["D"])
        if straight:
            if rng.random() < 0.55:
                rng.choice(INLINE_DEVICES)(canvas, cx, cy, color, rng)
        else:
            node_elbow(canvas, cx, cy, color, rng)


# ---------------------------------------------------------------
#  Luces, vineta y marcadores
# ---------------------------------------------------------------

def draw_glows(canvas, lines, rng):
    roads = [(x, y) for y, line in enumerate(lines)
             for x, c in enumerate(line) if c != "#"]
    rng.shuffle(roads)
    for i, (x, y) in enumerate(roads[:8]):
        color = VIOLET if i % 2 == 0 else LIME
        cx, cy = center(x, y)
        circle(canvas, cx, cy, 46, color, stipple="gray12")
        circle(canvas, cx, cy, 28, color, stipple="gray25")
        circle(canvas, cx, cy, 3.5, color)
        circle(canvas, cx, cy, 1.5, "white")


def draw_vignette(canvas):
    for i, stipple in enumerate(("gray50", "gray25", "gray12")):
        d = 26 * (i + 1)
        rect(canvas, 0, 0, WIDTH, d, "#000000", stipple=stipple)
        rect(canvas, 0, HEIGHT - d, WIDTH, HEIGHT, "#000000", stipple=stipple)
        rect(canvas, 0, 0, d, HEIGHT, "#000000", stipple=stipple)
        rect(canvas, WIDTH - d, 0, WIDTH, HEIGHT, "#000000", stipple=stipple)


def draw_marker(canvas, cx, cy, kind):
    if kind in ("entrada", "salida"):
        color = GREEN if kind == "entrada" else RED
        circle(canvas, cx, cy, 20, color, stipple="gray25")
        circle(canvas, cx, cy, 14, color, stipple="gray50")
        circle(canvas, cx, cy, 11, color, "white", 2)
        canvas.create_text(cx, cy, text="E" if kind == "entrada" else "S",
                           fill="#071018", font=("DejaVu Sans", 12, "bold"))
    elif kind == "nucleo":
        circle(canvas, cx, cy, 15, ORANGE, stipple="gray25")
        circle(canvas, cx, cy, 11, "#071A20", ORANGE, 3)
        circle(canvas, cx, cy, 6, CYAN)
        circle(canvas, cx, cy, 2.5, "white")
    elif kind == "compuerta":
        rect(canvas, cx - 14, cy - 14, cx + 14, cy + 14, ORANGE, stipple="gray25")
        rect(canvas, cx - 12, cy - 12, cx + 12, cy + 12, "#071A20", ORANGE, 3)
        neon_line(canvas, (cx - 7, cy, cx + 7, cy), CYAN, 3)
    elif kind == "electrico":
        circle(canvas, cx, cy, 15, CYAN, stipple="gray25")
        circle(canvas, cx, cy, 11, "#071A20", CYAN, 2)
        canvas.create_polygon(cx + 2, cy - 9, cx - 5, cy + 1, cx - 1, cy + 1,
                              cx - 3, cy + 9, cx + 5, cy - 2, cx + 1, cy - 2,
                              fill=YELLOW, outline=OUTLINE)
    elif kind == "calor":
        circle(canvas, cx, cy, 15, ORANGE, stipple="gray25")
        circle(canvas, cx, cy, 11, "#1A0E08", ORANGE, 2)
        for r_ in (7, 4):
            canvas.create_arc(cx - r_, cy - r_, cx + r_, cy + r_, start=60, extent=240,
                              style="arc", outline=ORANGE, width=2)
    elif kind == "dron":
        circle(canvas, cx, cy, 15, RED, stipple="gray25")
        circle(canvas, cx, cy, 11, "#1A0A10", RED, 2)
        canvas.create_line(cx - 15, cy, cx + 15, cy, fill=RED, width=2)
        canvas.create_line(cx, cy - 15, cx, cy + 15, fill=RED, width=1)
        circle(canvas, cx, cy, 3, RED)
    elif kind == "control":
        circle(canvas, cx, cy, 15, "#3A9BFF", stipple="gray25")
        circle(canvas, cx, cy, 11, "#F2F6F8", "#3A9BFF", 2)
        canvas.create_polygon(cx, cy - 7, cx - 6, cy + 5, cx + 6, cy + 5,
                              fill="#2B6FD6", outline=OUTLINE)


def draw_markers(canvas, lines):
    for y, line in enumerate(lines):
        for x, char in enumerate(line):
            if char in ("E", "S"):
                cx, cy = center(x, y)
                draw_marker(canvas, cx, cy, MARKERS[char])


# ---------------------------------------------------------------
#  Complejo completo
# ---------------------------------------------------------------

def draw_complex(canvas, lines):
    canvas.delete("all")
    rng = random.Random(SEED)

    rect(canvas, 0, 0, WIDTH, HEIGHT, BG)

    # 1) Suelo de circuitos
    for y, line in enumerate(lines):
        for x, char in enumerate(line):
            if char != "#":
                draw_floor(canvas, x, y, lines, rng)

    # 2) Fondo de los muros con cornisa
    for y, line in enumerate(lines):
        for x, char in enumerate(line):
            if char == "#":
                draw_wall_base(canvas, x, y, lines, rng)

    # 3) Red de tuberias entre muros conectados
    for y, line in enumerate(lines):
        for x, char in enumerate(line):
            if char != "#":
                continue
            ax, ay = center(x, y)
            color = zone_color(x, y)
            if x + 1 < SIZE and lines[y][x + 1] == "#":
                draw_tube(canvas, ax, ay, ax + CELL, ay, color, rng)
            if y + 1 < SIZE and lines[y + 1][x] == "#":
                draw_tube(canvas, ax, ay, ax, ay + CELL, color, rng)

    # 4) Nodos: reactores, valvulas, bombas, manometros
    for y, line in enumerate(lines):
        for x, char in enumerate(line):
            if char == "#":
                draw_node(canvas, x, y, lines, rng)

    # 5) Luces, vineta y marcadores
    draw_glows(canvas, lines, rng)
    draw_vignette(canvas)
    draw_markers(canvas, lines)


# ---------------------------------------------------------------
#  Ventana
# ---------------------------------------------------------------


# ---------------------------------------------------------------
#  Retos interactivos y controles de Kharvex
# ---------------------------------------------------------------
from collections import deque


def route(lines, start, goal):
    q = deque([start])
    prev = {start: None}
    while q:
        x, y = q.popleft()
        if (x, y) == goal:
            result = []
            node = goal
            while node is not None:
                result.append(node)
                node = prev[node]
            return result[::-1]
        for dx, dy in ((0, -1), (1, 0), (0, 1), (-1, 0)):
            p = (x + dx, y + dy)
            nx, ny = p
            if (0 <= ny < SIZE and 0 <= nx < SIZE
                    and lines[ny][nx] != "#" and p not in prev):
                prev[p] = (x, y)
                q.append(p)
    return None


class KharvexGame:
    def __init__(self, window, canvas, lines, status, counters):
        self.window, self.canvas = window, canvas
        self.lines, self.status, self.counters = lines, status, counters
        self.start = self.find("E")
        self.goal = self.find("S")
        path = route(lines, self.start, self.goal)
        if not path or len(path) < 20:
            raise ValueError("El laberinto de Kharvex no tiene una ruta valida")
        n = len(path) - 1
        self.core = path[n // 4]
        self.gate = path[3 * n // 4]
        self.checkpoints = {path[n // 3], path[2 * n // 3]}
        reserved = {self.start, self.goal, self.core, self.gate} | self.checkpoints
        # Peligros repartidos por nueve sectores; sin superposiciones.
        sectors = [[] for _ in range(9)]
        for y, row in enumerate(lines):
            for x, ch in enumerate(row):
                p = (x, y)
                if ch != "#" and p not in reserved and abs(x-1)+abs(y-1) > 3:
                    sectors[min(y * 3 // SIZE, 2)*3 + min(x * 3 // SIZE, 2)].append(p)
        rng = random.Random(9602)
        for sector in sectors:
            rng.shuffle(sector)
        order = list(range(9))
        rng.shuffle(order)
        used = set(reserved)

        def choose(count):
            found = []
            while len(found) < count:
                progress = False
                for i in order:
                    while sectors[i] and sectors[i][-1] in used:
                        sectors[i].pop()
                    if sectors[i]:
                        p = sectors[i].pop()
                        used.add(p)
                        found.append(p)
                        progress = True
                        if len(found) == count:
                            break
                if not progress:
                    raise ValueError("No hay espacio suficiente para los obstaculos")
            return found

        self.fields = set(choose(7))
        self.heat = set(choose(7))
        drone_starts = choose(3)
        self.drone_routes = []
        for x, y in drone_starts:
            neighbors = [(x+dx, y+dy) for dx, dy in ((0,1),(1,0),(0,-1),(-1,0))
                         if 0 <= x+dx < SIZE and 0 <= y+dy < SIZE
                         and lines[y+dy][x+dx] != "#"
                         and (x+dx, y+dy) not in reserved
                         and (x+dx, y+dy) not in self.fields
                         and (x+dx, y+dy) not in self.heat]
            self.drone_routes.append([(x, y)] + neighbors[:1])
        self.reset()
        self.window.bind("<KeyPress>", self.on_key)
        self.window.after(650, self.tick)

    def find(self, ch):
        for y, row in enumerate(self.lines):
            for x, value in enumerate(row):
                if value == ch:
                    return (x, y)
        raise ValueError("Falta el marcador " + ch)

    def draw(self):
        c = self.canvas
        c.delete("kharvex_dynamic")
        before = set(c.find_all())
        for p in self.checkpoints:
            draw_marker(c, *center(*p), "control")
        if not self.has_core:
            draw_marker(c, *center(*self.core), "nucleo")
        if not self.gate_open:
            draw_marker(c, *center(*self.gate), "compuerta")
        for p in self.fields:
            draw_marker(c, *center(*p), "electrico")
        for p in self.heat:
            draw_marker(c, *center(*p), "calor")
        for route_, index in zip(self.drone_routes, self.drone_indices):
            draw_marker(c, *center(*route_[index]), "dron")
        for item in set(c.find_all()) - before:
            c.addtag_withtag("kharvex_dynamic", item)
        # Atenuar los peligros desactivados sin alterar su icono original.
        # Marcador del jugador, siempre en primer plano.
        x, y = center(*self.player)
        c.create_oval(x-12, y-12, x+12, y+12, fill="#071A20",
                      outline=CYAN, width=3, tags="kharvex_dynamic")
        c.create_polygon(x, y-9, x-8, y+7, x, y+3, x+8, y+7,
                         fill="white", outline=CYAN, tags="kharvex_dynamic")

    def message(self, text):
        self.counters.config(text=f"ENERGIA: {self.energy}   PASOS: {self.steps}   NUCLEO: {'SI' if self.has_core else 'NO'}")
        self.status.config(text=text)

    def reset(self):
        self.player = self.start
        self.checkpoint = self.start
        self.energy = 100
        self.steps = 0
        self.has_core = False
        self.gate_open = False
        self.visited = set()
        self.fields_active = True
        self.heat_active = True
        self.drone_indices = [0] * len(self.drone_routes)
        self.clock = 0
        self.completed = False
        self.draw()
        self.message("Recupera el nucleo, abre la compuerta y llega a S.")

    def on_key(self, event):
        key = event.keysym.lower()
        if key == "r":
            self.reset()
            return
        directions = {"w": (0,-1), "up": (0,-1), "s": (0,1), "down": (0,1),
                      "a": (-1,0), "left": (-1,0), "d": (1,0), "right": (1,0)}
        if key not in directions or self.completed:
            return
        dx, dy = directions[key]
        p = (self.player[0]+dx, self.player[1]+dy)
        x, y = p
        if not (0 <= x < SIZE and 0 <= y < SIZE) or self.lines[y][x] == "#":
            self.message("Pared mecanica: paso bloqueado.")
            return
        if p == self.gate and not self.has_core:
            self.message("Compuerta cerrada: recupera el nucleo de energia.")
            return
        if p in self.fields and self.fields_active:
            self.message("Campo electrico activo: espera a que se desactive.")
            return
        if p == self.gate:
            self.gate_open = True
        self.player = p
        self.steps += 1
        msg = "Explorando Kharvex."
        if p == self.core and not self.has_core:
            self.has_core = True
            msg = "Nucleo de energia recuperado."
        if p in self.checkpoints:
            self.checkpoint = p
            self.visited.add(p)
            msg = "Punto de control activado."
        if p in self.heat and self.heat_active:
            self.energy -= 20
            msg = "Zona de calor: -20 de energia."
        if p in [route_[i] for route_, i in zip(self.drone_routes, self.drone_indices)]:
            self.energy -= 30
            msg = "Dron de mantenimiento: -30 de energia."
        if self.energy <= 0:
            self.energy = 100
            self.player = self.checkpoint
            msg = "Energia agotada: regreso al ultimo control."
        if self.player == self.goal:
            self.completed = True
            msg = "KHARVEX COMPLETADO. Pulsa R para reiniciar."
        self.draw()
        self.message(msg)

    def tick(self):
        if not self.completed:
            self.clock += 1
            if self.clock % 4 == 0:
                self.fields_active = not self.fields_active
            if self.clock % 5 == 0:
                self.heat_active = not self.heat_active
            if self.clock % 3 == 0:
                self.drone_indices = [(i+1) % len(route_)
                                      for i, route_ in zip(self.drone_indices, self.drone_routes)]
            self.draw()
        self.window.after(650, self.tick)


def main():
    lines = load_lines()
    if len(lines) != SIZE or any(len(row) != SIZE for row in lines):
        raise ValueError("El mapa debe ser de 21x21")
    # Los retos se gestionan en memoria; el mapa Rust no se sobrescribe.
    normalized = ["".join(ch if ch in "#ES" else " " for ch in row) for row in lines]
    window = tk.Tk()
    window.title("Astral Echo | Kharvex - Mecanic Planet")
    window.configure(bg=BG)
    window.resizable(False, False)
    PAD, PANEL_WIDTH = 14, 290
    window.geometry(f"{WIDTH+PANEL_WIDTH+PAD*3}x{HEIGHT+185}")
    header = tk.Frame(window, bg=BG, height=84)
    header.pack(fill="x")
    header.pack_propagate(False)
    tk.Label(header, text="KHARVEX | MECANIC PLANET", bg=BG, fg=ORANGE,
             font=("DejaVu Sans", 20, "bold")).pack(pady=(8,0))
    tk.Label(header, text="COMPLEJO MECANICO | REACTORES Y NEON", bg=BG, fg=CYAN,
             font=("DejaVu Sans", 11)).pack()
    body = tk.Frame(window, bg=BG)
    body.pack(fill="both", expand=True, padx=PAD)
    canvas = tk.Canvas(body, width=WIDTH, height=HEIGHT, bg=BG,
                       highlightthickness=2, highlightbackground=CYAN)
    canvas.pack(side="left", anchor="n")
    draw_complex(canvas, normalized)
    slot = tk.Frame(body, bg="#121324", width=PANEL_WIDTH, height=HEIGHT)
    slot.pack(side="left", fill="y", padx=(PAD, 0))
    slot.pack_propagate(False)
    panel = tk.Frame(slot, bg="#121324")
    panel.pack(fill="both", expand=True)
    tk.Label(panel, text="INSTRUCCIONES", bg="#121324", fg=ORANGE,
             font=("DejaVu Sans", 14, "bold")).pack(pady=(10, 8))
    entries = [
        ("entrada", "ENTRADA", "Punto inicial."),
        ("salida", "SALIDA", "Completa el laberinto."),
        ("nucleo", "NUCLEO", "Recuperalo para abrir la compuerta."),
        ("compuerta", "COMPUERTA", "Necesita el nucleo de energia."),
        ("electrico", "CAMPO ELECTRICO", "Activo: bloquea el paso. Espera."),
        ("calor", "CALOR", "Activo al entrar: -20 de energia."),
        ("dron", "DRON", "Contacto al entrar: -30 de energia."),
        ("control", "CONTROL", "Guarda el punto de reaparicion."),
    ]
    for kind, title, description in entries:
        row = tk.Frame(panel, bg="#121324")
        row.pack(fill="x", padx=8, pady=4)
        icon = tk.Canvas(row, width=40, height=40, bg="#121324", highlightthickness=0)
        icon.pack(side="left", padx=(0,8))
        draw_marker(icon, 20, 20, kind)  # EXACTAMENTE el mismo dibujo del mapa
        info = tk.Frame(row, bg="#121324")
        info.pack(side="left", fill="x", expand=True)
        tk.Label(info, text=title, bg="#121324", fg=ORANGE,
                 font=("DejaVu Sans", 9, "bold")).pack(anchor="w")
        tk.Label(info, text=description, bg="#121324", fg="#D7DDE6",
                 font=("DejaVu Sans", 9), wraplength=190,
                 justify="left").pack(anchor="w")
    tk.Label(panel, text="WASD / FLECHAS: MOVER   R: REINICIAR",
             bg="#121324", fg=CYAN, font=("DejaVu Sans", 9, "bold"),
             wraplength=260).pack(pady=8)
    footer = tk.Frame(window, bg=BG, height=95)
    footer.pack(fill="x", padx=PAD)
    footer.pack_propagate(False)
    def toggle_panel():
        if panel.winfo_manager():
            panel.pack_forget()
            toggle.config(text="MOSTRAR INSTRUCCIONES")
        else:
            panel.pack(fill="both", expand=True)
            toggle.config(text="OCULTAR INSTRUCCIONES")
    toggle = tk.Button(footer, text="OCULTAR INSTRUCCIONES", command=toggle_panel,
                       bg="#27304B", fg="white")
    toggle.pack(anchor="w", pady=(2, 3))
    counters = tk.Label(footer, bg=BG, fg=CYAN,
                        font=("DejaVu Sans", 10, "bold"), anchor="w")
    counters.pack(fill="x")
    status = tk.Label(footer, bg=BG, fg="#D7DDE6", anchor="w", justify="left",
                      font=("DejaVu Sans", 10), wraplength=WIDTH+PANEL_WIDTH,
                      height=2)
    status.pack(fill="x")
    KharvexGame(window, canvas, normalized, status, counters)
    window.mainloop()


if __name__ == "__main__":
    main()
