use crate::HudState;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HudOverlayRect { pub x: u32, pub y: u32, pub width: u32, pub height: u32, pub color: [u8; 4] }

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HudOverlay {
    pub background: HudOverlayRect,
    pub glyphs: Vec<HudOverlayRect>,
    pub contextual_button: Option<HudOverlayRect>,
    pub controls_button: Option<HudOverlayRect>,
    pub controls_close_button: Option<HudOverlayRect>,
}

const FONT: [([u8; 7], char); 39] = [
    ([14,17,17,31,17,17,17],'A'),([30,17,17,30,17,17,30],'B'),([14,17,16,16,16,17,14],'C'),
    ([30,17,17,17,17,17,30],'D'),([31,16,16,30,16,16,31],'E'),([31,16,16,30,16,16,16],'F'),
    ([14,17,16,23,17,17,15],'G'),([17,17,17,31,17,17,17],'H'),([14,4,4,4,4,4,14],'I'),
    ([1,1,1,1,17,17,14],'J'),([17,18,20,24,20,18,17],'K'),([16,16,16,16,16,16,31],'L'),
    ([17,27,21,21,17,17,17],'M'),([17,25,21,19,17,17,17],'N'),([14,17,17,17,17,17,14],'O'),
    ([30,17,17,30,16,16,16],'P'),([14,17,17,17,21,18,13],'Q'),([30,17,17,30,20,18,17],'R'),
    ([15,16,16,14,1,1,30],'S'),([31,4,4,4,4,4,4],'T'),([17,17,17,17,17,17,14],'U'),
    ([17,17,17,17,17,10,4],'V'),([17,17,17,21,21,27,17],'W'),([17,17,10,4,10,17,17],'X'),
    ([17,17,10,4,4,4,4],'Y'),([31,1,2,4,8,16,31],'Z'),([14,17,19,21,25,17,14],'0'),
    ([4,12,4,4,4,4,14],'1'),([14,17,1,2,4,8,31],'2'),([30,1,1,14,1,1,30],'3'),
    ([2,6,10,18,31,2,2],'4'),([31,16,16,30,1,1,30],'5'),([14,16,16,30,17,17,14],'6'),
    ([31,1,2,4,8,8,8],'7'),([14,17,17,14,17,17,14],'8'),([14,17,17,15,1,1,14],'9'),
    ([0,0,0,31,0,0,0],'-'),([0,0,4,0,0,4,0],':'),([1,2,4,8,16,0,0],'/')
];

fn glyph(ch: char) -> Option<[u8; 7]> { FONT.iter().find_map(|(p, c)| (*c == ch).then_some(*p)) }
fn normalize(ch: char) -> char {
    if ch == '—' {
        return '-';
    }
    match ch {
        'á'|'à'|'ä'|'â'|'Á'|'À'|'Ä'|'Â' => 'A', 'é'|'è'|'ë'|'ê'|'É'|'È'|'Ë'|'Ê' => 'E',
        'í'|'ì'|'ï'|'î'|'Í'|'Ì'|'Ï'|'Î' => 'I', 'ó'|'ò'|'ö'|'ô'|'Ó'|'Ò'|'Ö'|'Ô' => 'O',
        'ú'|'ù'|'ü'|'û'|'Ú'|'Ù'|'Ü'|'Û' => 'U', 'ñ'|'Ñ' => 'N', other => other.to_ascii_uppercase(),
    }
}
fn status(hud: &HudState) -> &'static str {
    match hud.mission_state {
        Some(crate::HudMissionState::Locked) => "BLOQUEADA", Some(crate::HudMissionState::Available) => "DISPONIBLE",
        Some(crate::HudMissionState::Active) => "ACTIVA", Some(crate::HudMissionState::Failed) => "FALLIDA",
        Some(crate::HudMissionState::Completed) => "COMPLETADA", None => "SIN ESTADO",
    }
}

fn objective_status(state: crate::HudObjectiveState) -> &'static str {
    match state {
        crate::HudObjectiveState::Locked => "BLOQUEADA",
        crate::HudObjectiveState::Available => "PENDIENTE",
        crate::HudObjectiveState::Active => "ACTIVA",
        crate::HudObjectiveState::Failed => "FALLIDA",
        crate::HudObjectiveState::Completed => "COMPLETADA",
    }
}

fn wrap_lines(lines: &[String], max_chars: usize) -> Vec<String> {
    let max_chars = max_chars.max(1);
    let mut wrapped = Vec::new();
    for source in lines {
        let mut current = String::new();
        for word in source.split_whitespace() {
            let word_len = word.chars().count();
            if word_len > max_chars {
                if !current.is_empty() {
                    wrapped.push(std::mem::take(&mut current));
                }
                let mut chunk = String::new();
                for ch in word.chars() {
                    chunk.push(ch);
                    if chunk.chars().count() == max_chars {
                        wrapped.push(std::mem::take(&mut chunk));
                    }
                }
                current = chunk;
            } else if current.is_empty() {
                current.push_str(word);
            } else if current.chars().count() + 1 + word_len <= max_chars {
                current.push(' ');
                current.push_str(word);
            } else {
                wrapped.push(std::mem::take(&mut current));
                current.push_str(word);
            }
        }
        if !current.is_empty() || source.is_empty() {
            wrapped.push(current);
        }
    }
    wrapped
}

fn panel_text_width(lines: &[String], char_width: u32, margin: u32) -> u32 {
    lines
        .iter()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(1) as u32
        * char_width
        + margin * 2
}

fn panel_char_capacity(width: u32, char_width: u32, margin: u32) -> usize {
    width
        .saturating_sub(margin.saturating_mul(2))
        .checked_div(char_width.max(1))
        .unwrap_or(1)
        .max(1) as usize
}

fn visible_text_rows(y: u32, margin: u32, height: u32, scale: u32) -> usize {
    let first_row = y.saturating_add(margin);
    let glyph_height = 7_u32.saturating_mul(scale);
    if first_row.saturating_add(glyph_height) > height {
        return 0;
    }
    let row_height = 10_u32.saturating_mul(scale).max(1);
    (1 + height.saturating_sub(first_row + glyph_height) / row_height) as usize
}

fn push_text_panel(
    glyphs: &mut Vec<HudOverlayRect>,
    lines: &[String],
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    scale: u32,
    char_width: u32,
    margin: u32,
    text_color: [u8; 4],
    background_color: [u8; 4],
) -> HudOverlayRect {
    let max_chars = width
        .saturating_sub(margin * 2)
        .checked_div(char_width.max(1))
        .unwrap_or(1)
        .max(1) as usize;
    let panel_height = (margin + lines.len() as u32 * 10 * scale + margin)
        .min(height.saturating_sub(y).max(1));
    let panel = HudOverlayRect {
        x,
        y,
        width,
        height: panel_height,
        color: background_color,
    };
    glyphs.push(panel);
    for (row, source) in lines
        .iter()
        .take(visible_text_rows(panel.y, margin, height, scale))
        .enumerate()
    {
        let text: String = source.chars().take(max_chars).collect();
        for (column, ch) in text.chars().enumerate() {
            let Some(pattern) = glyph(normalize(ch)) else { continue };
            for (glyph_row, bits) in pattern.iter().copied().enumerate() {
                for glyph_column in 0..5u32 {
                    if bits & (1 << (4 - glyph_column)) != 0 {
                        glyphs.push(HudOverlayRect {
                            x: panel.x + margin + column as u32 * char_width + glyph_column * scale,
                            y: panel.y + margin + row as u32 * 10 * scale + glyph_row as u32 * scale,
                            width: scale,
                            height: scale,
                            color: text_color,
                        });
                    }
                }
            }
        }
    }
    panel
}

pub fn hud_overlay(hud: &HudState, width: u32, height: u32) -> HudOverlay {
    if hud.mission_name.is_none()
        && hud.mission_description.is_none()
        && hud.objective_name.is_none()
        && hud.mission_objectives.is_empty()
        && hud.mission_progress.is_none()
        && hud.mission_state.is_none()
        && hud.mission_notification.is_none()
        && !hud.ship_shield_active
        && !hud.ship_boost_active
        && hud.ship_speed.is_none()
        && hud.weapon_name.is_none()
        && hud.weapon_ammunition.is_none()
        && hud.ship_fuel.is_none()
        && hud.ship_oxygen.is_none()
        && hud.ship_supplies.is_none()
        && hud.ship_resource_alert.is_none()
        && hud.contextual_action.is_none()
        && hud.controls_panel.entries.is_empty()
        && hud.interaction_prompt.is_none()
        && hud.guidance.is_none()
        && hud.controls.is_none()
        && hud.instructions.is_none()
        && hud.quick_actions.is_empty()
        && hud.feedback.as_deref().is_none_or(str::is_empty)
        && !hud.inventory_open
    {
        return HudOverlay {
            background: HudOverlayRect { x: 0, y: 0, width: 0, height: 0, color: [0; 4] },
            glyphs: Vec::new(),
            contextual_button: None,
            controls_button: None,
            controls_close_button: None,
        };
    }
    let scale = (width / 900).clamp(1, 2);
    let margin = 6 * scale;
    let char_width = 8 * scale;
    let progress = hud.mission_progress.map(|(d, t)| format!("{d}/{t}"));
    let mut lines = vec![
        hud.mission_name.as_deref().map(|v| format!("MISION: {v}")),
        Some(format!("ESTADO: {}  PROGRESO: {}", status(hud), progress.as_deref().unwrap_or("-"))),
        hud.mission_description.as_deref().map(|v| format!("CONTEXTO: {v}")),
        hud.objective_name.as_deref().map(|v| format!("OBJETIVO: {v}")),
        Some("OBJETIVOS:".to_owned()).filter(|_| !hud.mission_objectives.is_empty()),
        hud.guidance.as_deref().map(|v| format!("ERICK: {v}")),
        hud.instructions.as_deref().map(|v| format!("MODO: {v}")),
    ].into_iter().flatten().collect::<Vec<_>>();
    let inventory_lines = hud.inventory_open.then(|| {
        std::iter::once("INVENTARIO".to_owned())
            .chain(hud.inventory_items.iter().cloned())
            .collect::<Vec<_>>()
    });
    if !hud.mission_objectives.is_empty() {
        let objective_lines = hud.mission_objectives.iter().enumerate().map(|(index, objective)| {
            let optional = if objective.required { "" } else { " OPCIONAL" };
            format!(
                "{} {} {}/{} {}{}",
                index + 1,
                objective_status(objective.state),
                objective.progress,
                objective.required_progress,
                objective.name,
                optional,
            )
        });
        let insertion = lines.iter().position(|line| line == "OBJETIVOS:").unwrap_or(lines.len());
        lines.splice(insertion + 1..insertion + 1, objective_lines);
    }
    lines.retain(|line| !line.is_empty());
    if lines.is_empty() { lines.push("ASTRAL ECHO".to_owned()); }
    let left_max_width = (width * 45 / 100).min(width.saturating_sub(margin));
    lines = wrap_lines(&lines, panel_char_capacity(left_max_width, char_width, margin));
    let text_width = panel_text_width(&lines, char_width, margin);
    let button_height = hud.contextual_action.as_ref().map_or(0, |_| 12 * scale);
    let panel_width = text_width.min(left_max_width);
    let panel_height = (margin + lines.len() as u32 * 10 * scale + margin + button_height)
        .min(height.saturating_sub(margin));
    let terminal = hud.feedback.as_deref().is_some_and(|v| v.contains("DERROTADO"));
    let color = if terminal { [255,96,80,255] } else { [210,240,255,255] };
    let background = HudOverlayRect { x: margin, y: margin, width: panel_width, height: panel_height, color: [5,12,24,220] };
    let visible_left_rows = visible_text_rows(margin, 0, height, scale);
    let visible_left_chars = lines
        .iter()
        .take(visible_left_rows)
        .map(|line| line.chars().count())
        .sum::<usize>();
    let mut glyphs = Vec::with_capacity(visible_left_chars.saturating_mul(12).max(256));
    for (row, text) in lines
        .iter()
        .take(visible_left_rows)
        .enumerate()
    {
        for (column, ch) in text.chars().enumerate() {
            let Some(pattern) = glyph(normalize(ch)) else { continue };
            for (glyph_row, bits) in pattern.iter().copied().enumerate() {
                for glyph_column in 0..5u32 {
                    if bits & (1 << (4 - glyph_column)) != 0 {
                        glyphs.push(HudOverlayRect { x: margin + column as u32 * char_width + glyph_column * scale, y: margin + row as u32 * 10 * scale + glyph_row as u32 * scale, width: scale, height: scale, color });
                    }
                }
            }
        }
    }

    let right_lines = [
        hud.target.map(|target| format!("DESTINO: {target}")),
        Some(format!("VUELO: {:?}", hud.flight)).filter(|_| hud.ship_speed.is_some()),
        Some(format!("ESCUDO: {}", if hud.ship_shield_active { "ACTIVO" } else { "INACTIVO" }))
            .filter(|_| hud.ship_speed.is_some() || hud.ship_shield_active),
        hud.ship_speed.map(|(speed, limit)| format!("VELOCIDAD: {speed}/{limit}")),
        Some("BOOST: ACTIVO".to_owned()).filter(|_| hud.ship_boost_active),
        hud.weapon_name.as_deref().map(|name| format!("ARMA: {name}")),
        hud.weapon_ammunition.map(|(_, amount)| format!("MUNICION: {amount}")),
        hud.ship_fuel.map(|(current, capacity)| format!("COMBUSTIBLE: {current}/{capacity}")),
        hud.ship_oxygen.map(|(current, capacity)| format!("OXIGENO: {current}/{capacity}")),
        hud.ship_supplies.map(|amount| format!("SUMINISTROS: {amount}")),
        hud.ship_resource_alert.as_deref().map(|value| format!("ALERTA: {value}")),
        (hud.active_enemies > 0).then(|| format!("ENEMIGOS: {}", hud.active_enemies)),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    if !right_lines.is_empty() {
        let right_max_width = (width * 38 / 100).max(char_width + margin * 2);
        let right_wrapped = wrap_lines(&right_lines, panel_char_capacity(right_max_width, char_width, margin / 2));
        let right_width = panel_text_width(&right_wrapped, char_width, margin / 2).min(right_max_width);
        push_text_panel(
            &mut glyphs,
            &right_wrapped,
            width.saturating_sub(margin + right_width),
            margin,
            right_width,
            height,
            scale,
            char_width,
            margin / 2,
            [210, 240, 255, 255],
            [5, 12, 24, 220],
        );
    }

    let mut center_lines = Vec::new();
    if !hud.quick_actions.is_empty() {
        center_lines.push("ACCIONES RAPIDAS".to_owned());
        center_lines.extend(hud.quick_actions.iter().cloned());
    }
    if let Some(value) = hud.interaction_prompt.as_deref() {
        center_lines.push(format!("INTERACCION: {value}"));
    }
    if let Some(value) = hud
        .feedback
        .as_deref()
        .filter(|value| {
            !value.is_empty()
                && (!hud.quick_actions.is_empty() || terminal || value.starts_with("ERICK:"))
        })
    {
        if value.starts_with("ERICK:") {
            center_lines.push(value.to_owned());
        } else {
            center_lines.push(format!("FEEDBACK: {value}"));
        }
    }
    if !center_lines.is_empty() {
        let center_max_width = (width * 78 / 100).max(char_width + margin);
        center_lines = wrap_lines(&center_lines, panel_char_capacity(center_max_width, char_width, margin / 2));
        let center_width = panel_text_width(&center_lines, char_width, margin / 2).min(center_max_width);
        let estimated_height = margin + center_lines.len() as u32 * 10 * scale + margin;
        let center_y = height.saturating_sub(estimated_height + margin).max(height / 2);
        push_text_panel(
            &mut glyphs,
            &center_lines,
            width.saturating_sub(center_width) / 2,
            center_y,
            center_width,
            height,
            scale,
            char_width,
            margin / 2,
            if terminal { [255, 96, 80, 255] } else { [235, 250, 255, 255] },
            if terminal { [80, 20, 20, 235] } else { [8, 18, 32, 235] },
        );
    }

    if let Some(notification) = hud.mission_notification.as_deref() {
        let text = vec![format!("AVISO: {notification}")];
        let notification_max_width = (width * 78 / 100).max(char_width + margin);
        let wrapped = wrap_lines(&text, panel_char_capacity(notification_max_width, char_width, margin / 2));
        let notification_width = panel_text_width(&wrapped, char_width, margin / 2).min(notification_max_width);
        let notification_height = margin + wrapped.len() as u32 * 10 * scale + margin;
        let notification_x = width.saturating_sub(notification_width) / 2;
        let notification_y = height.saturating_sub(notification_height + margin);
        push_text_panel(&mut glyphs, &wrapped, notification_x, notification_y, notification_width,
            height, scale, char_width, margin / 2, color, [8, 18, 32, 235]);
    }
    if let Some(inventory_lines) = inventory_lines {
        let inventory_max_width = (width * 38 / 100).max(char_width + margin);
        let inventory_lines = wrap_lines(&inventory_lines, panel_char_capacity(inventory_max_width, char_width, margin / 2));
        let inventory_width = panel_text_width(&inventory_lines, char_width, margin / 2).min(inventory_max_width);
        let inventory_height = (margin + inventory_lines.len() as u32 * 10 * scale + margin)
            .min(height.saturating_sub(margin * 2));
        let inventory_x = width.saturating_sub(margin + inventory_width);
        glyphs.push(HudOverlayRect {
            x: inventory_x,
            y: margin,
            width: inventory_width,
            height: inventory_height,
            color: [12, 24, 44, 245],
        });
        for (row, text) in inventory_lines
            .iter()
            .take(visible_text_rows(margin, margin / 2, height, scale))
            .enumerate()
        {
            for (column, ch) in text.chars().enumerate() {
                let Some(pattern) = glyph(normalize(ch)) else { continue };
                for (glyph_row, bits) in pattern.iter().copied().enumerate() {
                    for glyph_column in 0..5u32 {
                        if bits & (1 << (4 - glyph_column)) != 0 {
                            glyphs.push(HudOverlayRect {
                                x: inventory_x + margin / 2 + column as u32 * char_width + glyph_column * scale,
                                y: margin + margin / 2 + row as u32 * 10 * scale + glyph_row as u32 * scale,
                                width: scale,
                                height: scale,
                                color: [245, 250, 255, 255],
                            });
                        }
                    }
                }
            }
        }
    }
    let contextual_button = hud.contextual_action.as_ref().map(|action| {
        let label_width = (action.key.chars().count() + action.action.chars().count() + 1) as u32
            * char_width;
        let width = (label_width + margin).min(panel_width.saturating_sub(margin));
        let height = button_height.max(8 * scale);
        let button = HudOverlayRect {
            x: margin,
            y: panel_height.saturating_sub(margin + height),
            width,
            height,
            color: [24, 106, 150, 255],
        };
        let label = format!("{} {}", action.key, action.action);
        for (column, ch) in label.chars().enumerate() {
            let Some(pattern) = glyph(normalize(ch)) else { continue };
            for (glyph_row, bits) in pattern.iter().copied().enumerate() {
                for glyph_column in 0..5u32 {
                    if bits & (1 << (4 - glyph_column)) != 0 {
                        glyphs.push(HudOverlayRect {
                            x: button.x + margin / 2 + column as u32 * char_width + glyph_column * scale,
                            y: button.y + (button.height.saturating_sub(7 * scale)) / 2 + glyph_row as u32 * scale,
                            width: scale,
                            height: scale,
                            color: [235, 250, 255, 255],
                        });
                    }
                }
            }
        }
        glyphs.insert(0, button);
        button
    });
    let controls_available = !hud.controls_panel.entries.is_empty();
    let indicator_height = 12 * scale;
    let indicator_label = if hud.controls_panel.open { "F1 CERRAR" } else { "F1 CONTROLES" };
    let indicator_width = ((indicator_label.chars().count() as u32 + 1) * char_width + margin)
        .min(width.saturating_sub(margin * 2));
    let indicator = HudOverlayRect {
        x: width.saturating_sub(margin + indicator_width),
        y: height.saturating_sub(margin + indicator_height),
        width: indicator_width,
        height: indicator_height,
        color: [42, 76, 110, 255],
    };
    if controls_available {
        glyphs.insert(0, indicator);
    }
    for (column, ch) in indicator_label.chars().enumerate() {
        if !controls_available { break; }
        let Some(pattern) = glyph(normalize(ch)) else { continue };
        for (glyph_row, bits) in pattern.iter().copied().enumerate() {
            for glyph_column in 0..5u32 {
                if bits & (1 << (4 - glyph_column)) != 0 {
                    glyphs.push(HudOverlayRect {
                        x: indicator.x + margin / 2 + column as u32 * char_width + glyph_column * scale,
                        y: indicator.y + (indicator.height.saturating_sub(7 * scale)) / 2 + glyph_row as u32 * scale,
                        width: scale,
                        height: scale,
                        color: [235, 250, 255, 255],
                    });
                }
            }
        }
    }
    let controls_close_button = if controls_available && hud.controls_panel.open {
        let raw_panel_lines = std::iter::once("CONTROLES - F1 CERRAR".to_owned())
            .chain(hud.controls_panel.entries.iter().cloned())
            .collect::<Vec<_>>();
        let controls_max_width = (width * 42 / 100).max(char_width + margin);
        let panel_lines = wrap_lines(&raw_panel_lines, panel_char_capacity(controls_max_width, char_width, margin / 2));
        let panel_width = panel_text_width(&panel_lines, char_width, margin / 2).min(controls_max_width);
        let panel_height = (margin + panel_lines.len() as u32 * 10 * scale + margin)
            .min(height.saturating_sub(margin * 2));
        let panel = HudOverlayRect {
            x: width.saturating_sub(margin + panel_width),
            y: indicator.y.saturating_sub(margin + panel_height),
            width: panel_width,
            height: panel_height,
            color: [5, 18, 36, 245],
        };
        glyphs.insert(0, panel);
        for (row, text) in panel_lines
            .iter()
            .take(visible_text_rows(panel.y, margin, height, scale))
            .enumerate()
        {
            for (column, ch) in text.chars().enumerate() {
                let Some(pattern) = glyph(normalize(ch)) else { continue };
                for (glyph_row, bits) in pattern.iter().copied().enumerate() {
                    for glyph_column in 0..5u32 {
                        if bits & (1 << (4 - glyph_column)) != 0 {
                            glyphs.push(HudOverlayRect {
                                x: panel.x + margin / 2 + column as u32 * char_width + glyph_column * scale,
                                y: panel.y + margin + row as u32 * 10 * scale + glyph_row as u32 * scale,
                                width: scale,
                                height: scale,
                                color: [235, 250, 255, 255],
                            });
                        }
                    }
                }
            }
        }
        Some(indicator)
    } else {
        None
    };
    HudOverlay {
        background,
        glyphs,
        contextual_button,
        controls_button: controls_available.then_some(indicator),
        controls_close_button,
    }
}

pub fn hud_contextual_button_hit(
    hud: &HudState,
    width: u32,
    height: u32,
    x: u32,
    y: u32,
) -> bool {
    hud_overlay(hud, width, height)
        .contextual_button
        .is_some_and(|button| x >= button.x && x < button.x.saturating_add(button.width)
            && y >= button.y && y < button.y.saturating_add(button.height))
}

pub fn hud_controls_button_hit(
    hud: &HudState,
    width: u32,
    height: u32,
    x: u32,
    y: u32,
) -> bool {
    hud_overlay(hud, width, height)
        .controls_button
        .is_some_and(|button| x >= button.x && x < button.x.saturating_add(button.width)
            && y >= button.y && y < button.y.saturating_add(button.height))
}

pub fn hud_controls_panel_hit(
    hud: &HudState,
    width: u32,
    height: u32,
    x: u32,
    y: u32,
) -> bool {
    hud_overlay(hud, width, height)
        .controls_close_button
        .is_some_and(|button| x >= button.x && x < button.x.saturating_add(button.width)
            && y >= button.y && y < button.y.saturating_add(button.height))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hud_layout_contains_mission_and_feedback_pixels() {
        let hud = HudState { mission_name: Some("El despertar".into()), feedback: Some("Listo".into()), ..Default::default() };
        let overlay = hud_overlay(&hud, 640, 360);
        assert!(overlay.background.width > 0 && !overlay.glyphs.is_empty());
    }
    #[test]
    fn hud_layout_scales_and_clips_long_text() {
        let hud = HudState { mission_name: Some("X".repeat(500)), ..Default::default() };
        let overlay = hud_overlay(&hud, 320, 180);
        assert!(overlay.background.x + overlay.background.width <= 320);
        assert!(overlay.glyphs.iter().all(|r| r.x + r.width <= 320 && r.y + r.height <= 180));
    }

    #[test]
    fn crowded_inventory_and_controls_stay_inside_small_viewports() {
        let hud = HudState {
            inventory_open: true,
            inventory_items: vec!["RECURSO MUY LARGO".repeat(20); 20],
            controls_panel: crate::HudControlsPanel {
                open: true,
                entries: vec!["CONTROL MUY LARGO".repeat(20); 20],
            },
            ..Default::default()
        };
        let overlay = hud_overlay(&hud, 320, 180);
        assert!(overlay
            .glyphs
            .iter()
            .all(|rect| rect.x + rect.width <= 320 && rect.y + rect.height <= 180));
    }

    #[test]
    fn terminal_feedback_uses_high_contrast_color() {
        let hud = HudState { feedback: Some("JUGADOR DERROTADO".into()), ..Default::default() };
        let overlay = hud_overlay(&hud, 640, 360);
        assert!(overlay.glyphs.iter().all(|rect| rect.color[0] > rect.color[1]));
    }

    #[test]
    fn erick_feedback_is_visible_without_quick_actions() {
        let hud = HudState {
            feedback: Some("ERICK: destino de navegación seleccionado.".into()),
            ..Default::default()
        };
        let overlay = hud_overlay(&hud, 640, 360);
        assert!(overlay.glyphs.iter().any(|glyph| glyph.y > 180));
    }

    #[test]
    fn contextual_action_has_a_hit_testable_button() {
        let hud = HudState {
            contextual_action: Some(crate::HudContextualAction {
                target: 7,
                object: "PUERTA".into(),
                action: "ABRIR".into(),
                key: "I".into(),
            }),
            ..Default::default()
        };
        let overlay = hud_overlay(&hud, 640, 360);
        let button = overlay.contextual_button.expect("contextual button should exist");
        assert!(hud_contextual_button_hit(&hud, 640, 360, button.x + 1, button.y + 1));
        assert!(!hud_contextual_button_hit(&hud, 640, 360, 0, 0));
    }

    #[test]
    fn controls_panel_has_open_and_close_hit_targets() {
        let hud = HudState {
            controls_panel: crate::HudControlsPanel {
                open: true,
                entries: vec!["WASD MOVER".into()],
            },
            ..Default::default()
        };
        let overlay = hud_overlay(&hud, 640, 360);
        let button = overlay.controls_button.expect("controls indicator should exist");
        assert!(hud_controls_button_hit(&hud, 640, 360, button.x + 1, button.y + 1));
        assert!(hud_controls_panel_hit(&hud, 640, 360, button.x + 1, button.y + 1));
        assert!(!hud_controls_panel_hit(&hud, 640, 360, 0, 0));
    }

    #[test]
    fn controls_indicator_contains_visible_text_inside_its_button() {
        let hud = HudState {
            controls_panel: crate::HudControlsPanel {
                entries: vec!["WASD MOVER".into()],
                ..Default::default()
            },
            ..Default::default()
        };
        let overlay = hud_overlay(&hud, 640, 360);
        let button = overlay.controls_button.expect("controls indicator should exist");
        let text_color = [235, 250, 255, 255];
        assert!(overlay.glyphs.iter().any(|rect| {
            rect.color == text_color
                && rect.x >= button.x
                && rect.y >= button.y
                && rect.x.saturating_add(rect.width) <= button.x.saturating_add(button.width)
                && rect.y.saturating_add(rect.height) <= button.y.saturating_add(button.height)
        }));
        assert!(hud_controls_button_hit(
            &hud,
            640,
            360,
            button.x + button.width / 2,
            button.y + button.height / 2,
        ));
    }

    #[test]
    fn mission_overlay_contains_all_objective_states_and_progress() {
        let hud = HudState {
            mission_name: Some("El despertar".into()),
            mission_objectives: vec![
                crate::HudMissionObjective {
                    id: 1,
                    name: "Evaluar el estado".into(),
                    state: crate::HudObjectiveState::Completed,
                    progress: 1,
                    required_progress: 1,
                    required: true,
                },
                crate::HudMissionObjective {
                    id: 2,
                    name: "Restaurar energia".into(),
                    state: crate::HudObjectiveState::Active,
                    progress: 0,
                    required_progress: 1,
                    required: true,
                },
            ],
            ..Default::default()
        };
        let overlay = hud_overlay(&hud, 640, 360);
        assert!(overlay.background.height > 60);
        assert!(!overlay.glyphs.is_empty());
        assert!(overlay.glyphs.iter().all(|rect| {
            rect.x + rect.width <= 640 && rect.y + rect.height <= 360
        }));
    }

    #[test]
    fn spaces_have_no_fallback_glyph_and_words_keep_an_advance() {
        assert!(glyph(' ').is_none());
        assert!(glyph('A').is_some());

        let overlay = hud_overlay(
            &HudState { feedback: Some("A B".into()), ..Default::default() },
            640,
            360,
        );
        
        let space_start = 6 + 11 * 8;
        let next_word_start = space_start + 8;
        assert!(!overlay.glyphs.iter().any(|rect| {
            rect.x >= space_start && rect.x < next_word_start
        }));
        assert!(overlay.glyphs.iter().any(|rect| rect.x >= next_word_start));
    }
}
