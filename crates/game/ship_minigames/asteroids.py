"""Astral Echo: defensa de asteroides. Python 3 + Tkinter."""
import tkinter as tk
import random
import math
import time

BG = '#0B101C'
PANEL = '#151D2B'
BORDER = '#40566B'
TEXT = '#E8F3FF'
CYAN = '#80D5E8'
GREEN = '#36F0A4'
YELLOW = '#FFC233'
RED = '#FF3548'
W, H = 900, 650
RX1, RY1, RX2, RY2 = 80, 70, 770, 390
LEVELS = [('ARMAMENTO AUXILIAR', 5), ('DEFENSA EXTERIOR', 7), ('ESCUDOS ORBITALES', 9), ('REACTOR PRINCIPAL', 12)]
TOTAL_TIME = 30

class AsteroidsTask(tk.Frame):
    def __init__(self, master, on_success=None, on_close=None, task_name='Armeria: Eliminar asteroides', progress=0.2, gain=0.2, target=None):
        super().__init__(master, bg=BG)
        self.master = master
        self.on_success = on_success
        self.on_close = on_close
        self.task_name = task_name
        self.progress = progress
        self.gain = gain
        self.target_override = target
        self.level = 0
        self.destroyed = 0
        self.rocks = []
        self.effects = []
        self.phase = 'playing'
        self.deadline = time.monotonic() + TOTAL_TIME
        self.last = time.monotonic()
        self.spawn_wait = 0.2
        self.pointer = None
        self.after_id = None
        self.generation = 0
        self.title = tk.Label(self, text='DEFENSA DE ASTEROIDES', bg=BG, fg=TEXT, font=('DejaVu Sans', 24, 'bold'))
        self.title.pack(pady=(14, 4))
        self.level_label = tk.Label(self, bg=BG, fg=YELLOW, font=('DejaVu Sans', 15, 'bold'))
        self.level_label.pack(pady=2)
        self.timer_label = tk.Label(self, bg=BG, fg=CYAN, font=('DejaVu Sans Mono', 18, 'bold'))
        self.timer_label.pack(pady=(2, 10))
        self.canvas = tk.Canvas(self, width=850, height=445, bg=PANEL, highlightbackground=BORDER, highlightthickness=2)
        self.canvas.pack(padx=18, pady=5)
        self.canvas.bind('<Motion>', self.motion)
        self.canvas.bind('<Leave>', self.leave)
        self.canvas.bind('<Button-1>', self.click)
        self.master.bind('<KeyPress-r>', self.restart)
        self.master.protocol('WM_DELETE_WINDOW', self.close)
        self.start_level()
        self.tick()

    def start_level(self):
        self.phase = 'playing'
        self.destroyed = 0
        self.rocks.clear()
        self.effects.clear()
        self.pointer = None
        self.spawn_wait = 0.15
        name, count = LEVELS[self.level]
        self.goal = self.target_override if self.target_override is not None else count
        self.level_label.config(text=f'NIVEL {self.level + 1}/4 | {name}')
        self.status = 'SISTEMA LISTO'
        self.draw()

    def draw(self):
        c = self.canvas
        c.delete('all')
        c.create_rectangle(20, 20, 830, 425, fill='#0D1421', outline=BORDER, width=2)
        c.create_text(425, 43, text='SISTEMA DE DEFENSA   //   AE-10', fill=CYAN, font=('DejaVu Sans Mono', 13, 'bold'))
        c.create_rectangle(RX1-5, RY1-5, RX2+5, RY2+5, fill='#071A12', outline='#739E87', width=3)
        c.create_rectangle(RX1, RY1, RX2, RY2, fill='#0B301C', outline='#286942', width=2)
        for x in range(RX1+25, RX2, 35):
            c.create_line(x, RY1+2, x, RY2-2, fill='#16482B')
        for y in range(RY1+25, RY2, 35):
            c.create_line(RX1+2, y, RX2-2, y, fill='#16482B')
        cx, cy = (RX1+RX2)/2, (RY1+RY2)/2
        for r in (45, 90, 135):
            c.create_oval(cx-r, cy-r, cx+r, cy+r, outline='#286942')
        c.create_line(RX1, cy, RX2, cy, fill='#357A4E')
        c.create_line(cx, RY1, cx, RY2, fill='#357A4E')
        c.create_rectangle(RX1+10, RY1+10, RX1+215, RY1+44, fill='#092419', outline='#367F55')
        c.create_text(RX1+22, RY1+27, text='RADAR ACTIVO', anchor='w', fill='#B7EAC4', font=('DejaVu Sans Mono', 12, 'bold'))
        c.create_rectangle(RX2-220, RY1+10, RX2-10, RY1+44, fill='#092419', outline='#367F55')
        c.create_text(RX2-115, RY1+27, text=f'OBJETIVOS: {self.destroyed}/{self.goal}', fill=GREEN, font=('DejaVu Sans Mono', 12, 'bold'), tags='counter')
        for rock in self.rocks:
            self.draw_rock(rock)
        for effect in self.effects:
            x, y, r, age = effect
            if age < 0.35:
                rad = r * (1 + age * 3)
                c.create_oval(x-rad, y-rad, x+rad, y+rad, outline=GREEN, width=2)
        if self.pointer and self.phase == 'playing':
            x, y = self.pointer
            c.create_oval(x-13, y-13, x+13, y+13, outline=TEXT, width=2)
            c.create_line(x-20, y, x-7, y, fill=TEXT, width=2)
            c.create_line(x+7, y, x+20, y, fill=TEXT, width=2)
            c.create_line(x, y-20, x, y-7, fill=TEXT, width=2)
            c.create_line(x, y+7, x, y+20, fill=TEXT, width=2)
        # Pie de panel: dos zonas independientes con margenes seguros.
        c.create_line(42, 399, 808, 399, fill=BORDER, width=1)
        c.create_text(48, 412, anchor='w',
                      text='HAZ CLIC EN LOS ASTEROIDES',
                      fill='#AFC5D6', font=('DejaVu Sans', 10, 'bold'))
        c.create_oval(593, 406, 605, 418,
                      fill=GREEN if self.phase == 'playing' else RED if self.phase == 'timeout' else YELLOW,
                      outline='')
        # El estado se acorta para no invadir el borde derecho.
        state_labels = {
            'SISTEMA LISTO': 'SISTEMA LISTO',
            'OBJETIVO ELIMINADO': 'OBJETIVO OK',
            'OBJETIVO NO DETECTADO': 'SIN IMPACTO',
            'SECTOR ASEGURADO': 'SECTOR LISTO',
            'MISIÓN COMPLETADA': 'COMPLETADO',
            'TIEMPO AGOTADO': 'SIN TIEMPO',
        }
        c.create_text(615, 412, anchor='w',
                      text=state_labels.get(self.status, self.status[:17]),
                      fill=CYAN, font=('DejaVu Sans Mono', 10, 'bold'))
        if self.phase in ('completed', 'timeout', 'transition'):
            color = GREEN if self.phase == 'completed' else RED if self.phase == 'timeout' else YELLOW
            label = 'DEFENSA COMPLETADA' if self.phase == 'completed' else 'TIEMPO AGOTADO' if self.phase == 'timeout' else 'NIVEL COMPLETADO'
            c.create_rectangle(195, 175, 655, 285, fill='#080D17', outline=color, width=4)
            c.create_text(425, 218, text=label, fill=color, font=('DejaVu Sans', 22, 'bold'))
            if self.phase in ('completed', 'timeout'):
                c.create_text(425, 256, text='PRESIONA R PARA REINICIAR', fill=TEXT, font=('DejaVu Sans', 11))

    def draw_rock(self, rock):
        x, y, r, angle, shape = rock['x'], rock['y'], rock['r'], rock['angle'], rock['shape']
        pts = []
        for i, scale in enumerate(shape):
            a = angle + 2*math.pi*i/len(shape)
            pts.extend((x+r*scale*math.cos(a), y+r*scale*math.sin(a)))
        self.canvas.create_polygon(*pts, fill='#16482B', outline='#55E784', width=2)
        self.canvas.create_oval(x-r*.28, y-r*.25, x+r*.15, y+r*.13, fill='#0C3721', outline='#31774A')

    def spawn(self):
        if len(self.rocks) >= 6:
            return
        r = random.randint(14, 25)
        side = random.randrange(4)
        if side == 0:
            x, y = RX1+r, random.uniform(RY1+r, RY2-r)
        elif side == 1:
            x, y = RX2-r, random.uniform(RY1+r, RY2-r)
        elif side == 2:
            x, y = random.uniform(RX1+r, RX2-r), RY1+r
        else:
            x, y = random.uniform(RX1+r, RX2-r), RY2-r
        tx, ty = random.uniform(RX1+110, RX2-110), random.uniform(RY1+70, RY2-70)
        angle = math.atan2(ty-y, tx-x)
        speed = random.uniform(45, 95) + self.level * 12
        self.rocks.append(dict(x=x, y=y, r=r, vx=math.cos(angle)*speed, vy=math.sin(angle)*speed,
                               angle=random.random()*math.tau, spin=random.uniform(-1.3, 1.3),
                               shape=[random.uniform(.76, 1.15) for _ in range(10)]))

    def motion(self, event):
        self.pointer = (event.x, event.y) if RX1 <= event.x <= RX2 and RY1 <= event.y <= RY2 else None

    def leave(self, _event):
        self.pointer = None

    def click(self, event):
        if self.phase != 'playing' or not (RX1 <= event.x <= RX2 and RY1 <= event.y <= RY2):
            return
        for rock in reversed(self.rocks):
            if math.hypot(event.x-rock['x'], event.y-rock['y']) <= rock['r']+9:
                self.rocks.remove(rock)
                self.effects.append((rock['x'], rock['y'], rock['r'], 0))
                self.destroyed += 1
                self.status = 'OBJETIVO ELIMINADO'
                if self.destroyed >= self.goal:
                    self.level_done()
                self.draw()
                return
        self.status = 'OBJETIVO NO DETECTADO'

    def level_done(self):
        if self.level == 3:
            self.phase = 'completed'
            self.status = 'MISIÓN COMPLETADA'
            if self.on_success:
                self.after(800, self.on_success)
        else:
            self.phase = 'transition'
            self.status = 'SECTOR ASEGURADO'
            self.generation += 1
            generation = self.generation
            def advance():
                if generation == self.generation and self.phase == 'transition':
                    if time.monotonic() >= self.deadline:
                        self.phase = 'timeout'
                    else:
                        self.level += 1
                        self.start_level()
            self.after(850, advance)

    def restart(self, _event=None):
        self.generation += 1
        self.level = 0
        self.deadline = time.monotonic() + TOTAL_TIME
        self.last = time.monotonic()
        self.start_level()

    def tick(self):
        t = time.monotonic()
        dt = min(t-self.last, 0.05)
        self.last = t
        remaining = max(0, math.ceil(self.deadline-t))
        self.timer_label.config(text=f'{remaining//60:02d}:{remaining%60:02d}', fg=RED if remaining <= 8 else CYAN)
        if self.phase in ('playing', 'transition') and t >= self.deadline:
            self.phase = 'timeout'
            self.status = 'TIEMPO AGOTADO'
            self.generation += 1
        if self.phase == 'playing':
            self.spawn_wait -= dt
            if self.spawn_wait <= 0:
                self.spawn()
                self.spawn_wait = random.uniform(.3, .75)
            for rock in list(self.rocks):
                rock['x'] += rock['vx']*dt
                rock['y'] += rock['vy']*dt
                rock['angle'] += rock['spin']*dt
                r = rock['r']
                if rock['x'] < RX1+r or rock['x'] > RX2-r:
                    rock['vx'] *= -1
                    rock['x'] = max(RX1+r, min(RX2-r, rock['x']))
                if rock['y'] < RY1+r or rock['y'] > RY2-r:
                    rock['vy'] *= -1
                    rock['y'] = max(RY1+r, min(RY2-r, rock['y']))
        self.effects = [(x,y,r,age+dt) for x,y,r,age in self.effects if age+dt < .35]
        self.draw()
        self.after_id = self.after(33, self.tick)

    def close(self):
        self.generation += 1
        if self.after_id is not None:
            self.after_cancel(self.after_id)
            self.after_id = None
        if self.on_close:
            self.on_close()
        else:
            self.master.destroy()


def open_asteroids_task(parent, on_success=None):
    win = tk.Toplevel(parent)
    win.title('Astral Echo | Eliminar asteroides')
    win.configure(bg=BG)
    win.resizable(False, False)
    task = AsteroidsTask(win, on_success=on_success, on_close=win.destroy)
    task.pack()
    win.transient(parent)
    win.grab_set()
    return task


def main():
    root = tk.Tk()
    root.title('Astral Echo | Eliminar asteroides')
    root.configure(bg=BG)
    root.resizable(False, False)
    AsteroidsTask(root, on_success=lambda: print('Tarea completada')).pack()
    root.mainloop()

if __name__ == '__main__':
    main()