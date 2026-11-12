
import tkinter as tk
import random
import math
import time

BG = "#0B101C"
PANEL = "#151D2B"
BORDER = "#40566B"
TEXT = "#E8F3FF"
CYAN = "#80D5E8"
GREEN = "#36F0A4"
RED = "#FF3548"
YELLOW = "#FFC233"

COLORS = [
    "#FF3548",
    "#FFC233",
    "#347CFF",
    "#FF38C8",
    "#27D6AA",
    "#A16AFF",
]

LEVELS = [
    ("PANEL AUXILIAR", 3),
    ("DISTRIBUCION ELECTRICA", 4),
    ("SISTEMA DE NAVEGACION", 5),
    ("REACTOR PRINCIPAL", 6),
]

TOTAL_TIME = 60
TRANSITION_MS = 1500

WIDTH = 900
HEIGHT = 590


class WiringGame:
    def __init__(self, root, on_success=None, on_close=None):
        self.root = root
        self.on_success = on_success
        self.on_close = on_close
        self.root.title("Astral Echo | Mantenimiento de la nave")
        self.root.geometry(f"{WIDTH}x{HEIGHT}")
        self.root.resizable(False, False)
        self.root.configure(bg=BG)

        self.level = 0
        self.phase = "playing"
        self.generation = 0

        self.dragging = None
        self.mouse = None
        self.connections = {}
        self.starts = {}
        self.ends = {}

        self.deadline = time.monotonic() + TOTAL_TIME

        self.title = tk.Label(
            root,
            text="MANTENIMIENTO DE LA NAVE",
            font=("DejaVu Sans", 23, "bold"),
            fg=TEXT,
            bg=BG,
        )
        self.title.pack(pady=(20, 8))

        self.level_label = tk.Label(
            root,
            text="",
            font=("DejaVu Sans", 14, "bold"),
            fg=YELLOW,
            bg=BG,
        )
        self.level_label.pack(pady=5)

        self.timer_label = tk.Label(
            root,
            text="02:00",
            font=("DejaVu Sans", 17, "bold"),
            fg=CYAN,
            bg=BG,
        )
        self.timer_label.pack(pady=(3, 10))

        self.canvas = tk.Canvas(
            root,
            width=850,
            height=420,
            bg=PANEL,
            highlightthickness=2,
            highlightbackground=BORDER,
        )
        self.canvas.pack(pady=8)

        self.canvas.bind("<ButtonPress-1>", self.mouse_down)
        self.canvas.bind("<B1-Motion>", self.mouse_move)
        self.canvas.bind("<ButtonRelease-1>", self.mouse_up)

        self.start_level()
        self.update_timer()

    def start_level(self):
        self.phase = "playing"
        self.dragging = None
        self.mouse = None
        self.connections.clear()

        name, count = LEVELS[self.level]
        self.count = count

        self.left_colors = COLORS[:count].copy()
        self.right_colors = COLORS[:count].copy()

        random.shuffle(self.right_colors)

        if self.left_colors == self.right_colors:
            self.right_colors.reverse()

        self.level_label.config(
            text=f"NIVEL {self.level + 1}/4 | {name}"
        )

        self.draw()

    def positions(self):
        top = 65
        bottom = 365
        gap = (bottom - top) / (self.count - 1)

        return [
            top + i * gap
            for i in range(self.count)
        ]

    def draw_terminal(self, x, y, color, left=True):
        x1, x2 = (x - 65, x) if left else (x, x + 65)

        self.canvas.create_rectangle(
            x1, y - 18,
            x2, y + 18,
            fill="#263548",
            outline="#8498AA",
            width=2,
        )

        self.canvas.create_rectangle(
            x1 + 5, y - 10,
            x2 - 5, y + 10,
            fill=color,
            outline="",
        )

        self.canvas.create_oval(
            x - 7, y - 7,
            x + 7, y + 7,
            fill=BG,
            outline="#D6E8F5",
            width=2,
        )

    def draw_wire(self, start, end, color, width=7):
        x1, y1 = start
        x2, y2 = end
        points = []

        for i in range(45):
            t = i / 44
            smooth = 3 * t * t - 2 * t * t * t
            x = x1 + (x2 - x1) * t
            y = y1 + (y2 - y1) * smooth
            points.extend([x, y])

        for wire_color, wire_width in [
            ("#02040A", width + 5),
            (color, width),
            ("#FFFFFF", 1),
        ]:
            self.canvas.create_line(
                points,
                fill=wire_color,
                width=wire_width,
                smooth=True,
                capstyle=tk.ROUND,
                joinstyle=tk.ROUND,
            )

    def draw(self):
        self.canvas.delete("all")

        self.canvas.create_rectangle(
            25, 25, 825, 395,
            fill="#0D1421",
            outline=BORDER,
            width=2,
        )

        self.canvas.create_text(
            110, 35,
            text="ENTRADA",
            fill="#AFC5D6",
            font=("DejaVu Sans", 11, "bold"),
        )

        self.canvas.create_text(
            740, 35,
            text="SALIDA",
            fill="#AFC5D6",
            font=("DejaVu Sans", 11, "bold"),
        )

        for x in range(120, 760, 30):
            self.canvas.create_line(
                x, 45, x, 385,
                fill="#1D293A",
            )

        ys = self.positions()
        self.starts = {}
        self.ends = {}

        for i, color in enumerate(self.left_colors):
            y = ys[i]
            self.starts[color] = (120, y)
            self.draw_terminal(120, y, color, True)

        for i, color in enumerate(self.right_colors):
            y = ys[i]
            self.ends[color] = (730, y)
            self.draw_terminal(730, y, color, False)

        for color in self.connections:
            self.draw_wire(
                self.starts[color],
                self.ends[color],
                color,
            )

        if self.dragging and self.mouse:
            self.draw_wire(
                self.starts[self.dragging],
                self.mouse,
                self.dragging,
                width=5,
            )

    def find_start(self, x, y):
        for color, (sx, sy) in self.starts.items():
            if color in self.connections:
                continue

            if math.hypot(x - sx, y - sy) < 25:
                return color

        return None

    def find_end(self, x, y):
        for color, (ex, ey) in self.ends.items():
            if math.hypot(x - ex, y - ey) < 28:
                return color

        return None

    def mouse_down(self, event):
        if self.phase != "playing":
            return

        if time.monotonic() >= self.deadline:
            self.time_out()
            return

        color = self.find_start(event.x, event.y)

        if color:
            self.dragging = color
            self.mouse = (event.x, event.y)
            self.draw()

    def mouse_move(self, event):
        if self.phase != "playing" or not self.dragging:
            return

        self.mouse = (event.x, event.y)
        self.draw()

    def mouse_up(self, event):
        if self.phase != "playing" or not self.dragging:
            return

        if time.monotonic() >= self.deadline:
            self.time_out()
            return

        target = self.find_end(event.x, event.y)
        selected = self.dragging

        self.dragging = None
        self.mouse = None

        # Soltar fuera de un terminal cancela el arrastre.
        if target is None:
            self.draw()
            return

        if target != selected:
            self.show_message(
                "ERROR",
                RED,
                self.restart_game,
            )
            return

        self.connections[selected] = True
        self.draw()

        if len(self.connections) == self.count:
            self.complete_level()

    def show_message(self, message, color, callback):
        self.phase = "transition"
        self.dragging = None
        self.mouse = None
        self.draw()

        # Fondo central para destacar el resultado.
        self.canvas.create_rectangle(
            175, 145, 675, 275,
            fill="#080D17",
            outline=color,
            width=4,
            tags="message",
        )

        self.canvas.create_text(
            425, 210,
            text=message,
            fill=color,
            font=("DejaVu Sans", 29, "bold"),
            tags="message",
        )

        self.generation += 1
        current_generation = self.generation

        def finish():
            if current_generation == self.generation:
                callback()

        self.root.after(TRANSITION_MS, finish)

    def complete_level(self):
        if self.level < len(LEVELS) - 1:
            self.show_message(
                "CORRECTO",
                GREEN,
                self.next_level,
            )
        else:
            self.phase = "completed"
            self.generation += 1
            self.draw()

            self.canvas.create_rectangle(
                90, 145, 760, 275,
                fill="#080D17",
                outline=GREEN,
                width=4,
            )

            self.canvas.create_text(
                425, 210,
                text="MANTENIMIENTO COMPLETADO",
                fill=GREEN,
                font=("DejaVu Sans", 21, "bold"),
            )
            if self.on_success:
                self.root.after(800, self.on_success)

    def next_level(self):
        self.level += 1
        self.start_level()

    def restart_game(self):
        self.generation += 1
        self.level = 0
        self.deadline = time.monotonic() + TOTAL_TIME
        self.start_level()
        self.update_timer_display()

    def time_out(self):
        if self.phase in ("completed", "timeout"):
            return

        self.phase = "timeout"
        self.show_message(
            "TIEMPO AGOTADO",
            RED,
            self.restart_game,
        )
        self.phase = "timeout"

    def update_timer_display(self):
        remaining = max(
            0,
            math.ceil(self.deadline - time.monotonic())
        )

        minutes, seconds = divmod(remaining, 60)

        self.timer_label.config(
            text=f"{minutes:02d}:{seconds:02d}",
            fg=RED if remaining <= 20 else CYAN,
        )

    def update_timer(self):
        if self.phase not in ("completed", "timeout"):
            self.update_timer_display()

            if time.monotonic() >= self.deadline:
                self.time_out()

        self.root.after(100, self.update_timer)

    def close(self):
        self.generation += 1
        if self.on_close:
            self.on_close()


def open_wiring_task(parent, on_success=None):
    """Open wiring as one stage of the ship maintenance mission."""
    win = tk.Toplevel(parent)
    win.title("Astral Echo | Mantenimiento de la nave")
    win.configure(bg=BG)
    win.resizable(False, False)
    task = WiringGame(win, on_success=on_success, on_close=win.destroy)
    win.protocol("WM_DELETE_WINDOW", task.close)
    win.transient(parent)
    win.grab_set()
    return task


def main():
    root = tk.Tk()
    task = WiringGame(root, on_close=root.destroy)
    root.protocol("WM_DELETE_WINDOW", task.close)
    root.mainloop()


if __name__ == "__main__":
    main()
