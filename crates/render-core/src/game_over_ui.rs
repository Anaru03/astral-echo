//! Original, renderer-independent Astral Echo defeat artwork.
//! The UI uses the existing framebuffer size and is shared by CPU and GPU.
use crate::hud::{glyph, HudOverlayRect};

type Rgba = [u8; 4];
const GOLD: Rgba = [237, 184, 95, 255];
const RED: Rgba = [255, 49, 105, 255];
const CYAN: Rgba = [75, 209, 226, 255];
const WHITE: Rgba = [230, 234, 245, 255];
const MUTED: Rgba = [148, 173, 196, 255];

struct Draw {
    width: u32,
    height: u32,
    scale: f32,
    ox: f32,
    oy: f32,
    shapes: Vec<HudOverlayRect>,
}
impl Draw {
    fn new(width: u32, height: u32) -> Self {
        let scale = (width as f32 / 800.0).min(height as f32 / 450.0);
        Self {
            width,
            height,
            scale,
            ox: (width as f32 - 800.0 * scale) / 2.0,
            oy: (height as f32 - 450.0 * scale) / 2.0,
            shapes: Vec::with_capacity(6000),
        }
    }
    fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, color: Rgba) {
        if w <= 0 || h <= 0 {
            return;
        }
        let xx = |v: i32| {
            (self.ox + v as f32 * self.scale)
                .round()
                .clamp(0.0, self.width as f32) as u32
        };
        let yy = |v: i32| {
            (self.oy + v as f32 * self.scale)
                .round()
                .clamp(0.0, self.height as f32) as u32
        };
        let (x0, x1, y0, y1) = (xx(x), xx(x + w), yy(y), yy(y + h));
        if x1 > x0 && y1 > y0 {
            self.shapes.push(HudOverlayRect {
                x: x0,
                y: y0,
                width: x1 - x0,
                height: y1 - y0,
                color,
            });
        }
    }
    fn line(&mut self, a: (i32, i32), b: (i32, i32), color: Rgba) {
        let n = (b.0 - a.0).abs().max((b.1 - a.1).abs()).max(1);
        for i in 0..=n {
            self.rect(
                a.0 + (b.0 - a.0) * i / n,
                a.1 + (b.1 - a.1) * i / n,
                2,
                2,
                color,
            );
        }
    }
    fn poly(&mut self, points: &[(i32, i32)], color: Rgba) {
        if points.len() < 3 {
            return;
        }
        let min = points.iter().map(|p| p.1).min().unwrap_or(0);
        let max = points.iter().map(|p| p.1).max().unwrap_or(0);
        for y in min..max {
            let mut intersections = Vec::new();
            for i in 0..points.len() {
                let (x1, y1) = points[i];
                let (x2, y2) = points[(i + 1) % points.len()];
                if (y1 <= y && y < y2) || (y2 <= y && y < y1) {
                    intersections.push(x1 + (x2 - x1) * (y - y1) / (y2 - y1));
                }
            }
            intersections.sort_unstable();
            for p in intersections.chunks_exact(2) {
                self.rect(p[0], y, p[1] - p[0], 1, color);
            }
        }
    }
    fn text(&mut self, msg: &str, cx: i32, y: i32, px: i32, color: Rgba) {
        let advance = 6 * px;
        let start = cx - (msg.chars().count() as i32 * advance - px) / 2;
        for (i, ch) in msg.chars().enumerate() {
            if let Some(rows) = glyph(ch.to_ascii_uppercase()) {
                for (ry, bits) in rows.iter().enumerate() {
                    for col in 0..5i32 {
                        if bits & (1u8 << (4 - col)) != 0 {
                            self.rect(
                                start + i as i32 * advance + col * px,
                                y + ry as i32 * px,
                                px,
                                px,
                                color,
                            );
                        }
                    }
                }
            }
        }
    }
}

/// One frame of original procedural artwork; no copied screenshots or RLE assets.
pub fn game_over_rects(width: u32, height: u32) -> Vec<HudOverlayRect> {
    if width == 0 || height == 0 {
        return Vec::new();
    }
    let mut d = Draw::new(width, height);
    // Deep-space background: opaque scanlines (works in both backends).
    d.rect(0, 0, 800, 450, [7, 9, 23, 255]);
    for y in (0..450).step_by(3) {
        let t = y as f32 / 450.0;
        let cloud = (1.0 - ((t - 0.60) * 2.4).abs()).max(0.0);
        d.rect(
            0,
            y,
            800,
            3,
            [
                (10.0 + 38.0 * cloud) as u8,
                (10.0 + 6.0 * t) as u8,
                (26.0 + 26.0 * t) as u8,
                255,
            ],
        );
    }
    // Layered soft-looking red cloud, kept behind the ship.
    for i in 0..44i32 {
        let dy = i - 22;
        let span = 300 - dy.abs() * 6;
        d.rect(
            400 - span,
            170 + i * 4,
            span * 2,
            3,
            [
                55 + (22 - dy.abs()) as u8,
                18,
                45 + (22 - dy.abs()) as u8,
                255,
            ],
        );
    }
    // Stars and glints.
    for i in 0..170i32 {
        let x = (i * 173 + 71).rem_euclid(800);
        let y = (i * 127 + 37).rem_euclid(450);
        let v = (125 + i * 17 % 100) as u8;
        d.rect(x, y, 1, 1, [v, v, 230, 255]);
        if i % 37 == 0 {
            d.line((x - 3, y), (x + 3, y), [112, 164, 191, 255]);
            d.line((x, y - 3), (x, y + 3), [112, 164, 191, 255]);
        }
    }
    // Angular cinematic framing.
    for (a, b) in [
        ((18, 27), (154, 27)),
        ((154, 27), (177, 14)),
        ((177, 14), (623, 14)),
        ((623, 14), (646, 27)),
        ((646, 27), (782, 27)),
        ((18, 422), (154, 422)),
        ((154, 422), (177, 436)),
        ((177, 436), (623, 436)),
        ((623, 436), (646, 422)),
        ((646, 422), (782, 422)),
    ] {
        d.line(a, b, GOLD);
    }
    d.rect(18, 27, 1, 395, [102, 68, 69, 255]);
    d.rect(781, 27, 1, 395, [102, 68, 69, 255]);
    // Header and typography (placed above the art, no overlap).
    d.text("AE-07 / PROTOCOLO DE EMERGENCIA", 400, 35, 1, MUTED);
    d.text("MISION FALLIDA", 400, 68, 4, [85, 18, 49, 255]);
    d.text("MISION FALLIDA", 400, 65, 4, RED);
    d.text("LA TRIPULACION NO PUEDE CONTINUAR", 400, 121, 1, WHITE);
    d.rect(183, 142, 434, 1, [114, 48, 67, 255]);
    // Explosion rings, sparks, and layered fiery core.
    for k in 0..120i32 {
        let a = k as f32 * std::f32::consts::TAU / 120.0;
        let x = 400 + (212.0 * a.cos()) as i32;
        let y = 265 + (52.0 * a.sin()) as i32;
        d.rect(x, y, 2, 1, [140, 61, 75, 255]);
    }
    for radius in (12..83).rev().step_by(4) {
        let r = radius as f32;
        let color = if radius > 54 {
            [93, 25, 51, 255]
        } else if radius > 34 {
            [166, 43, 45, 255]
        } else if radius > 20 {
            [226, 82, 42, 255]
        } else {
            [255, 177, 77, 255]
        };
        for y in (-radius..=radius).step_by(3) {
            let half = (r * r - (y * y) as f32).max(0.0).sqrt() as i32;
            d.rect(400 - half, 260 + y, 2 * half + 1, 3, color);
        }
    }
    // ECHO-7: original modular ship wreck, projected in three-quarter view.
    // Dark silhouettes first, then armour, recessed machinery, lights and damage.
    let shadow = [15, 23, 36, 255];
    let hull = [30, 43, 60, 255];
    let side = [45, 63, 82, 255];
    let plate = [77, 99, 117, 255];
    let edge = [122, 145, 157, 255];
    let soot = [47, 29, 38, 255];
    let ember = [255, 132, 67, 255];

    // Rear thruster module and engine exhausts (left wreck).
    d.poly(
        &[
            (167, 267),
            (192, 242),
            (248, 232),
            (346, 242),
            (374, 257),
            (344, 297),
            (232, 309),
            (184, 293),
        ],
        shadow,
    );
    d.poly(
        &[
            (184, 263),
            (210, 243),
            (322, 244),
            (358, 258),
            (330, 285),
            (215, 292),
        ],
        hull,
    );
    d.poly(
        &[
            (203, 253),
            (227, 241),
            (312, 245),
            (337, 258),
            (317, 275),
            (218, 279),
        ],
        plate,
    );
    d.poly(
        &[
            (198, 280),
            (223, 289),
            (330, 283),
            (342, 291),
            (309, 306),
            (221, 309),
        ],
        side,
    );
    // Longitudinal spine and inset channels.
    d.poly(
        &[(210, 255), (322, 252), (333, 258), (215, 265)],
        [35, 51, 67, 255],
    );
    d.line((219, 260), (318, 255), CYAN);
    d.line((231, 277), (324, 275), [35, 139, 156, 255]);
    // Rear engine nacelles: separate layered structures, not flat blocks.
    for (x, y) in [(185, 248), (187, 276)] {
        d.poly(
            &[
                (x - 20, y - 4),
                (x + 10, y - 8),
                (x + 25, y + 1),
                (x + 3, y + 10),
                (x - 20, y + 8),
            ],
            shadow,
        );
        d.poly(
            &[
                (x - 16, y - 2),
                (x + 8, y - 5),
                (x + 18, y + 1),
                (x + 1, y + 6),
                (x - 16, y + 5),
            ],
            side,
        );
        d.line((x - 15, y + 1), (x + 9, y - 2), [46, 169, 180, 255]);
        d.rect(x - 21, y, 5, 5, [160, 65, 55, 255]);
    }
    // Side wings and armour slabs on the rear section.
    d.poly(
        &[(241, 239), (275, 213), (329, 218), (341, 245), (311, 249)],
        shadow,
    );
    d.poly(
        &[(250, 237), (279, 220), (318, 224), (329, 239), (304, 244)],
        plate,
    );
    d.poly(
        &[(242, 291), (272, 319), (323, 317), (339, 291), (303, 282)],
        shadow,
    );
    d.poly(
        &[(252, 291), (276, 308), (313, 307), (326, 291), (303, 287)],
        side,
    );
    for k in 0..5 {
        let x = 267 + k * 10;
        d.line((x, 224), (x + 5, 239), if k % 2 == 0 { CYAN } else { edge });
        d.line((x, 294), (x + 4, 305), [31, 116, 133, 255]);
    }
    // Jagged break, burned edges and exposed interior of aft half.
    d.poly(
        &[
            (340, 241),
            (358, 248),
            (348, 258),
            (369, 265),
            (346, 277),
            (358, 287),
            (332, 291),
            (321, 275),
        ],
        soot,
    );
    for (a, b) in [
        ((346, 245), (357, 251)),
        ((348, 259), (361, 267)),
        ((344, 278), (354, 286)),
    ] {
        d.line(a, b, ember);
    }
    d.line((360, 259), (388, 242), [255, 173, 83, 255]);
    d.line((355, 276), (377, 299), [214, 83, 65, 255]);

    // Forward bridge and long pointed prow (right wreck), rotated away.
    d.poly(
        &[
            (429, 244),
            (457, 223),
            (506, 208),
            (569, 222),
            (646, 251),
            (667, 270),
            (629, 290),
            (538, 301),
            (468, 284),
            (437, 268),
        ],
        shadow,
    );
    d.poly(
        &[
            (440, 241),
            (468, 226),
            (509, 216),
            (570, 231),
            (632, 254),
            (648, 268),
            (619, 281),
            (534, 289),
            (466, 274),
        ],
        hull,
    );
    d.poly(
        &[
            (458, 238),
            (500, 220),
            (565, 234),
            (617, 253),
            (630, 264),
            (566, 270),
            (486, 259),
        ],
        plate,
    );
    d.poly(
        &[
            (476, 272),
            (538, 283),
            (614, 277),
            (626, 284),
            (540, 296),
            (477, 285),
        ],
        side,
    );
    // Twin raised superstructure housings with ventilated ribbing.
    d.poly(
        &[(485, 225), (501, 203), (540, 208), (551, 232), (529, 241)],
        shadow,
    );
    d.poly(
        &[(491, 223), (507, 210), (534, 214), (541, 230), (525, 234)],
        side,
    );
    d.poly(
        &[(547, 232), (565, 211), (606, 225), (617, 246), (592, 252)],
        shadow,
    );
    d.poly(
        &[(555, 232), (570, 219), (599, 229), (607, 244), (586, 247)],
        side,
    );
    for k in 0..5 {
        let dx = k * 7;
        d.line(
            (502 + dx, 215 + dx / 5),
            (506 + dx, 224 + dx / 5),
            [30, 129, 145, 255],
        );
        d.line(
            (571 + dx, 225 + dx / 5),
            (575 + dx, 237 + dx / 5),
            [30, 129, 145, 255],
        );
    }
    // Central flight deck, segmented armour and turquoise cockpit glass.
    d.poly(
        &[
            (491, 240),
            (520, 231),
            (577, 246),
            (594, 256),
            (556, 263),
            (506, 252),
        ],
        [24, 40, 56, 255],
    );
    d.poly(
        &[(515, 235), (550, 237), (576, 249), (547, 251)],
        [27, 178, 192, 255],
    );
    d.poly(
        &[(518, 236), (542, 238), (556, 244), (530, 244)],
        [113, 218, 219, 255],
    );
    for k in 0..4 {
        let x = 484 + k * 19;
        d.line((x, 258 + k / 2), (x + 12, 262 + k / 2), edge);
    }
    // Outer wing tips and interrupted luminous conduits.
    d.poly(
        &[(553, 268), (610, 265), (653, 275), (610, 290), (562, 286)],
        shadow,
    );
    d.poly(&[(565, 272), (612, 270), (638, 276), (604, 283)], side);
    d.line((577, 274), (620, 277), CYAN);
    d.poly(&[(462, 229), (482, 207), (508, 205), (498, 221)], side);
    d.line((469, 225), (493, 212), [37, 175, 184, 255]);
    // Torn connection: irregular split rather than a clean cut.
    d.poly(
        &[
            (431, 244),
            (448, 240),
            (441, 250),
            (453, 257),
            (438, 263),
            (447, 272),
            (425, 274),
            (419, 259),
        ],
        soot,
    );
    for (a, b) in [
        ((431, 244), (441, 250)),
        ((440, 262), (449, 268)),
        ((422, 271), (434, 276)),
    ] {
        d.line(a, b, ember);
    }
    d.line((426, 258), (400, 246), [255, 186, 93, 255]);
    d.line((433, 272), (409, 291), [243, 103, 66, 255]);

    // Dislodged armour and detached engine module, retaining ship detailing.
    d.poly(
        &[(244, 184), (274, 157), (307, 173), (316, 197), (279, 215)],
        shadow,
    );
    d.poly(
        &[(251, 184), (277, 165), (303, 177), (304, 192), (278, 206)],
        side,
    );
    d.poly(&[(265, 180), (284, 171), (300, 181), (284, 192)], plate);
    for k in 0..4 {
        d.line(
            (260 + k * 8, 188 - k * 3),
            (267 + k * 8, 195 - k * 3),
            if k == 1 { CYAN } else { edge },
        );
    }
    d.poly(
        &[(337, 318), (354, 305), (398, 310), (405, 341), (352, 350)],
        shadow,
    );
    d.poly(
        &[(345, 318), (360, 312), (393, 316), (396, 335), (355, 341)],
        side,
    );
    d.line((357, 323), (389, 326), [32, 171, 184, 255]);
    // Small irregular shrapnel with hot edges.
    for k in 0..16i32 {
        let a = k as f32 * std::f32::consts::TAU / 16.0;
        let r = 65.0 + (k * 29 % 75) as f32;
        let x = 400 + (a.cos() * r) as i32;
        let y = 265 + (a.sin() * r * 0.52) as i32;
        d.poly(
            &[
                (x, y),
                (x + 3 + k % 4, y - 2),
                (x + 5, y + 3),
                (x - 1, y + 2),
            ],
            if k % 3 == 0 {
                [115, 139, 151, 255]
            } else {
                soot
            },
        );
        d.line(
            (x, y),
            (x + 3, y - 2),
            if k % 2 == 0 { GOLD } else { ember },
        );
    }
    // Scattered sparks in front of the debris.
    for k in 0..22i32 {
        let a = k as f32 * std::f32::consts::TAU / 22.0;
        let r = 38.0 + (k * 23 % 115) as f32;
        let x = 400 + (a.cos() * r) as i32;
        let y = 265 + (a.sin() * r * 0.56) as i32;
        d.line(
            (x, y),
            (x + 3 + (k % 3), y - 1),
            if k % 3 == 0 { CYAN } else { GOLD },
        );
    }
    // Final status bar is separated from the explosion and footer.
    d.rect(145, 365, 510, 1, [105, 65, 78, 255]);
    d.text("ESTADO DE LA NAVE", 230, 377, 1, MUTED);
    d.text("SISTEMA CRITICO", 230, 393, 1, WHITE);
    d.text("ESTADO DE MISION", 570, 377, 1, MUTED);
    d.text("FINALIZADA", 570, 393, 1, GOLD);
    d.text("REGISTRO DE INCIDENTE / ECHO-7", 400, 416, 1, CYAN);
    d.shapes
}
