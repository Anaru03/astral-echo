from collections import deque
import random

CYAN = "#19E6E0"
RED = "#FF365D"
GREEN = "#36F0A4"
YELLOW = "#FFD447"


def shortest_path(lines, start, goal, blocked=None):
    queue = deque([start])
    previous = {start: None}

    while queue:
        x, y = queue.popleft()

        if (x, y) == goal:
            path = []
            current = goal
            while current is not None:
                path.append(current)
                current = previous[current]
            return path[::-1]

        for dx, dy in ((0, -1), (1, 0), (0, 1), (-1, 0)):
            point = (x + dx, y + dy)
            nx, ny = point

            if not (0 <= ny < len(lines)):
                continue
            if not (0 <= nx < len(lines[ny])):
                continue
            if lines[ny][nx] == "#":
                continue
            if point == blocked or point in previous:
                continue

            previous[point] = (x, y)
            queue.append(point)

    return None


class BuildingObstacles:
    def __init__(self, canvas, lines, cell, status):
        self.canvas = canvas
        self.lines = lines
        self.cell = cell
        self.status = status
        self.start = self.find("E")
        self.goal = self.find("S")

        self.path = shortest_path(lines, self.start, self.goal)
        if not self.path or len(self.path) < 16:
            raise ValueError("Building necesita una ruta valida")

        self.rng = random.Random(9002)
        self.configure_positions()
        self.running = True
        self.reset()
        self.canvas.after(700, self.update)

    def find(self, symbol):
        for y, row in enumerate(self.lines):
            for x, value in enumerate(row):
                if value == symbol:
                    return (x, y)
        raise ValueError(f"No se encontro {symbol}")

    def configure_positions(self):
        n = len(self.path) - 1

        self.key_position = self.path[n // 4]
        self.door_position = self.path[3 * n // 4]

        self.checkpoints = {
            self.path[n // 3],
            self.path[2 * n // 3],
        }

        reserved = {
            self.start,
            self.goal,
            self.key_position,
            self.door_position,
        } | self.checkpoints

        walkable = [
            (x, y)
            for y, row in enumerate(self.lines)
            for x, char in enumerate(row)
            if char != "#" and (x, y) not in reserved
        ]

        # Dividir el mapa en sectores para distribuir peligros.
        sectors = [[] for _ in range(9)]
        for x, y in walkable:
            sx = min(x * 3 // len(self.lines[y]), 2)
            sy = min(y * 3 // len(self.lines), 2)
            sectors[sy * 3 + sx].append((x, y))

        for sector in sectors:
            self.rng.shuffle(sector)

        used = set(reserved)

        def choose(count):
            result = set()
            order = list(range(9))
            self.rng.shuffle(order)

            while len(result) < count:
                found = False
                for index in order:
                    while sectors[index] and sectors[index][-1] in used:
                        sectors[index].pop()

                    if sectors[index]:
                        point = sectors[index].pop()
                        used.add(point)
                        result.add(point)
                        found = True
                        if len(result) == count:
                            break

                if not found:
                    raise ValueError("No hay suficientes celdas libres")

            return result

        self.lasers = choose(6)
        self.steam = choose(7)

        drone_points = list(choose(3))
        self.drone_routes = []

        for point in drone_points:
            x, y = point
            neighbors = [
                (x + dx, y + dy)
                for dx, dy in ((0, -1), (1, 0), (0, 1), (-1, 0))
                if 0 <= y + dy < len(self.lines)
                and 0 <= x + dx < len(self.lines[y + dy])
                and self.lines[y + dy][x + dx] != "#"
                and (x + dx, y + dy) not in reserved
            ]

            if neighbors:
                self.drone_routes.append([point, neighbors[0]])
            else:
                self.drone_routes.append([point])

        self.drone_indices = [0] * len(self.drone_routes)

    def center(self, point):
        x, y = point
        return (
            x * self.cell + self.cell / 2,
            y * self.cell + self.cell / 2,
        )

    def circle(self, point, radius, color, tag):
        cx, cy = self.center(point)
        self.canvas.create_oval(
            cx-radius, cy-radius,
            cx+radius, cy+radius,
            fill=color, outline="white",
            width=2, tags=tag,
        )

    def draw(self):
        tag = "building_obstacle"
        self.canvas.delete(tag)

        for point in self.checkpoints:
            color = GREEN if point in self.activated_checkpoints else "#287DCA"
            self.circle(point, 9, color, tag)

        for point in self.steam:
            cx, cy = self.center(point)
            self.circle(
                point, 11,
                "#A9D8DF" if self.steam_active else "#36525A",
                tag,
            )
            self.canvas.create_text(
                cx, cy, text="V",
                fill="#071018",
                font=("DejaVu Sans", 10, "bold"),
                tags=tag,
            )

        for point in self.lasers:
            cx, cy = self.center(point)
            color = RED if self.laser_active else "#34505B"

            for x1, y1, x2, y2 in (
                (-11, -11, 11, 11),
                (11, -11, -11, 11),
            ):
                self.canvas.create_line(
                    cx+x1, cy+y1, cx+x2, cy+y2,
                    fill=color, width=3, tags=tag,
                )

        if not self.has_key:
            cx, cy = self.center(self.key_position)
            self.canvas.create_rectangle(
                cx-10, cy-7, cx+10, cy+7,
                fill=YELLOW, outline="white",
                tags=tag,
            )
            self.canvas.create_text(
                cx, cy, text="K",
                fill="#151515", tags=tag,
            )

        if not self.door_open:
            cx, cy = self.center(self.door_position)
            self.canvas.create_rectangle(
                cx-13, cy-13, cx+13, cy+13,
                fill="#62202D", outline=RED,
                width=3, tags=tag,
            )
            self.canvas.create_text(
                cx, cy, text="D",
                fill="white", tags=tag,
            )

        for route, index in zip(
            self.drone_routes, self.drone_indices
        ):
            point = route[index]
            self.circle(point, 10, "#36102C", tag)
            self.circle(point, 3, RED, tag)

        self.canvas.tag_raise("maze_player")

    def update(self):
        if not self.running:
            return

        self.tick += 1

        if self.tick % 4 == 0:
            self.laser_active = not self.laser_active

        if self.tick % 5 == 0:
            self.steam_active = not self.steam_active

        if self.tick % 3 == 0:
            for i, route in enumerate(self.drone_routes):
                self.drone_indices[i] = (
                    self.drone_indices[i] + 1
                ) % len(route)

        self.draw()
        self.canvas.after(700, self.update)

    def can_enter(self, point):
        if point == self.door_position and not self.door_open:
            if not self.has_key:
                return False, "NECESITAS LA TARJETA"

            self.door_open = True
            self.draw()

        if point in self.lasers and self.laser_active:
            return False, "LASER ACTIVO: ESPERA"

        return True, None

    def enter(self, point):
        messages = []

        if point == self.key_position and not self.has_key:
            self.has_key = True
            messages.append("TARJETA OBTENIDA")

        if point in self.checkpoints:
            self.active_checkpoint = point
            if point not in self.activated_checkpoints:
                self.activated_checkpoints.add(point)
                messages.append("CONTROL ACTIVADO")

        if point in self.steam and self.steam_active:
            self.energy -= 20
            messages.append("VAPOR: -20")

        drone_positions = [
            route[index]
            for route, index in zip(
                self.drone_routes, self.drone_indices
            )
        ]

        if point in drone_positions:
            self.energy -= 30
            messages.append("DRON: -30")

        if self.energy <= 0:
            self.energy = 100
            messages.append("REGRESO AL CONTROL")
            point = self.active_checkpoint

        self.draw()
        return point, messages

    def reset(self):
        self.energy = 100
        self.has_key = False
        self.door_open = False
        self.active_checkpoint = self.start
        self.activated_checkpoints = set()
        self.laser_active = True
        self.steam_active = True
        self.drone_indices = [0] * len(self.drone_routes)
        self.tick = 0
        self.draw()
