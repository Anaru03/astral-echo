import tkinter as tk
import random
import math
from pathlib import Path

# ---------------------------------------------------------------
#  TESSARA | CLOCK PLANET  -  laberinto de relojeria (vista cenital)
#  Engranajes de bronce, piedra azul oscura, suelo de ladrillos,
#  tuberias de cobre, relojes y manometros.
# ---------------------------------------------------------------

MAP = Path("/tmp/astral_echo_mazes/clock.txt")

CELL = 34
SIZE = 21
WIDTH = CELL * SIZE
HEIGHT = CELL * SIZE
SEED = 4242

# Paleta (inspirada en la imagen: bronce + piedra azul verdosa)
BG = "#0D0A0B"
STONE = "#15252D"
STONE_LIGHT = "#223A44"
STONE_SEAM = "#0B151A"
BOLT = "#6F8B93"
OUTLINE = "#050809"

COPPER = "#B8692E"
COPPER_LIGHT = "#E39B55"
COPPER_DARK = "#6A3517"
BRASS = "#C9A24A"
DARK_DISC = "#1A100C"

WARM_BRICKS = ["#7A5C48", "#8A6A52", "#6E5240", "#94705A", "#7C5A50"]
CREAM_BRICKS = ["#D9D3C3", "#C9C2B0", "#E4DECF", "#BDB6A3"]
MORTAR_WARM = "#2A1F1A"
MORTAR_CREAM = "#8E8878"

GEAR_PALETTES = [
    (COPPER, COPPER_LIGHT, COPPER_DARK),
    ("#A85A28", "#D98B48", "#5E2F14"),
    ("#B88A3A", "#E2B965", "#6B4D18"),
    ("#8E5A3A", "#C98B63", "#4E2C18"),
]

GEM_COLORS = ["#E04040", "#36F0A4", "#3A9BFF", "#F2C879"]

# Simbolos opcionales del mapa (si existen en el archivo se dibujan)
MARKERS = {
    "E": ("#2EE6D6", "E"),
    "S": ("#FF526E", "S"),
    "K": ("#F2C879", "K"),
    "V": ("#E8EEF0", "V"),
    "T": ("#FF5A5A", "T"),
    "!": ("#E04848", "!"),
    "C": ("#3A7BD5", "C"),
    "D": ("#2A1612", "D"),
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


def is_wall(lines, x, y):
    if 0 <= y < len(lines) and 0 <= x < len(lines[y]):
        return lines[y][x] == "#"
    return True


# ---------------------------------------------------------------
#  Generacion de mapa (solo si no existe el archivo)
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

    for _ in range(20):
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
#  Engranajes y relojes
# ---------------------------------------------------------------

def gear_points(cx, cy, r_out, r_in, teeth, rot):
    pts = []
    step = 2 * math.pi / teeth
    for i in range(teeth):
        a = rot + i * step
        for frac, rad in ((0.00, r_in), (0.12, r_out), (0.48, r_out), (0.60, r_in)):
            ang = a + frac * step
            pts.extend((cx + math.cos(ang) * rad, cy + math.sin(ang) * rad))
    return pts


def draw_gear(canvas, cx, cy, r, teeth, rot, palette, rng, gem=True):
    base, light, dark = palette

    # Disco oscuro de fondo (como en el mapa de referencia)
    circle(canvas, cx, cy, r + 1.5, "#0A0706", OUTLINE, 1)

    canvas.create_polygon(
        gear_points(cx, cy, r, r * 0.80, teeth, rot),
        fill=base, outline=dark, width=1,
    )

    # Aro interior y disco oscuro
    circle(canvas, cx, cy, r * 0.70, DARK_DISC, dark, 1)
    circle(canvas, cx, cy, r * 0.58, "", base, 2)

    # Radios
    for i in range(3):
        a = rot + i * 2 * math.pi / 3 + math.pi / 6
        canvas.create_line(cx, cy, cx + math.cos(a) * r * 0.62, cy + math.sin(a) * r * 0.62,
                           fill=dark, width=4)
        canvas.create_line(cx, cy, cx + math.cos(a) * r * 0.62, cy + math.sin(a) * r * 0.62,
                           fill=base, width=2)

    # Cubo central con brillo
    circle(canvas, cx, cy, r * 0.24, base, dark, 1)
    circle(canvas, cx - 1, cy - 1, r * 0.09, light)

    if gem and rng.random() < 0.35:
        color = rng.choice(GEM_COLORS)
        a = rot + 0.9
        gx, gy = cx + math.cos(a) * r * 0.45, cy + math.sin(a) * r * 0.45
        circle(canvas, gx, gy, 3, color, OUTLINE, 1)
        circle(canvas, gx - 1, gy - 1, 1, "white")

    # Brillo en los dientes
    canvas.create_arc(cx - r, cy - r, cx + r, cy + r, start=110, extent=50,
                      style="arc", outline=light, width=2)


def draw_clock_face(canvas, cx, cy, rng):
    circle(canvas, cx, cy, 15, COPPER_DARK, OUTLINE, 2)
    circle(canvas, cx, cy, 13, "#E9DFC6", COPPER, 2)
    for i in range(12):
        a = i * math.pi / 6
        r1 = 11 if i % 3 == 0 else 12
        canvas.create_line(cx + math.cos(a) * r1, cy + math.sin(a) * r1,
                           cx + math.cos(a) * 13, cy + math.sin(a) * 13,
                           fill="#2A1A10", width=2 if i % 3 == 0 else 1)
    ah = rng.uniform(0, 2 * math.pi)
    am = rng.uniform(0, 2 * math.pi)
    canvas.create_line(cx, cy, cx + math.cos(ah) * 6, cy + math.sin(ah) * 6,
                       fill="#2A1A10", width=3)
    canvas.create_line(cx, cy, cx + math.cos(am) * 10, cy + math.sin(am) * 10,
                       fill="#2A1A10", width=2)
    circle(canvas, cx, cy, 2, "#B03A2E")


def draw_gauge(canvas, cx, cy, rng):
    circle(canvas, cx, cy, 14, COPPER, OUTLINE, 2)
    circle(canvas, cx, cy, 11, "#102229", COPPER_LIGHT, 1)
    canvas.create_arc(cx - 9, cy - 9, cx + 9, cy + 9, start=-30, extent=240,
                      style="arc", outline="#36F0A4", width=2)
    canvas.create_arc(cx - 9, cy - 9, cx + 9, cy + 9, start=-30, extent=50,
                      style="arc", outline="#E04040", width=2)
    a = rng.uniform(-0.5, 3.6)
    canvas.create_line(cx, cy, cx + math.cos(a) * 8, cy + math.sin(a) * 8,
                       fill="#F2E6C8", width=2)
    circle(canvas, cx, cy, 2, COPPER_LIGHT)


# ---------------------------------------------------------------
#  Suelo: ladrillos
# ---------------------------------------------------------------

def draw_floor(canvas, x, y, lines, rng):
    px, py = x * CELL, y * CELL
    cream = ((x // 3) + (y // 3)) % 4 == 0
    palette = CREAM_BRICKS if cream else WARM_BRICKS
    mortar = MORTAR_CREAM if cream else MORTAR_WARM

    rect(canvas, px, py, px + CELL, py + CELL, mortar)

    rows = ((0, 11), (11, 23), (23, 34))
    for ri, (ya, yb) in enumerate(rows):
        global_row = y * 3 + ri
        offset = 8 if global_row % 2 else 0
        bx = px if offset == 0 else px + offset - 17
        while bx < px + CELL:
            xa = max(bx, px)
            xb = min(bx + 17, px + CELL)
            if xb - xa > 1:
                color = rng.choice(palette)
                # Algunos ladrillos mas rojizos
                if not cream and rng.random() < 0.06:
                    color = "#7C4E50"
                rect(canvas, xa + 1, py + ya + 1, xb - 1, py + yb - 1, color)
                canvas.create_line(xa + 1, py + ya + 1, xb - 1, py + ya + 1,
                                   fill="#FFFFFF", stipple="gray12")
            bx += 17

    # Desgaste
    if rng.random() < 0.25:
        gx = px + rng.randint(4, CELL - 14)
        gy = py + rng.randint(4, CELL - 12)
        rect(canvas, gx, gy, gx + rng.randint(6, 12), gy + rng.randint(4, 8),
             "#000000", stipple="gray12")

    # Sombras proyectadas por los muros vecinos (arriba e izquierda)
    if is_wall(lines, x, y - 1):
        rect(canvas, px, py, px + CELL, py + 8, "#000000", stipple="gray50")
    if is_wall(lines, x - 1, y):
        rect(canvas, px, py, px + 7, py + CELL, "#000000", stipple="gray50")


# ---------------------------------------------------------------
#  Muros: piedra con borde de cobre
# ---------------------------------------------------------------

def draw_wall_base(canvas, x, y, lines, rng):
    px, py = x * CELL, y * CELL

    rect(canvas, px, py, px + CELL, py + CELL, STONE)

    # Bloques de piedra
    canvas.create_line(px, py + CELL / 2, px + CELL, py + CELL / 2, fill=STONE_SEAM)
    canvas.create_line(px + CELL / 2, py, px + CELL / 2, py + CELL / 2, fill=STONE_SEAM)
    canvas.create_line(px + CELL / 4, py + CELL / 2, px + CELL / 4, py + CELL, fill=STONE_SEAM)
    canvas.create_line(px + CELL * 3 / 4, py + CELL / 2, px + CELL * 3 / 4, py + CELL,
                       fill=STONE_SEAM)
    canvas.create_line(px + 1, py + 1, px + CELL - 1, py + 1, fill=STONE_LIGHT)

    # Borde de cobre hacia los pasillos
    edges = (
        (0, -1, px, py + 1, px + CELL, py + 1),
        (0, 1, px, py + CELL - 1, px + CELL, py + CELL - 1),
        (-1, 0, px + 1, py, px + 1, py + CELL),
        (1, 0, px + CELL - 1, py, px + CELL - 1, py + CELL),
    )
    for dx, dy, ax, ay, bx, by in edges:
        if not is_wall(lines, x + dx, y + dy):
            canvas.create_line(ax, ay, bx, by, fill=OUTLINE, width=5)
            canvas.create_line(ax, ay, bx, by, fill=COPPER_DARK, width=4)
            canvas.create_line(ax, ay, bx, by, fill=COPPER, width=2)
            if dx == 0:
                off = 1 if dy < 0 else -1
                canvas.create_line(ax, ay + off, bx, by + off, fill=COPPER_LIGHT)
            else:
                off = 1 if dx < 0 else -1
                canvas.create_line(ax + off, ay, bx + off, by, fill=COPPER_LIGHT)


def draw_pipes(canvas, x, y, lines, rng):
    cx, cy = x * CELL + CELL / 2, y * CELL + CELL / 2
    for dx, dy in ((1, 0), (0, 1)):
        nx, ny = x + dx, y + dy
        if (0 <= nx < SIZE and 0 <= ny < SIZE and is_wall(lines, nx, ny)
                and rng.random() < 0.40):
            ex, ey = cx + dx * CELL, cy + dy * CELL
            canvas.create_line(cx, cy, ex, ey, fill=OUTLINE, width=9)
            canvas.create_line(cx, cy, ex, ey, fill=COPPER_DARK, width=7)
            canvas.create_line(cx, cy, ex, ey, fill=COPPER, width=5)
            if dx:
                canvas.create_line(cx, cy - 1, ex, ey - 1, fill=COPPER_LIGHT, width=1)
            else:
                canvas.create_line(cx - 1, cy, ex - 1, ey, fill=COPPER_LIGHT, width=1)
            # Abrazaderas
            mx, my = (cx + ex) / 2, (cy + ey) / 2
            circle(canvas, mx, my, 5, BRASS, OUTLINE, 1)
            circle(canvas, mx, my, 1.5, COPPER_DARK)


def draw_wall_decor(canvas, x, y, lines, rng):
    px, py = x * CELL, y * CELL
    cx, cy = px + CELL / 2, py + CELL / 2

    roll = rng.random()
    if roll < 0.60:
        # Engranajes que encajan en tablero de ajedrez
        step = math.pi / 5
        rot = (step / 2 if (x + y) % 2 else 0) + rng.uniform(-0.05, 0.05)
        draw_gear(canvas, cx, cy, 15, 10, rot, rng.choice(GEAR_PALETTES), rng)
    elif roll < 0.72:
        draw_clock_face(canvas, cx, cy, rng)
    elif roll < 0.82:
        draw_gauge(canvas, cx, cy, rng)
    else:
        # Placa remachada
        rect(canvas, px + 6, py + 6, px + CELL - 6, py + CELL - 6,
             COPPER_DARK, OUTLINE, 1)
        rect(canvas, px + 8, py + 8, px + CELL - 8, py + CELL - 8, COPPER)
        for bx, by in ((10, 10), (CELL - 10, 10), (10, CELL - 10), (CELL - 10, CELL - 10)):
            circle(canvas, px + bx, py + by, 1.5, COPPER_LIGHT)
        canvas.create_line(px + 11, cy, px + CELL - 11, cy, fill=COPPER_DARK, width=2)

    # Pernos en las esquinas
    for bx, by in ((4, 4), (CELL - 4, 4), (4, CELL - 4), (CELL - 4, CELL - 4)):
        if rng.random() < 0.5:
            circle(canvas, px + bx, py + by, 1.5, BOLT)


# ---------------------------------------------------------------
#  Luces, vineta y marcadores
# ---------------------------------------------------------------

def draw_glows(canvas, lines, rng):
    roads = [(x, y) for y, line in enumerate(lines)
             for x, c in enumerate(line) if c != "#"]
    rng.shuffle(roads)

    for i, (x, y) in enumerate(roads[:8]):
        color = "#FFB347" if i % 2 == 0 else "#36F0D4"
        cx, cy = x * CELL + CELL / 2, y * CELL + CELL / 2
        circle(canvas, cx, cy, 50, color, stipple="gray12")
        circle(canvas, cx, cy, 32, color, stipple="gray25")
        circle(canvas, cx, cy, 4, color)
        circle(canvas, cx, cy, 1.5, "white")


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
            if char not in MARKERS:
                continue

            cx = x * CELL + CELL / 2
            cy = y * CELL + CELL / 2
            color, label = MARKERS[char]

            if char == "D":
                rect(canvas, cx - 12, cy - 12, cx + 12, cy + 12, color, "#FF3B3B", 3)
                canvas.create_text(cx, cy, text=label, fill="#F2E6C8",
                                   font=("DejaVu Sans", 11, "bold"))
                continue

            if char in ("E", "S"):
                circle(canvas, cx, cy, 20, color, stipple="gray25")
                circle(canvas, cx, cy, 14, color, stipple="gray50")
            circle(canvas, cx, cy, 11, color, "white", 2)
            canvas.create_text(cx, cy, text=label, fill="#071018",
                               font=("DejaVu Sans", 12, "bold"))


# ---------------------------------------------------------------
#  Mundo completo
# ---------------------------------------------------------------

def draw_world(canvas, lines):
    canvas.delete("all")
    rng = random.Random(SEED)

    rect(canvas, 0, 0, WIDTH, HEIGHT, BG)

    # 1) Suelo de ladrillos
    for y, line in enumerate(lines):
        for x, char in enumerate(line):
            if char != "#":
                draw_floor(canvas, x, y, lines, rng)

    # 2) Muros de piedra con borde de cobre
    for y, line in enumerate(lines):
        for x, char in enumerate(line):
            if char == "#":
                draw_wall_base(canvas, x, y, lines, rng)

    # 3) Tuberias de cobre entre muros
    for y, line in enumerate(lines):
        for x, char in enumerate(line):
            if char == "#":
                draw_pipes(canvas, x, y, lines, rng)

    # 4) Engranajes, relojes y manometros
    for y, line in enumerate(lines):
        for x, char in enumerate(line):
            if char == "#":
                draw_wall_decor(canvas, x, y, lines, rng)

    # 5) Luces, vineta y marcadores
    draw_glows(canvas, lines, rng)
    draw_vignette(canvas)
    draw_markers(canvas, lines)


# ---------------------------------------------------------------
#  Ventana
# ---------------------------------------------------------------

# ---------------------------------------------------------------
# Retos jugables de Tessara
# ---------------------------------------------------------------
from collections import deque


def ruta_corta(lines, inicio, fin):
    cola = deque([inicio])
    anterior = {inicio: None}
    while cola:
        x, y = cola.popleft()
        if (x, y) == fin:
            resultado = []
            punto = fin
            while punto is not None:
                resultado.append(punto)
                punto = anterior[punto]
            return resultado[::-1]
        for dx, dy in ((0, -1), (1, 0), (0, 1), (-1, 0)):
            nx, ny = x + dx, y + dy
            if (0 <= ny < SIZE and 0 <= nx < SIZE
                    and lines[ny][nx] != "#" and (nx, ny) not in anterior):
                anterior[(nx, ny)] = (x, y)
                cola.append((nx, ny))
    return None


class ClockGame:
    def __init__(self, window, canvas, lines, status):
        self.window, self.canvas, self.lines, self.status = window, canvas, lines, status
        self.start, self.goal = (1, 1), (SIZE - 2, SIZE - 2)
        self.path = ruta_corta(lines, self.start, self.goal)
        if self.path is None:
            raise ValueError("El laberinto de Tessara no tiene salida")
        self.rng = random.Random(9601)
        n = len(self.path) - 1
        self.key = self.path[n // 4]
        self.door = self.path[3 * n // 4]
        self.checkpoints = {self.path[n // 3], self.path[2 * n // 3]}
        reserved = {self.start, self.goal, self.key, self.door} | self.checkpoints
        # Los peligros se reparten por nueve sectores, nunca sobre objetos clave.
        sectors = [[] for _ in range(9)]
        for y, row in enumerate(lines):
            for x, symbol in enumerate(row):
                point = (x, y)
                if symbol != "#" and point not in reserved:
                    sectors[min(y * 3 // SIZE, 2) * 3 + min(x * 3 // SIZE, 2)].append(point)
        for sector in sectors:
            self.rng.shuffle(sector)
        used = set(reserved)

        def distribute(count):
            result = set()
            while len(result) < count:
                changed = False
                for sector in sectors:
                    while sector and sector[-1] in used:
                        sector.pop()
                    if sector and len(result) < count:
                        point = sector.pop()
                        used.add(point)
                        result.add(point)
                        changed = True
                if not changed:
                    raise ValueError("No hay espacio para los obstaculos")
            return result

        self.gears = distribute(7)
        self.steam = distribute(6)
        self.sentinels = list(sorted(distribute(3)))
        self.reset()
        self.window.bind("<KeyPress>", self.on_key)
        self.window.after(650, self.tick)

    def reset(self):
        self.player = self.start
        self.checkpoint = self.start
        self.visited_checkpoints = set()
        self.energy = 100
        self.steps = 0
        self.has_key = False
        self.door_open = False
        self.completed = False
        self.clock = 0
        self.gears_active = True
        self.steam_active = True
        self.draw()
        self.message("BUSCA LA LLAVE K Y LLEGA A S")

    def center(self, point):
        return (point[0] * CELL + CELL / 2, point[1] * CELL + CELL / 2)

    def draw(self):
        c = self.canvas
        c.delete("clock_game")
        tag = "clock_game"
        for point in self.checkpoints:
            x, y = self.center(point)
            color = "#36F0A4" if point in self.visited_checkpoints else "#348BEE"
            c.create_oval(x-9, y-9, x+9, y+9, fill=color, outline="white", tags=tag)
        for point in self.steam:
            x, y = self.center(point)
            c.create_oval(x-10, y-10, x+10, y+10,
                          fill="#D9E6E5" if self.steam_active else "#51605F",
                          outline=COPPER, width=2, tags=tag)
            c.create_text(x, y, text="V", fill="#231610", tags=tag)
        for point in self.gears:
            x, y = self.center(point)
            color = "#F05A38" if self.gears_active else "#79614C"
            c.create_oval(x-12, y-12, x+12, y+12, fill="#23130D", outline=color, width=3, tags=tag)
            c.create_text(x, y, text="G", fill=color, font=("DejaVu Sans", 10, "bold"), tags=tag)
        if not self.has_key:
            x, y = self.center(self.key)
            c.create_rectangle(x-10, y-8, x+10, y+8, fill="#FFD76A", outline="white", tags=tag)
            c.create_text(x, y, text="K", fill="#241409", tags=tag)
        if not self.door_open:
            x, y = self.center(self.door)
            c.create_rectangle(x-13, y-13, x+13, y+13, fill="#66251D", outline="#FF7049", width=2, tags=tag)
            c.create_text(x, y, text="D", fill="white", tags=tag)
        for i, point in enumerate(self.sentinels):
            x, y = self.center(point)
            c.create_oval(x-9, y-9, x+9, y+9, fill="#461D27", outline="#FF5C77", width=2, tags=tag)
            c.create_text(x, y, text="!", fill="white", tags=tag)
        x, y = self.center(self.player)
        c.create_oval(x-12, y-12, x+12, y+12, fill="#09232C", outline="#40F3E5", width=3, tags=tag)
        c.create_polygon(x, y-9, x-8, y+7, x, y+3, x+8, y+7,
                         fill="white", outline="#40F3E5", tags=tag)

    def message(self, message):
        self.status.config(text=(f"ENERGIA {self.energy}  |  PASOS {self.steps}  |  "
                                 f"LLAVE {'SI' if self.has_key else 'NO'}\n{message}"))

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
        nx, ny = self.player[0] + dx, self.player[1] + dy
        point = (nx, ny)
        if not (0 <= nx < SIZE and 0 <= ny < SIZE) or self.lines[ny][nx] == "#":
            self.message("MURO: PASO BLOQUEADO")
            return
        if point == self.door and not self.has_key:
            self.message("PUERTA CERRADA: NECESITAS LA LLAVE K")
            return
        if point in self.gears and self.gears_active:
            self.message("ENGRANAJE ACTIVO: ESPERA QUE SE DETENGA")
            return
        if point == self.door:
            self.door_open = True
        self.player = point
        self.steps += 1
        msg = "EXPLORANDO TESSARA"
        if point == self.key and not self.has_key:
            self.has_key = True
            msg = "LLAVE MECANICA OBTENIDA"
        if point in self.checkpoints:
            self.checkpoint = point
            self.visited_checkpoints.add(point)
            msg = "PUNTO DE CONTROL ACTIVADO"
        if point in self.steam and self.steam_active:
            self.energy -= 20
            msg = "VAPOR CALIENTE: -20 ENERGIA"
        if point in self.sentinels:
            self.energy -= 25
            msg = "CENTINELA MECANICO: -25 ENERGIA"
        if self.energy <= 0:
            self.energy = 100
            self.player = self.checkpoint
            msg = "ENERGIA AGOTADA: REGRESAS AL CONTROL"
        if self.player == self.goal:
            self.completed = True
            msg = "TESSARA COMPLETADO - PULSA R PARA REINICIAR"
        self.draw()
        self.message(msg)

    def tick(self):
        if not self.completed:
            self.clock += 1
            if self.clock % 4 == 0:
                self.gears_active = not self.gears_active
            if self.clock % 5 == 0:
                self.steam_active = not self.steam_active
            self.draw()
        self.window.after(650, self.tick)


def main():
    lines = load_lines()
    if len(lines) != SIZE or any(len(line) != SIZE for line in lines):
        raise ValueError("El mapa debe ser de 21x21")
    window = tk.Tk()
    window.title("Astral Echo | Tessara - Clock Planet")
    window.configure(bg=BG)
    window.resizable(False, False)
    PANEL_WIDTH, PAD = 280, 16
    HEADER_HEIGHT, FOOTER_HEIGHT = 85, 100
    window.geometry(f"{WIDTH + PANEL_WIDTH + PAD * 3}x{HEIGHT + HEADER_HEIGHT + FOOTER_HEIGHT}")
    header = tk.Frame(window, bg=BG, height=HEADER_HEIGHT)
    header.pack(fill="x")
    header.pack_propagate(False)
    tk.Label(header, text="TESSARA | CLOCK PLANET", bg=BG, fg="#F2B35E",
             font=("DejaVu Sans", 20, "bold")).pack(pady=(10, 0))
    tk.Label(header, text="LABERINTO DE RELOJERIA | ENGRANAJES Y VAPOR",
             bg=BG, fg="#C98B4A", font=("DejaVu Sans", 10)).pack()
    body = tk.Frame(window, bg=BG)
    body.pack(fill="both", expand=True, padx=PAD)
    canvas = tk.Canvas(body, width=WIDTH, height=HEIGHT, bg=BG,
                       highlightthickness=2, highlightbackground=COPPER)
    canvas.pack(side="left", anchor="n")
    draw_world(canvas, lines)
    panel_slot = tk.Frame(body, bg=BG, width=PANEL_WIDTH, height=HEIGHT)
    panel_slot.pack(side="left", fill="y", padx=(PAD, 0))
    panel_slot.pack_propagate(False)
    panel = tk.Frame(panel_slot, bg="#1C1818")
    panel.pack(fill="both", expand=True)
    tk.Label(panel, text="INSTRUCCIONES", bg="#1C1818", fg="#F2B35E",
             font=("DejaVu Sans", 13, "bold")).pack(anchor="w", padx=12, pady=(12, 8))


    def legend_icon(parent, kind):
        icon = tk.Canvas(
            parent,
            width=42,
            height=42,
            bg="#1C1818",
            highlightthickness=0,
        )
        icon.pack(side="left", padx=(8, 10))

        x, y = 21, 21

        if kind == "gear":
            # Mismo engranaje G del laberinto.
            color = "#F05A38"
            icon.create_oval(
                x-12, y-12, x+12, y+12,
                fill="#23130D",
                outline=color,
                width=3,
            )
            icon.create_text(
                x, y, text="G",
                fill=color,
                font=("DejaVu Sans", 10, "bold"),
            )

        elif kind == "steam":
            # Mismo vapor V del laberinto.
            icon.create_oval(
                x-10, y-10, x+10, y+10,
                fill="#D9E6E5",
                outline=COPPER,
                width=2,
            )
            icon.create_text(
                x, y, text="V",
                fill="#231610",
            )

        elif kind == "sentinel":
            # Mismo centinela ! del laberinto.
            icon.create_oval(
                x-9, y-9, x+9, y+9,
                fill="#461D27",
                outline="#FF5C77",
                width=2,
            )
            icon.create_text(
                x, y, text="!",
                fill="white",
            )

        elif kind == "key":
            # Misma tarjeta K del laberinto.
            icon.create_rectangle(
                x-10, y-8, x+10, y+8,
                fill="#FFD76A",
                outline="white",
            )
            icon.create_text(
                x, y, text="K",
                fill="#241409",
            )

        elif kind == "door":
            # Misma puerta D del laberinto.
            icon.create_rectangle(
                x-13, y-13, x+13, y+13,
                fill="#66251D",
                outline="#FF7049",
                width=2,
            )
            icon.create_text(
                x, y, text="D",
                fill="white",
            )

        elif kind == "checkpoint":
            # Mismo circulo azul del laberinto.
            icon.create_oval(
                x-10, y-10, x+10, y+10,
                fill="#4288E8",
                outline="white",
                width=2,
            )

        elif kind == "entrance":
            # Entrada verde del mapa.
            icon.create_oval(
                x-11, y-11, x+11, y+11,
                fill="#2EE6D6",
                outline="white",
                width=2,
            )
            icon.create_text(
                x, y, text="E",
                fill="#071018",
                font=("DejaVu Sans", 12, "bold"),
            )

        elif kind == "exit":
            # Salida roja del mapa.
            icon.create_oval(
                x-11, y-11, x+11, y+11,
                fill="#FF526E",
                outline="white",
                width=2,
            )
            icon.create_text(
                x, y, text="S",
                fill="#071018",
                font=("DejaVu Sans", 12, "bold"),
            )

        return icon

    legend = [
        ("entrance", "ENTRADA", "Punto inicial del jugador."),
        ("exit", "SALIDA", "Llega aqui para completar Tessara."),
        ("gear", "ENGRANAJE G", "Si esta activo bloquea el paso. Espera."),
        ("steam", "VAPOR V", "Si esta activo al entrar, pierdes 20 de energia."),
        ("sentinel", "CENTINELA !", "Al entrar en su celda pierdes 25 de energia."),
        ("key", "LLAVE K", "Recogela para desbloquear la puerta."),
        ("door", "PUERTA D", "No permite pasar sin haber recogido K."),
        ("checkpoint", "CONTROL AZUL", "Guarda donde reapareces si agotas la energia."),
    ]

    for kind, name, description in legend:
        row = tk.Frame(panel, bg="#1C1818")
        row.pack(fill="x", padx=6, pady=4)

        legend_icon(row, kind)

        information = tk.Frame(row, bg="#1C1818")
        information.pack(side="left", fill="x", expand=True)

        tk.Label(
            information,
            text=name,
            bg="#1C1818",
            fg="#D79B54",
            font=("DejaVu Sans", 9, "bold"),
        ).pack(anchor="w")

        tk.Label(
            information,
            text=description,
            bg="#1C1818",
            fg="#E1D1BB",
            font=("DejaVu Sans", 9),
            wraplength=190,
            justify="left",
        ).pack(anchor="w")

    tk.Label(
        panel,
        text="WASD / FLECHAS: MOVER    R: REINICIAR",
        bg="#1C1818",
        fg="#F2B35E",
        font=("DejaVu Sans", 8, "bold"),
        wraplength=PANEL_WIDTH - 20,
    ).pack(pady=(10, 0))

    footer = tk.Frame(window, bg=BG, height=FOOTER_HEIGHT)
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
                       bg="#443021", fg="white", activebackground="#65432A")
    toggle.pack(anchor="w", pady=(4, 0))
    status = tk.Label(footer, text="", bg=BG, fg="#E1D1BB", anchor="w",
                      justify="left", font=("DejaVu Sans", 10, "bold"), height=2,
                      wraplength=WIDTH + PANEL_WIDTH)
    status.pack(fill="x")
    ClockGame(window, canvas, lines, status)
    window.mainloop()


if __name__ == "__main__":
    main()
