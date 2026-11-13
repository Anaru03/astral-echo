"""Astral Echo - minijuego "Limpiar filtro de O2" (tkinter).

Mecanica: el filtro de aire esta lleno de hojas. Agarra cada hoja con el
mouse y arrastrala hasta el conducto oscuro de la izquierda; al soltarla ahi
es succionada. Cuando no quede ninguna hoja, la tarea se completa.

Uso independiente:   python o2_filter.py
Dentro de tu juego:  open_o2_task(ventana, on_success=mi_funcion)
"""
import math
import random
import time
import tkinter as tk

now = time.monotonic

# ---------------------------------------------------------------
#  Dimensiones (todo el layout sale de aqui)
# ---------------------------------------------------------------
W, H = 900, 650

PANEL = (145, 235, 755, 525)              # conducto + filtro
PX1, PY1, PX2, PY2 = PANEL
CHUTE = (145, 235, 245, 525)              # conducto (zona de descarte)
FILTER = (245, 235, 755, 525)             # filtro azul con las hojas
FX1, FY1, FX2, FY2 = FILTER
SLOT = (160, 266, 230, 498)              # abertura oscura del conducto
SLOT_CX = (SLOT[0] + SLOT[2]) / 2

N_LEAVES = 5
SUCK_TIME = 0.40                         # segundos de la animacion de succion

# Colores
BG = "#2a3a3e"
TEXT = "#f2f2f2"
CURRENT = "#f5e663"
DONE = "#3df04a"
VEIN = "#4a1408"
GLOW_OFF, GLOW_ON = "#2a2a2a", "#3df04a"
CHEV_OFF, CHEV_ON = "#4a4a4a", "#7dff8a"

# (relleno, centro, contorno)
PALETTE = [
    ("#e0661f", "#f6b845", "#7a1c0c"),
    ("#b5381a", "#ea8f33", "#58110a"),
    ("#7f9524", "#cfc43e", "#3b4a10"),
    ("#c9531a", "#93ad2c", "#6a1a0a"),
    ("#d84a1c", "#f2a03a", "#6b150a"),
]

FONT = "DejaVu Sans"
MONO = "DejaVu Sans Mono"


def rrect(c, x1, y1, x2, y2, r, **kw):
    pts = [x1 + r, y1, x2 - r, y1, x2, y1, x2, y1 + r, x2, y2 - r, x2, y2,
           x2 - r, y2, x1 + r, y2, x1, y2, x1, y2 - r, x1, y1 + r, x1, y1]
    return c.create_polygon(pts, smooth=True, **kw)


def lerp_hex(c1, c2, t):
    a = [int(c1[i:i + 2], 16) for i in (1, 3, 5)]
    b = [int(c2[i:i + 2], 16) for i in (1, 3, 5)]
    return "#%02x%02x%02x" % tuple(int(a[i] + (b[i] - a[i]) * t) for i in range(3))


class BaseFilterTask(tk.Canvas):
    def __init__(self, master, on_success=None, on_close=None,
                 task_name="O2: Limpiar filtro", progress=0.2, gain=0.2,
                 n_leaves=N_LEAVES):
        super().__init__(master, width=W, height=H, bg=BG, highlightthickness=0)
        self.on_success, self.on_close = on_success, on_close
        self.task_name = task_name
        self.progress, self.gain = progress, gain
        self.n_leaves = n_leaves

        self.leaves = []             # orden de apilado: el ultimo queda arriba
        self.drag = None
        self.grab = (0, 0)
        self.done = False
        self.closing = False
        self.t_last = now()

        self.build_left()
        self.build_chute()
        self.build_filter()
        self.build_leaves()
        self.leaf_top = self.create_rectangle(0, 0, 1, 1, state="hidden", outline="")
        self.build_frame()
        self.build_close()

        self.bind("<ButtonPress-1>", self.on_press)
        self.bind("<B1-Motion>", self.on_move)
        self.bind("<ButtonRelease-1>", self.on_release)
        self.bind("<Destroy>", self.on_destroy)
        self.loop_job = self.after(16, self.tick)

    # ------------------------------------------------------------
    #  Dibujo
    # ------------------------------------------------------------
    def build_left(self):
        # Barra de progreso
        bx1, by1, bx2, by2 = 16, 14, 350, 50
        self.create_rectangle(bx1, by1, bx2, by2, fill="#17302a", outline="#9db8b0", width=3)
        self.bar_x0, self.bar_x1 = bx1 + 6, bx2 - 6
        self.bar_y = (by1 + 6, by2 - 6)
        self.bar = self.create_rectangle(self.bar_x0, self.bar_y[0], self.bar_x0, self.bar_y[1],
                                         fill="#43d94f", outline="")
        self.set_progress(self.progress)

        # Lista de tareas (la actual en amarillo)
        self.task_items = []
        tasks = (self.task_name, "Reactor: Abrir colectores", "Escudos: Cargar escudos")
        for i, name in enumerate(tasks):
            item = self.create_text(20, 72 + i * 32, anchor="nw", text=name,
                                    fill=CURRENT if i == 0 else TEXT, font=(FONT, 13), width=320)
            self.task_items.append(item)

        # Panel de ayuda
        rrect(self, 16, 330, 350, 624, 14, fill="#314549", outline="#3d555a", width=2)
        self.create_text(183, 352, text="CÓMO SE JUEGA", fill="#9db8b0", font=(FONT, 11, "bold"))
        self.create_text(36, 384, anchor="nw", fill=TEXT, font=(FONT, 12), width=292,
                         text="1. Agarra una hoja del filtro con el mouse.\n\n"
                              "2. Arrástrala hasta el conducto de la izquierda y suéltala.\n\n"
                              "3. Quita todas las hojas para limpiar el filtro.")
        self.create_text(183, 604, text="O2", fill="#9db8b0", font=(FONT, 16, "bold"))

    def build_chute(self):
        x1, y1, x2, y2 = CHUTE
        # Cuerpo metalico (degradado horizontal)
        for x in range(x1, x2):
            v = int(215 - 75 * (x - x1) / (x2 - x1))
            self.create_line(x, y1, x, y2, fill=f"#{v:02x}{v:02x}{v:02x}")
        # Tornillos
        for sx, sy in ((x1 + 14, y1 + 14), (x2 - 14, y1 + 14), (x1 + 14, y2 - 14), (x2 - 14, y2 - 14)):
            self.create_oval(sx - 5, sy - 5, sx + 5, sy + 5, fill="#8d8d8d", outline="#444")
            self.create_line(sx - 3, sy + 3, sx + 3, sy - 3, fill="#444")
        # Abertura oscura
        sx1, sy1, sx2, sy2 = SLOT
        self.create_rectangle(sx1 - 5, sy1 - 5, sx2 + 5, sy2 + 5, fill="#6a6a6a", outline="")
        self.slot_rect = self.create_rectangle(sx1, sy1, sx2, sy2, fill="#0a0a0a",
                                               outline=GLOW_OFF, width=4)
        self.create_line(sx1 + 6, sy1 + 6, sx1 + 6, sy2 - 6, fill="#1c1c1c", width=2)
        # Chevrones que indican hacia donde arrastrar
        self.chevrons = []
        for cy in (250, 330, 410):
            self.chevrons.append(self.create_line(SLOT_CX + 12, cy - 15, SLOT_CX - 12, cy,
                                                  SLOT_CX + 12, cy + 15, fill=CHEV_OFF, width=5))
        # Flecha metalica de la parte baja del conducto
        self.create_polygon(x1 + 18, 570, x1 + 52, 552, x1 + 52, 588, fill="#9a9a9a", outline="#555", width=2)

    def build_filter(self):
        # Degradado azul horizontal (claro a la izquierda, mas profundo a la derecha)
        for x in range(FX1, FX2):
            t = (x - FX1) / (FX2 - FX1)
            self.create_line(x, FY1, x, FY2, fill=lerp_hex("#cfdcf7", "#6f8fd2", t))
        # Brillo diagonal
        self.create_polygon(FX1 + 60, FY1, FX1 + 190, FY1, FX1 + 40, FY2, FX1 - 60, FY2,
                            fill="#ffffff", outline="", stipple="gray12")
        # Marco interior
        self.create_rectangle(FX1, FY1, FX2, FY2, outline="#5a6f98", width=3)

    def build_frame(self):
        # Marco general y sombra del conducto
        self.create_rectangle(PX1, PY1, PX2, PY2, outline="#111", width=5)
        self.create_line(FX1, FY1, FX1, FY2, fill="#222", width=4)
        # Contador y banner de exito
        self.count_text = self.create_text((PX1 + PX2) / 2, 626, fill=TEXT, font=(MONO, 14, "bold"),
                                           text=f"Hojas restantes: {self.n_leaves}")
        cx, cy = (FX1 + FX2) / 2, (FY1 + FY2) / 2
        self.create_rectangle(cx - 160, cy - 34, cx + 160, cy + 34, fill="#0c2a4a",
                              outline="#cfe0ff", width=3, tags="banner", state="hidden")
        self.create_text(cx, cy, text="FILTRO LIMPIO", fill="#e6f0ff",
                         font=(MONO, 24, "bold"), tags="banner", state="hidden")

    def build_close(self):
        self.create_oval(PX1 - 20, PY1 - 20, PX1 + 20, PY1 + 20, fill="#ececec",
                         outline="#555", width=3, tags="close")
        self.create_line(PX1 - 9, PY1 - 9, PX1 + 9, PY1 + 9, fill="#222", width=4, tags="close")
        self.create_line(PX1 + 9, PY1 - 9, PX1 - 9, PY1 + 9, fill="#222", width=4, tags="close")
        self.tag_bind("close", "<ButtonPress-1>", lambda e: self.close())

    # ------------------------------------------------------------
    #  Hojas
    # ------------------------------------------------------------
    def build_leaves(self):
        placed = []
        for i in range(self.n_leaves):
            for _ in range(300):
                x = random.uniform(FX1 + 80, FX2 - 80)
                y = random.uniform(FY1 + 80, FY2 - 80)
                if all(math.hypot(x - px, y - py) > 100 for px, py in placed):
                    break
            placed.append((x, y))
            self.leaves.append(self.new_leaf(i, x, y))

    def new_leaf(self, i, x, y):
        n = 12
        l = dict(x=x, y=y, ang=random.uniform(0, math.tau),
                 L=random.uniform(88, 106), Wd=random.uniform(38, 48),
                 jt=[random.uniform(-0.13, 0.13) for _ in range(n - 1)],
                 jb=[random.uniform(-0.13, 0.13) for _ in range(n - 1)],
                 pal=PALETTE[i % len(PALETTE)], tag=f"leaf{i}",
                 state="idle", s=1.0, t=0.0, x0=0, y0=0, ty=0, a0=0)
        coords = self.leaf_layout(l)
        fill, inner, line = l["pal"]
        tag = l["tag"]
        items = [
            self.create_polygon(*coords[0], fill=fill, outline=line, width=2, tags=tag),
            self.create_polygon(*coords[1], fill=inner, outline="", tags=tag),
        ]
        for c in coords[2:-1]:
            items.append(self.create_line(*c, fill=VEIN, width=2, tags=tag))
        items.append(self.create_line(*coords[-1], fill=line, width=3, tags=tag))
        l["items"] = items
        return l

    @staticmethod
    def leaf_layout(l, s=1.0):
        """Devuelve listas de coordenadas: [cuerpo, centro, nervaduras..., tallo]."""
        cr, sr = math.cos(l["ang"]), math.sin(l["ang"])
        x, y, L, Wd = l["x"], l["y"], l["L"], l["Wd"]

        def T(lx, ly):
            return (x + s * (lx * cr - ly * sr), y + s * (lx * sr + ly * cr))

        def flat(pts):
            return [c for p in pts for c in T(*p)]

        n = len(l["jt"]) + 1
        top, bot = [], []
        for k in range(n + 1):
            u = k / n
            h = (Wd / 2) * math.sin(math.pi * u) ** 0.8
            lx = -L / 2 + u * L
            jt = l["jt"][k - 1] if 0 < k < n else 0
            jb = l["jb"][k - 1] if 0 < k < n else 0
            top.append((lx, -h * (1 + jt)))
            bot.append((lx, h * (1 + jb)))
        body = flat(top + list(reversed(bot[1:-1])))

        m = 8
        itop, ibot = [], []
        for k in range(m + 1):
            u = k / m
            h = (Wd / 2) * 0.5 * math.sin(math.pi * u) ** 0.8
            lx = -L / 2 + (0.12 + 0.76 * u) * L
            itop.append((lx, -h))
            ibot.append((lx, h))
        inner = flat(itop + list(reversed(ibot[1:-1])))

        parts = [body, inner, flat([(-L / 2 - 2, 0), (L / 2 * 0.92, 0)])]
        for k in range(3):
            lx = -L / 2 + (0.22 + 0.22 * k) * L
            for d in (-1, 1):
                parts.append(flat([(lx, 0), (lx + L * 0.14, d * Wd * 0.36 * (1 - 0.15 * k))]))
        parts.append(flat([(-L / 2 - 14, 4), (-L / 2, 0)]))
        return parts

    def redraw_leaf(self, l, s=1.0):
        for item, c in zip(l["items"], self.leaf_layout(l, s)):
            self.coords(item, *c)

    def leaf_at(self, px, py):
        for l in reversed(self.leaves):          # el de arriba primero
            if l["state"] != "idle":
                continue
            dx, dy = px - l["x"], py - l["y"]
            c, s = math.cos(l["ang"]), math.sin(l["ang"])
            lx, ly = dx * c + dy * s, -dx * s + dy * c
            body = (lx / (l["L"] / 2 + 6)) ** 2 + (ly / (l["Wd"] / 2 + 8)) ** 2 <= 1
            stem = -l["L"] / 2 - 16 <= lx <= -l["L"] / 2 and abs(ly) <= 8
            if body or stem:
                return l
        return None

    # ------------------------------------------------------------
    #  Entrada
    # ------------------------------------------------------------
    def on_press(self, e):
        if self.done or self.closing or math.hypot(e.x - PX1, e.y - PY1) <= 24:
            return
        l = self.leaf_at(e.x, e.y)
        if not l:
            return
        self.drag = l
        self.grab = (e.x - l["x"], e.y - l["y"])
        self.leaves.remove(l)
        self.leaves.append(l)                    # queda arriba de las demas
        self.tag_lower(l["tag"], self.leaf_top)
        self.configure(cursor="fleur")

    def on_move(self, e):
        l = self.drag
        if not l:
            return
        mg = l["L"] / 2 + 14
        nx = max(PX1 + mg, min(PX2 - mg, e.x - self.grab[0]))
        ny = max(PY1 + mg, min(PY2 - mg, e.y - self.grab[1]))
        self.move(l["tag"], nx - l["x"], ny - l["y"])
        l["x"], l["y"] = nx, ny
        self.set_glow(self.in_chute(l))

    def on_release(self, e):
        l = self.drag
        if not l:
            return
        self.drag = None
        self.configure(cursor="")
        self.set_glow(False)
        if self.in_chute(l):
            self.start_suck(l)

    @staticmethod
    def in_chute(l):
        return l["x"] < FX1

    def set_glow(self, on):
        self.itemconfigure(self.slot_rect, outline=GLOW_ON if on else GLOW_OFF)
        for c in self.chevrons:
            self.itemconfigure(c, fill=CHEV_ON if on else CHEV_OFF)

    # ------------------------------------------------------------
    #  Succion y progreso
    # ------------------------------------------------------------
    def start_suck(self, l):
        l["state"] = "sucking"
        l["t"] = 0.0
        l["x0"], l["y0"] = l["x"], l["y"]
        l["ty"] = max(SLOT[1] + 50, min(SLOT[3] - 50, l["y"]))
        a0 = l["ang"] % math.tau
        l["a0"] = a0
        l["da"] = ((math.pi - a0 + math.pi) % math.tau) - math.pi   # girar hasta apuntar a la izquierda

    def finish_leaf(self, l):
        self.delete(*l["items"])
        self.leaves.remove(l)
        left = len(self.leaves)
        self.itemconfigure(self.count_text, text=f"Hojas restantes: {left}")
        if left == 0:
            self.win()

    def win(self):
        self.done = True
        self.set_progress(self.progress + self.gain)
        self.itemconfigure(self.task_items[0], text=f"{self.task_name}  (completada)", fill=DONE)
        self.itemconfigure("banner", state="normal")
        if self.on_success:
            self.after(900, self.on_success)

    def set_progress(self, p):
        self.progress = max(0.0, min(1.0, p))
        x = self.bar_x0 + (self.bar_x1 - self.bar_x0) * self.progress
        self.coords(self.bar, self.bar_x0, self.bar_y[0], x, self.bar_y[1])

    # ------------------------------------------------------------
    #  Bucle (solo anima la succion)
    # ------------------------------------------------------------
    def tick(self):
        t = now()
        dt = min(0.05, t - self.t_last)
        self.t_last = t
        for l in list(self.leaves):
            if l["state"] != "sucking":
                continue
            l["t"] += dt / SUCK_TIME
            p = min(1.0, l["t"])
            e = p * p * (3 - 2 * p)
            l["x"] = l["x0"] + (SLOT_CX - l["x0"]) * e
            l["y"] = l["y0"] + (l["ty"] - l["y0"]) * e
            l["ang"] = l["a0"] + l["da"] * e
            self.redraw_leaf(l, max(0.02, 1 - e))
            if p >= 1.0:
                self.finish_leaf(l)
        self.loop_job = self.after(16, self.tick)

    def on_destroy(self, event):
        if event.widget is self and self.loop_job:
            self.after_cancel(self.loop_job)
            self.loop_job = None

    def close(self):
        self.closing = True
        if self.on_close:
            self.on_close()



# ------------------------------------------------------------------
# Astral Echo: interfaz unificada con los minijuegos de cables/codigo.
# Se reutilizan los dibujos vectoriales y la mecanica original de hojas.
# ------------------------------------------------------------------
DARK = "#0B101C"
SURFACE = "#151D2B"
EDGE = "#40566B"
WHITE = "#E8F3FF"
CYAN = "#80D5E8"
YELLOW = "#FFC233"
GREEN = "#36F0A4"
RED = "#FF3548"
LEVEL_NAMES = ("FILTRO AUXILIAR", "VENTILACION", "SOPORTE VITAL", "OXIGENO PRINCIPAL")
LEVEL_LEAVES = (5, 6, 7, 8)
TOTAL_TIME = 40  # 40 segundos para los cuatro niveles


class CleanFilterTask(BaseFilterTask):
    def __init__(self, master, on_success=None, on_close=None,
                 task_name="O2: Limpiar filtro", progress=0.2, gain=0.2,
                 n_leaves=N_LEAVES):
        self.level = 0
        self.deadline = now() + TOTAL_TIME
        self.phase = "playing"
        self.transition_job = None
        super().__init__(master, on_success=on_success, on_close=on_close,
                         task_name=task_name, progress=progress, gain=gain,
                         n_leaves=LEVEL_LEAVES[0])
        self.configure(bg=DARK)
        self.timer_job = self.after(100, self.update_timer)

    def build_left(self):
        # Cabecera igual a los otros minijuegos de Astral Echo.
        self.create_text(W / 2, 48, text="LIMPIEZA DE FILTRO O2",
                         fill=WHITE, font=(FONT, 27, "bold"))
        self.level_text = self.create_text(W / 2, 96,
                         text="NIVEL 1/4 | FILTRO AUXILIAR",
                         fill=YELLOW, font=(FONT, 16, "bold"))
        self.timer_text = self.create_text(W / 2, 136, text="00:40",
                         fill=CYAN, font=(MONO, 21, "bold"))
        self.create_rectangle(22, 173, 878, 632, fill=SURFACE,
                              outline=EDGE, width=2)
        self.create_rectangle(42, 190, 858, 615, fill="#0D1421",
                              outline=EDGE, width=2)
        for x in range(58, 850, 32):
            self.create_line(x, 198, x, 606, fill="#182537")
        self.create_text(W / 2, 207,
                         text="SISTEMA DE FILTRACION  //  AE-08",
                         fill=CYAN, font=(MONO, 12, "bold"))
        # Compatibilidad con la API del minijuego original.
        self.task_items = []

    def build_chute(self):
        x1, y1, x2, y2 = CHUTE
        for x in range(x1, x2):
            v = int(205 - 65 * (x - x1) / (x2 - x1))
            self.create_line(x, y1, x, y2, fill=f"#{v:02x}{v:02x}{v:02x}")
        sx1, sy1, sx2, sy2 = SLOT
        self.create_rectangle(sx1 - 5, sy1 - 5, sx2 + 5, sy2 + 5,
                              fill="#626a70", outline="#24282e", width=2)
        self.slot_rect = self.create_rectangle(*SLOT, fill="#080d12",
                                                outline="#3f5963", width=3)
        self.chevrons = []
        for cy in (307, 380, 453):
            self.chevrons.append(self.create_line(SLOT_CX + 12, cy - 13,
                     SLOT_CX - 12, cy, SLOT_CX + 12, cy + 13,
                     fill=CHEV_OFF, width=4))
        for x in (x1 + 12, x2 - 12):
            for y in (y1 + 12, y2 - 12):
                self.create_oval(x - 4, y - 4, x + 4, y + 4,
                                 fill="#858b92", outline="#34383d")

    def build_frame(self):
        self.create_rectangle(PX1 - 4, PY1 - 4, PX2 + 4, PY2 + 4,
                              outline="#6b8192", width=3)
        self.create_line(FX1, FY1, FX1, FY2, fill="#283340", width=4)
        self.create_rectangle(290, 542, 610, 576, fill="#172638",
                              outline=EDGE, width=2)
        self.count_text = self.create_text(W / 2, 559,
                             text=f"HOJAS RESTANTES: {self.n_leaves}",
                             fill=WHITE, font=(MONO, 13, "bold"))
        self.create_text(W / 2, 592,
                         text="ARRASTRA LAS HOJAS AL CONDUCTO IZQUIERDO",
                         fill="#AFC5D6", font=(FONT, 9, "bold"))
        self.create_rectangle(205, 344, 695, 438, fill="#080D17",
                              outline=GREEN, width=3, tags="banner", state="hidden")
        self.banner_title = self.create_text(W / 2, 375,
                            text="FILTRO LIMPIO", fill=GREEN,
                            font=(FONT, 17, "bold"), tags="banner", state="hidden")
        self.banner_subtitle = self.create_text(W / 2, 413,
                            text="SISTEMA RESTABLECIDO", fill=WHITE,
                            font=(FONT, 11), tags="banner", state="hidden")

    def build_close(self):
        # No hay boton superpuesto al conducto; se cierra con la X de la ventana.
        pass

    def on_press(self, e):
        if self.phase != "playing":
            return
        # Misma seleccion de hojas que el codigo original, sin el antiguo
        # bloqueo circular de la esquina del conducto.
        l = self.leaf_at(e.x, e.y)
        if l is None:
            return
        self.drag = l
        self.grab = (e.x - l["x"], e.y - l["y"])
        self.leaves.remove(l)
        self.leaves.append(l)
        self.tag_raise(l["tag"])
        self.configure(cursor="fleur")

    def on_move(self, e):
        if self.phase == "playing":
            super().on_move(e)

    def on_release(self, e):
        if self.phase == "playing":
            super().on_release(e)

    def finish_leaf(self, l):
        self.delete(*l["items"])
        if l in self.leaves:
            self.leaves.remove(l)
        remaining = len(self.leaves)
        self.itemconfigure(self.count_text,
                           text=f"HOJAS RESTANTES: {remaining}")
        if remaining == 0 and self.phase == "playing":
            self.win()

    def win(self):
        self.phase = "transition"
        self.done = True
        self.itemconfigure("banner", state="normal")
        if self.level < len(LEVEL_NAMES) - 1:
            self.itemconfigure(self.banner_title, text="FILTRO LIMPIO")
            self.itemconfigure(self.banner_subtitle, text="SIGUIENTE SISTEMA")
            self.transition_job = self.after(900, self.next_level)
        else:
            self.phase = "completed"
            self.itemconfigure(self.banner_title, text="OXÍGENO RESTABLECIDO")
            self.itemconfigure(self.banner_subtitle, text="MANTENIMIENTO COMPLETADO")
            if self.on_success:
                self.after(900, self.on_success)

    def next_level(self):
        if self.phase == "timeout" or self.closing:
            return
        self.level += 1
        self.n_leaves = LEVEL_LEAVES[self.level]
        self.done = False
        self.phase = "playing"
        self.drag = None
        self.itemconfigure("banner", state="hidden")
        self.itemconfigure(self.level_text,
                           text=f"NIVEL {self.level + 1}/4 | {LEVEL_NAMES[self.level]}")
        self.itemconfigure(self.count_text,
                           text=f"HOJAS RESTANTES: {self.n_leaves}")
        self.build_leaves()

    def update_timer(self):
        if self.closing:
            return
        if self.phase not in ("completed", "timeout"):
            remaining = max(0, math.ceil(self.deadline - now()))
            mm, ss = divmod(remaining, 60)
            self.itemconfigure(self.timer_text, text=f"{mm:02d}:{ss:02d}",
                               fill=RED if remaining <= 10 else CYAN)
            if remaining == 0:
                self.phase = "timeout"
                self.done = True
                self.drag = None
                self.configure(cursor="")
                if self.transition_job:
                    self.after_cancel(self.transition_job)
                    self.transition_job = None
                self.itemconfigure("banner", state="normal")
                self.itemconfigure(self.banner_title, text="TIEMPO AGOTADO", fill=RED)
                self.itemconfigure(self.banner_subtitle,
                                   text="PRESIONA R PARA REINICIAR")
        self.timer_job = self.after(100, self.update_timer)

    def restart(self, event=None):
        if self.phase not in ("timeout", "completed"):
            return
        for l in list(self.leaves):
            self.delete(*l["items"])
        self.leaves.clear()
        self.level = 0
        self.n_leaves = LEVEL_LEAVES[0]
        self.deadline = now() + TOTAL_TIME
        self.done = False
        self.phase = "playing"
        self.drag = None
        self.itemconfigure("banner", state="hidden")
        self.itemconfigure(self.banner_title, fill=GREEN)
        self.itemconfigure(self.level_text, text="NIVEL 1/4 | FILTRO AUXILIAR")
        self.itemconfigure(self.count_text,
                           text=f"HOJAS RESTANTES: {self.n_leaves}")
        self.build_leaves()

    def close(self):
        self.closing = True
        if self.transition_job:
            self.after_cancel(self.transition_job)
        if self.on_close:
            self.on_close()


def open_o2_task(parent, on_success=None):
    win = tk.Toplevel(parent)
    win.title("Astral Echo | Limpiar filtro de O2")
    win.configure(bg=DARK)
    win.resizable(False, False)
    task = CleanFilterTask(win, on_success=on_success, on_close=win.destroy)
    task.pack()
    win.bind("<r>", task.restart)
    win.bind("<R>", task.restart)
    win.protocol("WM_DELETE_WINDOW", task.close)
    # Mostrar el minijuego aunque la ventana principal este oculta.
    win.update_idletasks()
    win.deiconify()
    win.lift()
    win.focus_force()
    win.grab_set()
    return task


def main():
    root = tk.Tk()
    root.title("Astral Echo | Limpiar filtro de O2")
    root.configure(bg=DARK)
    root.resizable(False, False)
    task = CleanFilterTask(root, on_success=lambda: print("Tarea completada"),
                           on_close=root.destroy)
    task.pack()
    root.bind("<r>", task.restart)
    root.bind("<R>", task.restart)
    root.protocol("WM_DELETE_WINDOW", task.close)
    root.mainloop()


if __name__ == "__main__":
    main()