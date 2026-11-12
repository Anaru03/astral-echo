import tkinter as tk
import random
import time
import math

BG = '#0B101C'
PANEL = '#151D2B'
BORDER = '#40566B'
TEXT = '#E8F3FF'
CYAN = '#80D5E8'
GREEN = '#36F0A4'
RED = '#FF3548'
YELLOW = '#FFC233'
WIDTH, HEIGHT = 900, 650
TOTAL_TIME = 30
TRANSITION_MS = 1200
LEVELS = [('ADMINISTRACION', 4), ('COMUNICACIONES', 5), ('NAVEGACION', 6), ('REACTOR PRINCIPAL', 7)]

class CodeGame:
    def __init__(self, root):
        self.root = root
        root.title('Astral Echo | Ingresar codigo')
        root.geometry(f'{WIDTH}x{HEIGHT}')
        root.resizable(False, False)
        root.configure(bg=BG)
        self.level = 0
        self.phase = 'playing'
        self.generation = 0
        self.deadline = time.monotonic() + TOTAL_TIME
        self.secret = ''
        self.used_codes = set()
        self.entered = ''
        self.title = tk.Label(root, text='VALIDACION DE CODIGO', font=('DejaVu Sans', 23, 'bold'), fg=TEXT, bg=BG)
        self.title.pack(pady=(17, 6))
        self.level_label = tk.Label(root, text='', font=('DejaVu Sans', 14, 'bold'), fg=YELLOW, bg=BG)
        self.level_label.pack(pady=3)
        self.timer_label = tk.Label(root, text='00:30', font=('DejaVu Sans', 17, 'bold'), fg=CYAN, bg=BG)
        self.timer_label.pack(pady=(3, 8))
        self.canvas = tk.Canvas(root, width=850, height=455, bg=PANEL, highlightbackground=BORDER, highlightthickness=2)
        self.canvas.pack(pady=6)
        self.canvas.bind('<Button-1>', self.click)
        root.bind('<Key>', self.keypress)
        self.start_level()
        self.update_timer()

    def start_level(self):
        self.phase = 'playing'
        self.entered = ''
        name, digits = LEVELS[self.level]
        # Digitos distintos dentro de cada codigo y codigos nuevos en cada intento.
        while True:
            candidate = ''.join(random.sample('0123456789', digits))
            if candidate not in self.used_codes:
                self.secret = candidate
                self.used_codes.add(candidate)
                break
        self.level_label.config(text=f'NIVEL {self.level + 1}/4 | {name}')
        self.draw()

    def rounded(self, x1, y1, x2, y2, radius, **kwargs):
        p = [x1+radius,y1,x2-radius,y1,x2,y1,x2,y1+radius,x2,y2-radius,x2,y2,x2-radius,y2,x1+radius,y2,x1,y2,x1,y2-radius,x1,y1+radius,x1,y1]
        return self.canvas.create_polygon(p, smooth=True, **kwargs)

    def draw(self):
        c = self.canvas
        c.delete('all')
        # Marco interior: margenes iguales y ningun texto toca el borde.
        c.create_rectangle(22, 22, 828, 433, fill='#0D1421', outline=BORDER, width=2)
        for x in range(42, 826, 28):
            c.create_line(x, 30, x, 425, fill='#172333')
        c.create_text(425, 47, text='TERMINAL DE AUTENTICACION  //  AE-09',
                      fill=CYAN, font=('DejaVu Sans Mono', 11, 'bold'))

        # Portanotas y papel: todo el contenido queda dentro de sus limites.
        self.rounded(64, 84, 339, 396, 11, fill='#273747', outline='#698093', width=3)
        c.create_text(201, 110, text='NOTA DE SEGURIDAD', fill=TEXT,
                      font=('DejaVu Sans', 12, 'bold'))
        self.rounded(88, 136, 315, 368, 5, fill='#FFE27C', outline='#D3A83D', width=2)
        c.create_polygon(285, 136, 315, 136, 315, 166, fill='#E8C65E', outline='')
        c.create_text(201, 183, text='CLAVE DE ACCESO', fill='#55411F',
                      font=('DejaVu Sans', 11, 'bold'))
        # El codigo es mas pequeno cuando tiene 6-7 digitos.
        size = 29 if len(self.secret) <= 5 else 24 if len(self.secret) == 6 else 21
        c.create_text(201, 237, text=self.secret, fill='#30271D',
                      font=('DejaVu Sans Mono', size, 'bold'))
        c.create_line(112, 274, 291, 274, fill='#C59B3E', width=2)
        c.create_text(201, 304, text='INTRODUCE LA CLAVE', fill='#594522',
                      font=('DejaVu Sans', 10, 'bold'))
        c.create_text(201, 326, text='EN EL PANEL DIGITAL', fill='#594522',
                      font=('DejaVu Sans', 10))
        # Pequena chincheta decorativa.
        c.create_oval(193, 123, 209, 139, fill='#C58F3A', outline='#8B6729', width=2)

        # Consola: volumen, tornillos, pantalla y separadores.
        self.rounded(377, 77, 779, 400, 14, fill='#070C14', outline='#8195A7', width=3)
        self.rounded(388, 88, 768, 389, 10, fill='#344252', outline='#17232E', width=2)
        c.create_line(398, 94, 758, 94, fill='#6B7E8C', width=2)
        for sx, sy in ((399, 99), (757, 99), (399, 377), (757, 377)):
            c.create_oval(sx-3, sy-3, sx+3, sy+3, fill='#182733', outline='#718493')
        self.rounded(413, 108, 743, 169, 5, fill='#071A1B', outline='#83B0AD', width=3)
        display = self.entered + '·' * (len(self.secret) - len(self.entered))
        c.create_text(578, 138, text=display, fill=GREEN,
                      font=('DejaVu Sans Mono', 25 if len(self.secret) < 7 else 22, 'bold'))
        c.create_text(578, 187, text=f'INGRESA {len(self.secret)} DIGITOS', fill='#B6CBD8',
                      font=('DejaVu Sans', 10, 'bold'))

        # Teclas estilo consola: sombra, bisel, iluminacion y etiquetas.
        self.buttons = []
        rows = [('1', '2', '3'), ('4', '5', '6'), ('7', '8', '9'), ('X', '0', 'OK')]
        for r, row in enumerate(rows):
            for col, key in enumerate(row):
                x1 = 412 + col * 111
                y1 = 209 + r * 43
                x2, y2 = x1 + 104, y1 + 37
                special = key in ('X', 'OK')
                base = '#B93D48' if key == 'X' else '#188C6D' if key == 'OK' else '#B7C8D8'
                edge = '#6F1C2B' if key == 'X' else '#0B5547' if key == 'OK' else '#647C91'
                shine = '#E36C73' if key == 'X' else '#3AC8A0' if key == 'OK' else '#EAF4FC'
                self.rounded(x1+2, y1+5, x2+2, y2+5, 7, fill='#071019', outline='')
                self.rounded(x1, y1, x2, y2, 7, fill=edge, outline='#0B1520', width=2)
                self.rounded(x1+3, y1+2, x2-3, y2-4, 5, fill=base, outline='')
                c.create_line(x1+12, y1+5, x2-12, y1+5, fill=shine, width=2)
                c.create_text((x1+x2)/2, (y1+y2)/2-1, text=key,
                              fill='#FFFFFF' if special else '#122337',
                              font=('DejaVu Sans', 17, 'bold'))
                self.buttons.append((x1, y1, x2, y2, key))

        # Solo un indicador de estado discreto, sin leyenda de teclado.
        c.create_oval(401, 412, 409, 420, fill=GREEN, outline='')
        c.create_text(420, 416, text='SISTEMA LISTO', anchor='w', fill='#9BB3C4',
                      font=('DejaVu Sans Mono', 9, 'bold'))
        c.create_text(806, 416, text='AE-09', anchor='e', fill='#6D879B',
                      font=('DejaVu Sans Mono', 9))

    def click(self, event):
        if self.phase != 'playing':
            return
        for x1,y1,x2,y2,key in self.buttons:
            if x1 <= event.x <= x2 and y1 <= event.y <= y2:
                self.press(key)
                break

    def keypress(self, event):
        if self.phase == 'timeout' and event.keysym.lower() == 'r':
            self.new_game()
            return
        if self.phase != 'playing':
            return
        if event.char in '0123456789' and event.char:
            self.press(event.char)
        elif event.keysym in ('BackSpace','Delete'):
            self.press('X')
        elif event.keysym in ('Return','KP_Enter'):
            self.press('OK')

    def press(self, key):
        if time.monotonic() >= self.deadline:
            self.time_out()
            return
        if key == 'X':
            self.entered = self.entered[:-1]
        elif key == 'OK':
            if len(self.entered) != len(self.secret):
                return
            if self.entered == self.secret:
                if self.level == len(LEVELS)-1:
                    self.complete()
                else:
                    self.show_message('CODIGO CORRECTO', GREEN, self.next_level)
            else:
                self.show_message('CODIGO INCORRECTO', RED, self.restart)
            return
        elif len(self.entered) < len(self.secret):
            self.entered += key
        self.draw()

    def show_message(self, message, color, callback):
        self.phase = 'transition'
        self.draw()
        self.canvas.create_rectangle(180, 163, 670, 284, fill='#080D17', outline=color, width=4)
        self.canvas.create_text(425, 222, text=message, fill=color, font=('DejaVu Sans', 25, 'bold'))
        self.generation += 1
        current = self.generation
        def finish():
            if current == self.generation:
                callback()
        self.root.after(TRANSITION_MS, finish)

    def next_level(self):
        self.level += 1
        self.start_level()

    def restart(self):
        self.generation += 1
        self.level = 0
        # No se modifica deadline: el reloj sigue corriendo tras fallar.
        if time.monotonic() >= self.deadline:
            self.time_out()
        else:
            self.start_level()

    def new_game(self):
        # Solo tras agotar el tiempo, R inicia una partida nueva.
        self.generation += 1
        self.level = 0
        self.used_codes.clear()
        self.deadline = time.monotonic() + TOTAL_TIME
        self.start_level()

    def complete(self):
        self.phase = 'completed'
        self.generation += 1
        self.draw()
        self.canvas.create_rectangle(115, 160, 735, 285, fill='#080D17', outline=GREEN, width=4)
        self.canvas.create_text(425, 220, text='ACCESO AUTORIZADO', fill=GREEN, font=('DejaVu Sans', 25, 'bold'))

    def time_out(self):
        if self.phase in ('completed','timeout'):
            return
        self.phase = 'timeout'
        self.generation += 1
        self.draw()
        self.canvas.create_rectangle(150, 153, 700, 295, fill='#080D17', outline=RED, width=4)
        self.canvas.create_text(425, 205, text='TIEMPO AGOTADO', fill=RED, font=('DejaVu Sans', 25, 'bold'))
        self.canvas.create_text(425, 257, text='PRESIONA R PARA VOLVER A JUGAR', fill=TEXT, font=('DejaVu Sans', 12, 'bold'))
        self.timer_label.config(text='00:00', fg=RED)

    def update_timer(self):
        if self.phase not in ('completed','timeout'):
            remaining = max(0, math.ceil(self.deadline-time.monotonic()))
            mm,ss = divmod(remaining,60)
            self.timer_label.config(text=f'{mm:02d}:{ss:02d}',fg=RED if remaining<=20 else CYAN)
            if remaining == 0:
                self.time_out()
        self.root.after(100,self.update_timer)


def main():
    root=tk.Tk()
    CodeGame(root)
    root.mainloop()

if __name__ == '__main__':
    main()