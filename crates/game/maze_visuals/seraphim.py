"""Astral Echo - Nareth / Seraphim: prototipo 2D independiente.

Version con fondo de fantasia (santuario celestial). La logica del juego
es la misma; solo cambian background() e icon() (halo para que los
iconos se lean sobre el marmol claro)."""
import math
import random
import tkinter as tk
from collections import deque
from pathlib import Path

MAP = Path('/tmp/astral_echo_mazes/nareth.txt')
SIZE, CELL = 21, 34
WIDTH = HEIGHT = SIZE * CELL
BG, PANEL = '#0c0919', '#171126'
GOLD, VIOLET, CYAN = '#e7b96a', '#b98bff', '#74f0e5'
WHITE, RED, GREEN = '#f4edff', '#ff668f', '#65efb5'


def read_map():
    if not MAP.is_file():
        raise FileNotFoundError('Falta nareth.txt. Ejecuta: cargo run -p game --example planet_maze_preview')
    lines = MAP.read_text().splitlines()
    if len(lines) != SIZE or any(len(row) != SIZE for row in lines):
        raise ValueError('El mapa de Nareth debe medir 21x21')
    if sum(row.count('E') for row in lines) != 1 or sum(row.count('S') for row in lines) != 1:
        raise ValueError('El mapa necesita una entrada E y una salida S')
    return lines


def find(lines, symbol):
    return next(( (x,y) for y,row in enumerate(lines) for x,c in enumerate(row) if c==symbol), None)


def pathfind(lines, start, goal):
    q = deque([start]); prev = {start: None}
    while q:
        p = q.popleft()
        if p == goal:
            route=[]
            while p is not None:
                route.append(p); p=prev[p]
            return route[::-1]
        for dx,dy in ((1,0),(-1,0),(0,1),(0,-1)):
            x,y=p[0]+dx,p[1]+dy; n=(x,y)
            if 0<=x<SIZE and 0<=y<SIZE and lines[y][x]!='#' and n not in prev:
                prev[n]=p; q.append(n)
    return None


def icon(c, kind, cx, cy, radius=12, active=True, tag='icon'):
    """Una sola rutina dibuja los iconos tanto en el mapa como en la leyenda."""
    r=radius
    if tag=='game':
        # Medallon oscuro: mantiene los iconos legibles sobre el marmol claro.
        c.create_oval(cx-r-4,cy-r-4,cx+r+4,cy+r+4,fill='#1b1030',outline=GOLD,width=1,stipple='gray75',tags=tag)
    if kind in ('entrance','exit'):
        color=GREEN if kind=='entrance' else RED
        c.create_oval(cx-r,cy-r,cx+r,cy+r,fill=color,outline=WHITE,width=2,tags=tag)
        c.create_text(cx,cy,text='E' if kind=='entrance' else 'S',fill=BG,font=('DejaVu Sans',11,'bold'),tags=tag)
    elif kind=='fragment':
        c.create_polygon(cx,cy-r,cx+r*.7,cy,cx,cy+r,cx-r*.7,cy,fill=WHITE,outline=GOLD,width=2,tags=tag)
        c.create_oval(cx-3,cy-3,cx+3,cy+3,fill=CYAN,outline='',tags=tag)
    elif kind=='portal':
        c.create_oval(cx-r,cy-r,cx+r,cy+r,outline=GOLD,width=4,tags=tag)
        c.create_oval(cx-r+5,cy-r+5,cx+r-5,cy+r-5,fill=VIOLET if active else '#413453',outline=CYAN,tags=tag)
    elif kind=='ring':
        col=RED if active else '#5d5473'
        c.create_oval(cx-r,cy-r,cx+r,cy+r,outline=col,width=4,tags=tag)
        c.create_arc(cx-r+4,cy-r+4,cx+r-4,cy+r-4,start=40,extent=260,style='arc',outline=GOLD,width=2,tags=tag)
    elif kind=='ray':
        col=GOLD if active else '#625879'
        c.create_polygon(cx-3,cy-r,cx+5,cy-3,cx,cy-3,cx+4,cy+r,cx-7,cy+1,cx-2,cy+1,fill=col,outline=RED if active else '',tags=tag)
    elif kind=='guardian':
        c.create_oval(cx-r,cy-r,cx+r,cy+r,fill='#4b244d',outline=RED,width=2,tags=tag)
        c.create_polygon(cx-r+3,cy,cx,cy-r+3,cx+r-3,cy,cx,cy+r-3,fill=GOLD,outline=WHITE,tags=tag)
    elif kind=='checkpoint':
        c.create_oval(cx-r,cy-r,cx+r,cy+r,fill='#27416a' if not active else '#216c64',outline=CYAN,width=2,tags=tag)
        c.create_polygon(cx,cy-r+4,cx-r+5,cy+r-5,cx+r-5,cy+r-5,fill=WHITE,tags=tag)
    elif kind=='player':
        c.create_oval(cx-r,cy-r,cx+r,cy+r,fill='#0c2b38',outline=CYAN,width=3,tags=tag)
        c.create_polygon(cx,cy-r+3,cx-r+4,cy+r-4,cx,cy+2,cx+r-4,cy+r-4,fill=WHITE,outline=CYAN,tags=tag)


# ---------------------------------------------------------------
#  Fondo de fantasia: santuario celestial entre nubes
#  Caminos de marmol pálido, muros de roca violeta con nubes,
#  flores naranjas y sellos magicos (como la imagen de referencia).
# ---------------------------------------------------------------
SKY = '#2a1740'
MARBLE = ('#e9dde6', '#e3d4e2', '#efe4ea', '#ddcfe0')
MARBLE_PINK = '#f2d8e2'
MORTAR = '#9c86a8'
ROCK = ('#4a2a5e', '#5b3470', '#3b2350', '#6a3f7e')
BLOSSOM = ('#ff9a4d', '#ffb36b', '#ff7f3f', '#ffc48a')
SEAL = '#ff8a50'
RAIL = '#a23fbf'
CLOUD = '#fff4fb'
CLOUD_PINK = '#ffc9ec'
CLOUD_TEAL = '#7fc4cf'


_GRID = []


def is_wall(x, y):
    if 0 <= y < SIZE and 0 <= x < SIZE:
        return _GRID[y][x] == '#'
    return True


def cloud(c, cx, cy, s, rng, tint=CLOUD, stipple='gray50'):
    """Nube hecha de varios circulos semitransparentes."""
    for _ in range(rng.randint(3, 5)):
        ox, oy = rng.uniform(-s, s), rng.uniform(-s * .5, s * .5)
        r = rng.uniform(s * .5, s * .9)
        c.create_oval(cx + ox - r, cy + oy - r, cx + ox + r, cy + oy + r,
                      fill=tint, outline='', stipple=stipple)


def blossoms(c, cx, cy, rng):
    """Racimo de flores naranjas."""
    for _ in range(rng.randint(6, 9)):
        ox, oy = rng.uniform(-11, 11), rng.uniform(-11, 11)
        r = rng.uniform(2.5, 4.5)
        c.create_oval(cx + ox - r, cy + oy - r, cx + ox + r, cy + oy + r,
                      fill=rng.choice(BLOSSOM), outline='#b4501f')
        c.create_oval(cx + ox - 1, cy + oy - 1, cx + ox + 1, cy + oy + 1,
                      fill='#fff0d8', outline='')


def mandala(c, cx, cy, r, color, petals=8, width=2):
    """Sello magico: anillos, petalos y nucleo."""
    c.create_oval(cx - r, cy - r, cx + r, cy + r, outline=color, width=width)
    c.create_oval(cx - r * .55, cy - r * .55, cx + r * .55, cy + r * .55,
                  outline=color, width=1)
    for i in range(petals):
        a = i * 2 * math.pi / petals
        x1, y1 = cx + math.cos(a) * r * .55, cy + math.sin(a) * r * .55
        x2, y2 = cx + math.cos(a) * r * .95, cy + math.sin(a) * r * .95
        c.create_line(x1, y1, x2, y2, fill=color, width=1)
        c.create_oval(x2 - 1.5, y2 - 1.5, x2 + 1.5, y2 + 1.5, fill=color, outline='')
        b = a + math.pi / petals
        c.create_line(x1, y1, cx + math.cos(b) * r * .8, cy + math.sin(b) * r * .8,
                      fill=color)
    c.create_oval(cx - 2.5, cy - 2.5, cx + 2.5, cy + 2.5, fill=color, outline='')


def star4(c, cx, cy, r, color):
    c.create_polygon(cx, cy - r, cx + r * .3, cy - r * .3, cx + r, cy,
                     cx + r * .3, cy + r * .3, cx, cy + r, cx - r * .3, cy + r * .3,
                     cx - r, cy, cx - r * .3, cy - r * .3, fill=color, outline='')


def draw_marble(c, x, y, rng):
    a, b = x * CELL, y * CELL
    cx, cy = a + CELL / 2, b + CELL / 2
    base = MARBLE_PINK if rng.random() < .12 else rng.choice(MARBLE)
    c.create_rectangle(a, b, a + CELL, b + CELL, fill=base, outline='')

    # Ladrillos de marmol (dos hileras desfasadas)
    c.create_line(a, b + 17, a + CELL, b + 17, fill=MORTAR)
    if (x + y) % 2:
        c.create_line(a + 17, b, a + 17, b + 17, fill=MORTAR)
        c.create_line(a + 8, b + 17, a + 8, b + CELL, fill=MORTAR)
        c.create_line(a + 25, b + 17, a + 25, b + CELL, fill=MORTAR)
    else:
        c.create_line(a + 8, b, a + 8, b + 17, fill=MORTAR)
        c.create_line(a + 25, b, a + 25, b + 17, fill=MORTAR)
        c.create_line(a + 17, b + 17, a + 17, b + CELL, fill=MORTAR)
    c.create_line(a + 1, b + 1, a + CELL - 1, b + 1, fill='#ffffff')

    # Manchas suaves y grabados
    if rng.random() < .35:
        gx, gy = a + rng.randint(3, 18), b + rng.randint(3, 18)
        c.create_rectangle(gx, gy, gx + rng.randint(6, 12), gy + rng.randint(5, 10),
                           fill='#b79bc4', outline='', stipple='gray12')
    roll = rng.random()
    if roll < .08:
        c.create_polygon(cx, cy - 8, cx + 6, cy, cx, cy + 8, cx - 6, cy,
                         fill='', outline=SEAL, width=2)
        c.create_oval(cx - 1.5, cy - 1.5, cx + 1.5, cy + 1.5, fill=SEAL, outline='')
    elif roll < .18:
        star4(c, cx + rng.randint(-8, 8), cy + rng.randint(-8, 8), 3.5, GOLD)

    # Baranda violeta y sombra junto a los muros (como el puente de la referencia)
    rails = (((0, -1), (a, b + 1, a + CELL, b + 1), (a, b, a + CELL, b + 7)),
             ((0, 1), (a, b + CELL - 1, a + CELL, b + CELL - 1), (a, b + CELL - 7, a + CELL, b + CELL)),
             ((-1, 0), (a + 1, b, a + 1, b + CELL), (a, b, a + 7, b + CELL)),
             ((1, 0), (a + CELL - 1, b, a + CELL - 1, b + CELL), (a + CELL - 7, b, a + CELL, b + CELL)))
    for (dx, dy), line, shadow in rails:
        if is_wall(x + dx, y + dy):
            c.create_rectangle(*shadow, fill=SKY, outline='', stipple='gray50')
            c.create_line(*line, fill=RAIL, width=3)
            c.create_line(*line, fill='#ffb0e0', width=1)


def draw_rock(c, x, y, rng):
    a, b = x * CELL, y * CELL
    c.create_rectangle(a, b, a + CELL, b + CELL, fill=rng.choice(ROCK), outline='')
    # Vetas de roca claras y oscuras
    for _ in range(3):
        px, py = a + rng.randint(2, CELL - 12), b + rng.randint(2, CELL - 12)
        c.create_polygon(px, py + 4, px + 5, py, px + 12, py + 3, px + 9, py + 9,
                         px + 2, py + 8,
                         fill=rng.choice(('#7a4f8e', '#35204a', '#6a3f7e')), outline='')
    # Estrellitas
    if rng.random() < .25:
        c.create_oval(a + rng.randint(4, 30), b + rng.randint(4, 30),
                      a + rng.randint(31, 33), b + rng.randint(31, 33),
                      fill=CLOUD, outline='')


def wall_decor(c, x, y, rng):
    a, b = x * CELL, y * CELL
    cx, cy = a + CELL / 2, b + CELL / 2
    if (x * 7 + y * 11) % 9 == 0:
        # Sello magico sobre disco de marmol oscuro
        c.create_oval(cx - 14, cy - 14, cx + 14, cy + 14, fill='#2e1a42', outline='#8a6a9e', width=2)
        mandala(c, cx, cy, 11, SEAL, 8, 2)
    elif (x * 13 + y * 7) % 5 == 0:
        blossoms(c, cx, cy, rng)
    elif (x + 2 * y) % 6 == 0:
        # Roca flotante azulada
        c.create_polygon(cx - 11, cy + 3, cx - 6, cy - 9, cx + 4, cy - 10, cx + 12, cy - 2,
                         cx + 8, cy + 9, cx - 3, cy + 11, fill='#4d7f94', outline='#2c4c5e', width=2)
        c.create_polygon(cx - 6, cy - 9, cx + 4, cy - 10, cx + 1, cy - 1, fill='#7fc4cf', outline='')
    elif (x * 3 + y * 5) % 4 == 0:
        cloud(c, cx, cy, 11, rng, CLOUD_PINK if (x + y) % 2 else CLOUD, 'gray50')


def cloud_fringe(c, x, y, lines, rng):
    """Bordes esponjosos de nube donde el muro toca el camino."""
    a, b = x * CELL, y * CELL
    sides = (((0, -1), lambda t: (a + t, b + 1)),
             ((0, 1), lambda t: (a + t, b + CELL - 1)),
             ((-1, 0), lambda t: (a + 1, b + t)),
             ((1, 0), lambda t: (a + CELL - 1, b + t)))
    for (dx, dy), pos in sides:
        if not is_wall(x + dx, y + dy):
            for t in (5, 13, 21, 29):
                px, py = pos(t + rng.randint(-2, 2))
                r = rng.uniform(3, 5.5)
                c.create_oval(px - r, py - r, px + r, py + r,
                              fill=rng.choice((CLOUD, CLOUD_PINK, CLOUD)),
                              outline='', stipple='gray50')


def background(canvas, lines):
    global _GRID
    _GRID = lines
    rng = random.Random(9603)
    canvas.delete('all')
    canvas.create_rectangle(0, 0, WIDTH, HEIGHT, fill=SKY, outline='')

    # 1) Camino de marmol y base de roca
    for y, row in enumerate(lines):
        for x, tile in enumerate(row):
            if tile == '#':
                draw_rock(canvas, x, y, rng)
            else:
                draw_marble(canvas, x, y, rng)

    # 2) Decoracion de los muros y bordes de nube
    for y, row in enumerate(lines):
        for x, tile in enumerate(row):
            if tile == '#':
                wall_decor(canvas, x, y, rng)
                cloud_fringe(canvas, x, y, lines, rng)

    # 3) Marcos dorados y sellos magicos en los bordes del mapa
    for r, col in ((7, GOLD), (13, '#8a6a9e')):
        canvas.create_rectangle(r, r, WIDTH - r, HEIGHT - r, outline=col, width=2)
    for k in range(2, SIZE - 2, 4):
        for (cx, cy) in ((k * CELL + CELL / 2, CELL / 2), (k * CELL + CELL / 2, HEIGHT - CELL / 2),
                         (CELL / 2, k * CELL + CELL / 2), (WIDTH - CELL / 2, k * CELL + CELL / 2)):
            canvas.create_oval(cx - 13, cy - 13, cx + 13, cy + 13, fill='#2e1a42', outline=GOLD, width=2)
            mandala(canvas, cx, cy, 10, SEAL, 8, 2)

    # 4) Nubes grandes en las esquinas y velo de bruma en los bordes
    for (cx, cy) in ((20, 20), (WIDTH - 20, 20), (20, HEIGHT - 20), (WIDTH - 20, HEIGHT - 20)):
        cloud(canvas, cx, cy, 34, rng, CLOUD, 'gray75')
        cloud(canvas, cx, cy, 26, rng, CLOUD_PINK, 'gray50')
    for i, stp in enumerate(('gray50', 'gray25', 'gray12')):
        d = 20 * (i + 1)
        for box in ((0, 0, WIDTH, d), (0, HEIGHT - d, WIDTH, HEIGHT),
                    (0, 0, d, HEIGHT), (WIDTH - d, 0, WIDTH, HEIGHT)):
            canvas.create_rectangle(*box, fill='#2a0f3a', outline='', stipple=stp)


class SeraphimGame:
    def __init__(self,window,canvas,lines,status):
        self.window,self.canvas,self.lines,self.status=window,canvas,lines,status
        self.start=find(lines,'E'); self.goal=find(lines,'S')
        route=pathfind(lines,self.start,self.goal)
        if not route or len(route)<20:
            raise ValueError('Nareth necesita un camino valido con al menos 20 celdas')
        self.route=route
        self.place_objects()
        self.reset()
        self.window.bind('<KeyPress>',self.on_key)
        self.window.after(700,self.tick)

    def place_objects(self):
        route=self.route; n=len(route)-1
        self.fragments=[route[n//6],route[n//3],route[n//2]]
        self.portal=route[(n*5)//6]
        self.checkpoints={route[n//4],route[(n*3)//4]}
        reserved={self.start,self.goal,self.portal,*self.fragments,*self.checkpoints}
        # Distribucion por sectores, evitando superposiciones de objetos.
        rng=random.Random(9603)
        sectors=[[] for _ in range(9)]
        for y,row in enumerate(self.lines):
            for x,c in enumerate(row):
                p=(x,y)
                if c!='#' and p not in reserved:
                    sectors[min(2,y*3//SIZE)*3+min(2,x*3//SIZE)].append(p)
        for group in sectors: rng.shuffle(group)
        used=set(reserved)
        def choose(count):
            chosen=[]
            order=list(range(9)); rng.shuffle(order)
            while len(chosen)<count:
                progress=False
                for i in order:
                    while sectors[i] and sectors[i][-1] in used: sectors[i].pop()
                    if sectors[i]:
                        p=sectors[i].pop(); used.add(p); chosen.append(p); progress=True
                        if len(chosen)==count: break
                if not progress: raise ValueError('No hay suficientes casillas libres')
            return chosen
        self.rings=set(choose(7))
        self.rays=set(choose(6))
        starts=choose(3)
        self.patrols=[]
        for p in starts:
            neighbors=[]
            for dx,dy in ((1,0),(-1,0),(0,1),(0,-1)):
                x,y=p[0]+dx,p[1]+dy
                if 0<=x<SIZE and 0<=y<SIZE and self.lines[y][x]!='#' and (x,y) not in reserved and (x,y) not in self.rings and (x,y) not in self.rays:
                    neighbors.append((x,y))
            self.patrols.append([p,neighbors[0]] if neighbors else [p])

    def reset(self):
        self.player=self.start; self.steps=0; self.energy=100
        self.collected=set(); self.portal_open=False; self.completed=False
        self.checkpoint=self.start; self.visited=set()
        self.clock=0; self.rings_active=True; self.rays_active=True
        self.patrol_indices=[0]*len(self.patrols)
        self.draw(); self.message('REUNE LOS 3 FRAGMENTOS DE LUZ')

    def center(self,p):
        return (p[0]+.5)*CELL,(p[1]+.5)*CELL

    def draw(self):
        c=self.canvas; c.delete('game')
        icon(c,'entrance',*self.center(self.start),tag='game')
        icon(c,'exit',*self.center(self.goal),tag='game')
        for p in self.checkpoints:
            icon(c,'checkpoint',*self.center(p),active=p in self.visited,tag='game')
        for p in self.fragments:
            if p not in self.collected: icon(c,'fragment',*self.center(p),tag='game')
        icon(c,'portal',*self.center(self.portal),active=self.portal_open,tag='game')
        for p in self.rings: icon(c,'ring',*self.center(p),active=self.rings_active,tag='game')
        for p in self.rays: icon(c,'ray',*self.center(p),active=self.rays_active,tag='game')
        for route,i in zip(self.patrols,self.patrol_indices):
            icon(c,'guardian',*self.center(route[i]),tag='game')
        icon(c,'player',*self.center(self.player),tag='game')

    def message(self,msg):
        self.status.config(text=f'ENERGIA {self.energy}  |  PASOS {self.steps}  |  FRAGMENTOS {len(self.collected)}/3\n{msg}')

    def on_key(self,event):
        key=event.keysym.lower()
        if key=='r': self.reset(); return
        directions={'w':(0,-1),'up':(0,-1),'s':(0,1),'down':(0,1),'a':(-1,0),'left':(-1,0),'d':(1,0),'right':(1,0)}
        if key not in directions or self.completed: return
        dx,dy=directions[key]; x,y=self.player[0]+dx,self.player[1]+dy; p=(x,y)
        if not(0<=x<SIZE and 0<=y<SIZE) or self.lines[y][x]=='#':
            self.message('MURO CELESTIAL: PASO BLOQUEADO'); return
        if p==self.portal and len(self.collected)<3:
            self.message('PORTAL SELLADO: NECESITAS 3 FRAGMENTOS'); return
        if p in self.rings and self.rings_active:
            self.message('ANILLO ACTIVO: ESPERA'); return
        self.player=p; self.steps+=1; msg='EXPLORANDO NARETH'
        if p in self.fragments and p not in self.collected:
            self.collected.add(p); msg='FRAGMENTO DE LUZ RECUPERADO'
            if len(self.collected)==3:
                self.portal_open=True; msg='TRES FRAGMENTOS: PORTAL DESBLOQUEADO'
        if p in self.checkpoints:
            self.checkpoint=p; self.visited.add(p); msg='PUNTO DE CONTROL ACTIVADO'
        if p in self.rays and self.rays_active:
            self.energy-=25; msg='RAYO CELESTIAL: -25 ENERGIA'
        if any(p==route[i] for route,i in zip(self.patrols,self.patrol_indices)):
            self.energy-=30; msg='GUARDIAN: -30 ENERGIA'
        if self.energy<=0:
            self.energy=100; self.player=self.checkpoint
            msg='ENERGIA AGOTADA: REGRESAS AL CONTROL'
        if self.player==self.goal:
            if len(self.collected)==3:
                self.completed=True; msg='NARETH COMPLETADO - PULSA R PARA REINICIAR'
            else:
                msg='FALTAN FRAGMENTOS DE LUZ'
        self.draw(); self.message(msg)

    def tick(self):
        if not self.completed:
            self.clock+=1
            if self.clock%4==0: self.rings_active=not self.rings_active
            if self.clock%5==0: self.rays_active=not self.rays_active
            if self.clock%3==0:
                self.patrol_indices=[(i+1)%len(route) for i,route in zip(self.patrol_indices,self.patrols)]
            self.draw()
        self.window.after(700,self.tick)


def main():
    lines=read_map()
    window=tk.Tk(); window.title('Astral Echo | Nareth - Seraphim Planet')
    window.configure(bg=BG); window.resizable(False,False)
    PANEL_WIDTH=292; PAD=14; HEADER=82; FOOTER=96
    window.geometry(f'{WIDTH+PANEL_WIDTH+PAD*3}x{HEIGHT+HEADER+FOOTER}')
    header=tk.Frame(window,bg=BG,height=HEADER); header.pack(fill='x'); header.pack_propagate(False)
    tk.Label(header,text='NARETH | SERAPHIM PLANET',bg=BG,fg=GOLD,font=('DejaVu Sans',19,'bold')).pack(pady=(8,0))
    tk.Label(header,text='SANTUARIO CELESTIAL | ANILLOS Y LUZ',bg=BG,fg=VIOLET,font=('DejaVu Sans',10)).pack()
    body=tk.Frame(window,bg=BG); body.pack(fill='both',expand=True,padx=PAD)
    canvas=tk.Canvas(body,width=WIDTH,height=HEIGHT,bg=BG,highlightthickness=2,highlightbackground=GOLD)
    canvas.pack(side='left',anchor='n'); background(canvas,lines)
    slot=tk.Frame(body,bg=BG,width=PANEL_WIDTH,height=HEIGHT); slot.pack(side='left',fill='y',padx=(PAD,0)); slot.pack_propagate(False)
    panel=tk.Frame(slot,bg=PANEL); panel.pack(fill='both',expand=True)
    tk.Label(panel,text='INSTRUCCIONES',bg=PANEL,fg=GOLD,font=('DejaVu Sans',13,'bold')).pack(anchor='w',padx=12,pady=(12,8))
    legend=[
        ('entrance','ENTRADA','Comienzas aqui.'),('exit','SALIDA','Completa el santuario.'),
        ('fragment','FRAGMENTO','Recoge los 3 cristales de luz.'),('portal','PORTAL','Se abre al reunir los 3 fragmentos.'),
        ('ring','ANILLO','Activo: bloquea el paso. Espera.'),('ray','RAYO','Activo al entrar: -25 energia.'),
        ('guardian','GUARDIAN','Al entrar en su casilla: -30 energia.'),
        ('checkpoint','CONTROL','Guarda tu punto de reaparicion.')]
    for kind,title,description in legend:
        row=tk.Frame(panel,bg=PANEL); row.pack(fill='x',padx=6,pady=4)
        preview=tk.Canvas(row,width=40,height=40,bg=PANEL,highlightthickness=0); preview.pack(side='left',padx=(4,9))
        icon(preview,kind,20,20,tag='legend')
        info=tk.Frame(row,bg=PANEL); info.pack(side='left',fill='x',expand=True)
        tk.Label(info,text=title,bg=PANEL,fg=GOLD,font=('DejaVu Sans',9,'bold')).pack(anchor='w')
        tk.Label(info,text=description,bg=PANEL,fg=WHITE,font=('DejaVu Sans',9),wraplength=195,justify='left').pack(anchor='w')
    tk.Label(panel,text='WASD / FLECHAS: MOVER     R: REINICIAR',bg=PANEL,fg=CYAN,font=('DejaVu Sans',8,'bold'),wraplength=PANEL_WIDTH-20).pack(pady=(12,0))
    footer=tk.Frame(window,bg=BG,height=FOOTER); footer.pack(fill='x',padx=PAD); footer.pack_propagate(False)
    def toggle():
        if panel.winfo_manager():
            panel.pack_forget(); button.config(text='MOSTRAR INSTRUCCIONES')
        else:
            panel.pack(fill='both',expand=True); button.config(text='OCULTAR INSTRUCCIONES')
    button=tk.Button(footer,text='OCULTAR INSTRUCCIONES',command=toggle,bg='#3d2b52',fg=WHITE)
    button.pack(anchor='w',pady=(4,2))
    status=tk.Label(footer,text='',bg=BG,fg=WHITE,anchor='w',justify='left',height=2,wraplength=WIDTH+PANEL_WIDTH,font=('DejaVu Sans',10,'bold'))
    status.pack(fill='x')
    game=SeraphimGame(window,canvas,lines,status)
    window.mainloop()


if __name__=='__main__':
    main()