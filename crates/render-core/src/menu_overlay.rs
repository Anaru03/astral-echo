#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuScreen {
    MainMenu,
    Controls,
    GameOver,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MenuOverlay {
    pub screen: MenuScreen,
    pub selected: usize,
    pub title: String,
    pub subtitle: String,
    pub options: Vec<String>,
}

impl MenuOverlay {
    pub fn main_menu(selected: usize) -> Self {
        Self {
            screen: MenuScreen::MainMenu,
            selected,
            title: "ASTRAL ECHO".to_owned(),
            subtitle: "ECHO-7 // SISTEMA DE NAVEGACION".to_owned(),
            options: vec![
                "INICIAR PARTIDA".to_owned(),
                "CONTROLES".to_owned(),
                "SALIR".to_owned(),
            ],
        }
    }

    pub fn controls() -> Self {
        Self {
            screen: MenuScreen::Controls,
            selected: 0,
            title: "CONTROLES".to_owned(),
            subtitle: "SISTEMAS DE ECHO-7".to_owned(),
            options: vec![
                "WASD - MOVER".to_owned(),
                "FLECHAS - MIRAR".to_owned(),
                "I - INTERACTUAR".to_owned(),
                "M - INICIAR MISION".to_owned(),
                "ESC - REGRESAR".to_owned(),
            ],
        }
    }

    pub fn game_over(selected: usize, ship_destroyed: bool) -> Self {
        Self {
            screen: MenuScreen::GameOver,
            selected,
            title: "MISION FALLIDA".to_owned(),
            subtitle: if ship_destroyed {
                "LA NAVE ECHO-7 HA SIDO DESTRUIDA"
            } else {
                "LA TRIPULACION NO PUEDE CONTINUAR"
            }
            .to_owned(),
            options: vec![
                "REINTENTAR".to_owned(),
                "MENU PRINCIPAL".to_owned(),
            ],
        }
    }
}

// Lightweight 2D menu. All coordinates are in a 1000x600 reference space.
// Uses the existing CPU/GPU HUD drawing path; no additional textures or models.
pub fn draw_menu(menu: &MenuOverlay, width: u32, height: u32) -> crate::hud::HudOverlay {
    use crate::hud::{HudOverlay, HudOverlayRect};
    let mut glyphs = Vec::with_capacity(2800);
    let sx = |v: i32| (v.max(0) as u64 * width as u64 / 1000) as u32;
    let sy = |v: i32| (v.max(0) as u64 * height as u64 / 600) as u32;
    let mut rect = |x: i32, y: i32, w: i32, h: i32, color: [u8;4]| {
        if w <= 0 || h <= 0 { return; }
        glyphs.push(HudOverlayRect {
            x: sx(x), y: sy(y),
            width: sx(x+w).saturating_sub(sx(x)).max(1),
            height: sy(y+h).saturating_sub(sy(y)).max(1), color,
        });
    };
    let gold = [240, 198, 74, 255];
    let white = [235, 240, 251, 255];
    let cyan = [85, 219, 231, 255];
    let muted = [153, 177, 199, 255];
    let accent = if menu.screen == MenuScreen::GameOver { [255, 87, 108, 255] } else { gold };

    // Full-bleed dark nebula, with restrained right-side light and no opaque shapes behind text.
    for y in (0..600).step_by(6) {
        let t = y as f32 / 600.0;
        let r = (13.0 + 13.0 * (1.0-(t-0.6).abs())).round() as u8;
        let g = (18.0 + 13.0 * (1.0-t)).round() as u8;
        let b = (37.0 + 20.0 * (1.0-t)).round() as u8;
        rect(0,y,1000,6,[r,g,b,255]);
    }
    // Nebula streaks stay entirely on the right, away from menu typography.
    for i in 0..28i32 {
        let y = 130+i*10;
        let radius = 14-(i-14).abs();
        rect(590-radius*3,y,390+radius*5,5,[35,59,100,55]);
    }
    for i in 0..115i32 {
        let x = (i*7919+53).rem_euclid(1000);
        let y = (i*2971+71).rem_euclid(600);
        let light = if i%17==0 { 230 } else { 118+(i*11%74) as u8 };
        rect(x,y,if i%17==0 {2} else {1},if i%17==0 {2} else {1},[light,light,235,255]);
    }
    // Fine angular borders: upper and lower bands.
    for (x,y,w) in [(0,50,112),(155,24,330),(530,50,470),(0,550,112),(155,576,330),(530,550,470)] {
        rect(x,y,w,2,gold);
    }
    for i in 0..44 {
        rect(112+i,50-i*26/44,2,2,gold);
        rect(485+i,24+i*26/44,2,2,gold);
        rect(112+i,550+i*26/44,2,2,gold);
        rect(485+i,576-i*26/44,2,2,gold);
    }
    fn text(draw: &mut impl FnMut(i32,i32,i32,i32,[u8;4]), value: &str,
            x: i32, y: i32, scale: i32, color: [u8;4]) {
        for (i,ch) in value.chars().enumerate() {
            if let Some(rows) = crate::hud::menu_glyph(ch) {
                for (ry,bits) in rows.iter().enumerate() {
                    for cx in 0..5i32 {
                        if bits & (1 << (4-cx)) != 0 {
                            draw(x+i as i32*6*scale+cx*scale,y+ry as i32*scale,scale,scale,color);
                        }
                    }
                }
            }
        }
    }
    // Ship illustration stays within the right 40% of the screen.
    // Narrow horizontal scanlines avoid large polygons crossing the menu.
    fn poly(draw: &mut impl FnMut(i32,i32,i32,i32,[u8;4]), pts: &[(i32,i32)], color:[u8;4]) {
        if pts.len()<3 {return;}
        let miny=pts.iter().map(|p|p.1).min().unwrap_or(0);
        let maxy=pts.iter().map(|p|p.1).max().unwrap_or(0);
        for y in miny..maxy {
            let mut xs=Vec::with_capacity(8);
            for i in 0..pts.len() {
                let (x1,y1)=pts[i]; let (x2,y2)=pts[(i+1)%pts.len()];
                if (y1<=y && y<y2)||(y2<=y && y<y1) {
                    xs.push(x1+(x2-x1)*(y-y1)/(y2-y1));
                }
            }
            xs.sort_unstable();
            for pair in xs.chunks_exact(2) {draw(pair[0],y,pair[1]-pair[0],1,color);}
        }
    }
    // Distant halo behind the craft, confined to right-side.
    for i in 0..18i32 {
        rect(630+i*4,182+i*7,290-i*5,2,[49,92,127,80]);
    }
    poly(&mut rect,&[(632,274),(729,209),(946,287),(978,332),(851,385),(719,351)], [26,46,66,255]);
    poly(&mut rect,&[(659,269),(738,225),(935,294),(949,320),(814,325)], [104,137,153,255]);
    poly(&mut rect,&[(728,320),(822,355),(912,461),(848,431),(752,365)], [35,64,84,255]);
    poly(&mut rect,&[(732,230),(842,256),(896,296),(770,278)], [14,38,57,255]);
    poly(&mut rect,&[(763,245),(838,264),(861,280),(778,269)], [39,180,200,255]);
    poly(&mut rect,&[(860,352),(942,366),(979,389),(886,381)], [26,194,209,255]);
    for i in 0..8 {rect(725+i*18,280+i*5,10,2,[187,215,221,255]);}
    rect(799,310,58,2,gold);

    // Left-aligned identity with generous spacing and no boxes behind buttons.
    let heading = if menu.screen==MenuScreen::MainMenu {"ASTRAL ECHO"} else {&menu.title};
    let heading_scale = if heading.chars().count()>13 {4} else {6};
    text(&mut rect,heading,110,126,heading_scale,accent);
    rect(112,186,380,2,[165,135,65,255]);
    let section = match menu.screen {
        MenuScreen::MainMenu => "MENU PRINCIPAL",
        MenuScreen::Controls => "CONTROLES",
        MenuScreen::GameOver => "FIN DE MISION",
    };
    text(&mut rect,section,157,213,3,white);
    if menu.screen == MenuScreen::Controls {
        for (i,line) in menu.options.iter().enumerate() {
            text(&mut rect,line,130,295+i as i32*39,2,if i%2==0 {white} else {muted});
        }
    } else {
        if menu.screen==MenuScreen::GameOver {
            text(&mut rect,&menu.subtitle,118,273,2,muted);
        }
        for (i,line) in menu.options.iter().enumerate() {
            let y=305+i as i32*57;
            let selected=i==menu.selected;
            if selected {
                rect(115,y+9,13,13,accent);
                rect(146,y+37,320,2,[150,122,65,255]);
            }
            text(&mut rect,line,151,y,3,if selected {accent} else {white});
        }
    }
    text(&mut rect,if menu.screen==MenuScreen::Controls {"ESC - REGRESAR"} else {"FLECHAS - ELEGIR  ENTER - CONFIRMAR"},116,517,1,cyan);
    HudOverlay {
        background: HudOverlayRect {x:0,y:0,width:0,height:0,color:[0;4]},
        glyphs, contextual_button:None, controls_button:None, controls_close_button:None,
    }
}
