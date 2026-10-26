use glam::Vec4;

use crate::clip::ClipVertex;
use crate::color::vec4_to_rgba8;
use crate::error::RasterError;
use crate::framebuffer::{Color, Framebuffer};
use crate::viewport::Viewport;

/// Vértice en espacio de pantalla.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenVertex {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub inv_w: f32,
    pub color: Vec4,
}

/// Regla de relleno de triángulos.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FillRule {
    /// Incluye el borde superior e izquierdo (comportamiento estándar D3D/wgpu).
    TopLeft,
    /// Incluye todos los bordes.
    All,
}

/// Orientación del triángulo en pantalla.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Facing {
    Front,
    Back,
}

/// Fragmento generado por la rasterización.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fragment {
    pub x: u32,
    pub y: u32,
    pub depth: f32,
    pub color: Vec4,
}

/// Efecto ejecutado por la etapa de fragmentos del rasterizador CPU.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FragmentShader {
    /// Conserva el color interpolado de los vértices.
    VertexColor,
    /// Aplica bandas de iluminación deterministas en espacio de pantalla.
    SolarBands,
}

/// Resultado de una llamada de rasterización.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RasterOutput {
    pub fragments: u32,
    pub shaded: u32,
    pub depth_rejected: u32,
    pub written: u32,
}

/// Convierte clip space a espacio de pantalla.
pub fn to_screen(v: &ClipVertex, viewport: &Viewport) -> Option<ScreenVertex> {
    if v.clip.w.abs() < f32::EPSILON {
        return None;
    }
    let inv_w = 1.0 / v.clip.w;
    let ndc_x = v.clip.x * inv_w;
    let ndc_y = v.clip.y * inv_w;
    let ndc_z = v.clip.z * inv_w;
    if !ndc_x.is_finite() || !ndc_y.is_finite() || !ndc_z.is_finite() {
        return None;
    }
    let (x, y) = viewport.ndc_to_pixel(ndc_x, ndc_y);
    if !x.is_finite() || !y.is_finite() {
        return None;
    }
    Some(ScreenVertex {
        x,
        y,
        z: ndc_z.clamp(0.0, 1.0),
        inv_w,
        color: v.color,
    })
}

/// Función de arista: area con signo del triángulo `(a, b, c)` evaluada en `p`.
pub fn edge_function(a: &ScreenVertex, b: &ScreenVertex, c: &ScreenVertex) -> f32 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

pub fn is_top_left(from: &ScreenVertex, to: &ScreenVertex) -> bool {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    if dy.abs() <= f32::EPSILON {
        // Arista horizontal: es "top" si avanza hacia la izquierda.
        return dx < 0.0;
    }
    // Arista no horizontal: es "left" si desciende en pantalla.
    dy > 0.0
}

/// Rasteriza un triángulo en pantalla y produce sus fragmentos.
///
/// Los fragmentos se acumulan en `out`; el test de profundidad y la escritura
/// en el framebuffer se realizan en etapas separadas.
pub fn raster_triangle(
    a: &ScreenVertex,
    b: &ScreenVertex,
    c: &ScreenVertex,
    framebuffer_size: (u32, u32),
    rule: FillRule,
    out: &mut Vec<Fragment>,
) -> u32 {
    raster_triangle_in_rect(
        a,
        b,
        c,
        framebuffer_size,
        rule,
        out,
        (0, 0, framebuffer_size.0, framebuffer_size.1),
    )
}

pub(crate) fn raster_triangle_in_rect(
    a: &ScreenVertex,
    b: &ScreenVertex,
    c: &ScreenVertex,
    framebuffer_size: (u32, u32),
    rule: FillRule,
    out: &mut Vec<Fragment>,
    bounds: (u32, u32, u32, u32),
) -> u32 {
    raster_triangle_visit(a, b, c, framebuffer_size, rule, bounds, |fragment| {
        out.push(fragment)
    })
}

pub(crate) fn raster_triangle_visit(
    a: &ScreenVertex,
    b: &ScreenVertex,
    c: &ScreenVertex,
    framebuffer_size: (u32, u32),
    rule: FillRule,
    bounds: (u32, u32, u32, u32),
    mut emit: impl FnMut(Fragment),
) -> u32 {
    let area = edge_function(a, b, c);
    if !area.is_finite() || area.abs() < 1e-8 {
        return 0;
    }
    // Normaliza el winding para que el interior del triángulo quede en `w <= 0`
    // en las tres aristas, independientemente del orden de entrada.
    let (b, c) = if area > 0.0 { (c, b) } else { (b, c) };
    // denom = w0 + w1 + w2 = area (constante por triángulo, no por píxel).
    let denom = edge_function(a, b, c);
    if denom.abs() < f32::EPSILON {
        return 0;
    }
    let (fb_w, fb_h) = framebuffer_size;
    let min_x = a.x.min(b.x).min(c.x);
    let max_x = a.x.max(b.x).max(c.x);
    let min_y = a.y.min(b.y).min(c.y);
    let max_y = a.y.max(b.y).max(c.y);
    if !min_x.is_finite() || !max_x.is_finite() || !min_y.is_finite() || !max_y.is_finite() {
        return 0;
    }
    let x0 = (min_x.max(0.0).floor() as i64).max(i64::from(bounds.0));
    let x1 = (max_x.min(fb_w as f32 - 1.0).ceil() as i64).min(i64::from(bounds.2) - 1);
    let y0 = (min_y.max(0.0).floor() as i64).max(i64::from(bounds.1));
    let y1 = (max_y.min(fb_h as f32 - 1.0).ceil() as i64).min(i64::from(bounds.3) - 1);
    if x1 < x0 || y1 < y0 {
        return 0;
    }
    let mut count = 0;
    for py in y0..=y1 {
        if py < 0 || py >= fb_h as i64 {
            continue;
        }
        let sy = (py as f32) + 0.5;
        for px in x0..=x1 {
            if px < 0 || px >= fb_w as i64 {
                continue;
            }
            let sx = (px as f32) + 0.5;
            let p = ScreenVertex {
                x: sx,
                y: sy,
                z: 0.0,
                inv_w: 0.0,
                color: Vec4::ZERO,
            };
            // Edge functions computadas una sola vez; se reusan para cobertura
            // y para las coordenadas barícéntricas (evita recomputarlas como
            // hacía la antigua llamada a covers() más el recálculo posterior).
            let w0 = edge_function(b, c, &p);
            let w1 = edge_function(c, a, &p);
            let w2 = edge_function(a, b, &p);
            // Cobertura: descarta si el punto está estrictamente fuera de
            // cualquier arista (equivalente a covers() con FillRule::All).
            if w0 > 0.0 || w1 > 0.0 || w2 > 0.0 {
                continue;
            }
            // FillRule::TopLeft: si el punto cae en una arista, solo se acepta
            // si esa arista es superior o izquierda.
            if rule == FillRule::TopLeft {
                if w0 == 0.0 && !is_top_left(b, c) {
                    continue;
                }
                if w1 == 0.0 && !is_top_left(c, a) {
                    continue;
                }
                if w2 == 0.0 && !is_top_left(a, b) {
                    continue;
                }
            }
            let b0 = w0 / denom;
            let b1 = w1 / denom;
            let b2 = w2 / denom;
            let depth = b0 * a.z + b1 * b.z + b2 * c.z;
            if !depth.is_finite() {
                continue;
            }
            let inv_w = b0 * a.inv_w + b1 * b.inv_w + b2 * c.inv_w;
            if inv_w.abs() < f32::EPSILON {
                continue;
            }
            let w0p = b0 * a.inv_w / inv_w;
            let w1p = b1 * b.inv_w / inv_w;
            let w2p = b2 * c.inv_w / inv_w;
            let color = a.color * w0p + b.color * w1p + c.color * w2p;
            emit(Fragment {
                x: px as u32,
                y: py as u32,
                depth: depth.clamp(0.0, 1.0),
                color,
            });
            count += 1;
        }
    }
    count
}

/// Convierte un fragmento a su color final (etapa de fragment/shading).
pub fn shade_fragment(fragment: &Fragment) -> Color {
    shade_fragment_with_shader(fragment, FragmentShader::VertexColor)
}

/// Ejecuta un shader de fragmentos software sobre un fragmento rasterizado.
pub fn shade_fragment_with_shader(fragment: &Fragment, shader: FragmentShader) -> Color {
    shade_fragment_with_time(fragment, shader, 0.0)
}

/// Ejecuta un shader de fragmentos software con una fase temporal explícita.
pub fn shade_fragment_with_time(
    fragment: &Fragment,
    shader: FragmentShader,
    time: f32,
) -> Color {
    let color = match shader {
        FragmentShader::VertexColor => fragment.color,
        FragmentShader::SolarBands => {
            let time = if time.is_finite() {
                time.rem_euclid(4096.0)
            } else {
                0.0
            };
            let band = (((fragment.x / 16 + fragment.y / 16) as f32 + time * 0.5).floor()
                as i32
                & 1)
                == 0;
            let base = if band { 1.0 } else { 0.55 };
            let glow = ((fragment.x as f32 * 0.08 + time * 0.7).sin() * 0.5 + 0.5) * 0.25;
            glam::Vec4::new(
                (fragment.color.x * (base + glow)).clamp(0.0, 1.0),
                (fragment.color.y * (base + glow * 0.35)).clamp(0.0, 1.0),
                (fragment.color.z * (base + glow * 0.1)).clamp(0.0, 1.0),
                fragment.color.w,
            )
        }
    };
    vec4_to_rgba8(color)
}

/// Aplica el test de profundidad y escribe el fragmento en el framebuffer.
pub fn depth_resolve(
    fragment: &Fragment,
    color: Color,
    framebuffer: &mut Framebuffer,
    depth_test: bool,
    depth_write: bool,
) -> Result<bool, RasterError> {
    if depth_test {
        let existing = framebuffer.depth_at(fragment.x, fragment.y).unwrap_or(1.0);
        if fragment.depth >= existing {
            return Ok(false);
        }
    }
    framebuffer.set_pixel(fragment.x, fragment.y, color);
    if depth_write {
        framebuffer.set_depth(fragment.x, fragment.y, fragment.depth);
    }
    Ok(true)
}

/// Aplica el test de profundidad y escribe el fragmento en el framebuffer.
pub fn resolve_fragment(
    fragment: &Fragment,
    framebuffer: &mut Framebuffer,
    depth_test: bool,
    depth_write: bool,
) -> Result<bool, RasterError> {
    depth_resolve(
        fragment,
        shade_fragment(fragment),
        framebuffer,
        depth_test,
        depth_write,
    )
}