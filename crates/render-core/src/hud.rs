use crate::HudState;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HudOverlayRect { pub x: u32, pub y: u32, pub width: u32, pub height: u32, pub color: [u8; 4] }

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HudOverlay { pub background: HudOverlayRect, pub glyphs: Vec<HudOverlayRect> }

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

pub fn hud_overlay(hud: &HudState, width: u32, height: u32) -> HudOverlay {
    if hud.mission_name.is_none()
        && hud.mission_description.is_none()
        && hud.objective_name.is_none()
        && hud.mission_objectives.is_empty()
        && hud.mission_progress.is_none()
        && hud.mission_state.is_none()
        && hud.mission_notification.is_none()
        && hud.interaction_prompt.is_none()
        && hud.guidance.is_none()
        && hud.controls.is_none()
        && hud.instructions.is_none()
        && hud.feedback.as_deref().is_none_or(str::is_empty)
    {
        return HudOverlay {
            background: HudOverlayRect { x: 0, y: 0, width: 0, height: 0, color: [0; 4] },
            glyphs: Vec::new(),
        };
    }
    let scale = (width / 480).clamp(1, 3);
    let margin = 12 * scale;
    let char_width = 7 * scale;
    let max_chars = ((width.saturating_sub(margin * 2)) / char_width).clamp(24, 64) as usize;
    let progress = hud.mission_progress.map(|(d, t)| format!("{d}/{t}"));
    let mut lines = vec![
        hud.mission_name.as_deref().map(|v| format!("MISION: {v}")),
        Some(format!("ESTADO: {}  PROGRESO: {}", status(hud), progress.as_deref().unwrap_or("-"))),
        hud.mission_description.as_deref().map(|v| format!("CONTEXTO: {v}")),
        hud.objective_name.as_deref().map(|v| format!("OBJETIVO: {v}")),
        hud.mission_notification.as_deref().map(|v| format!("AVISO: {v}")),
        None,
        Some("OBJETIVOS:".to_owned()).filter(|_| !hud.mission_objectives.is_empty()),
        None,
        None,
        hud.guidance.as_deref().map(|v| format!("GUIA: {v}")),
        hud.interaction_prompt.as_deref().map(|v| format!("INTERACCION: {v}")),
        hud.controls.as_deref().map(|v| format!("CONTROLES: {v}")),
        hud.instructions.as_deref().map(|v| format!("INSTRUCCIONES: {v}")),
        hud.feedback.as_deref().filter(|v| !v.is_empty()).map(|v| format!("FEEDBACK: {v}")),
    ].into_iter().flatten().collect::<Vec<_>>();
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
    for value in &mut lines { value.truncate(max_chars); }
    if lines.is_empty() { lines.push("ASTRAL ECHO".to_owned()); }
    let panel_width = (max_chars as u32 * char_width + margin * 2).min(width.saturating_sub(margin));
    let panel_height = (margin + lines.len() as u32 * 8 * scale + margin).min(height.saturating_sub(margin));
    let terminal = hud.feedback.as_deref().is_some_and(|v| v.contains("DERROTADO"));
    let color = if terminal { [255,96,80,255] } else { [210,240,255,255] };
    let background = HudOverlayRect { x: margin, y: margin, width: panel_width, height: panel_height, color: [5,12,24,220] };
    let mut glyphs = Vec::new();
    for (row, text) in lines.iter().enumerate() {
        for (column, ch) in text.chars().enumerate() {
            let Some(pattern) = glyph(normalize(ch)) else { continue };
            for (glyph_row, bits) in pattern.iter().copied().enumerate() {
                for glyph_column in 0..5u32 {
                    if bits & (1 << (4 - glyph_column)) != 0 {
                        glyphs.push(HudOverlayRect { x: margin + column as u32 * char_width + glyph_column * scale, y: margin + row as u32 * 8 * scale + glyph_row as u32 * scale, width: scale, height: scale, color });
                    }
                }
            }
        }
    }
    HudOverlay { background, glyphs }
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
    fn terminal_feedback_uses_high_contrast_color() {
        let hud = HudState { feedback: Some("JUGADOR DERROTADO".into()), ..Default::default() };
        let overlay = hud_overlay(&hud, 640, 360);
        assert!(overlay.glyphs.iter().all(|rect| rect.color[0] > rect.color[1]));
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
        // "FEEDBACK: A B": the second space starts at column 11 with the
        // seven-pixel advance used by the shared CPU/GPU layout.
        let space_start = 12 + 11 * 7;
        let next_word_start = space_start + 7;
        assert!(!overlay.glyphs.iter().any(|rect| {
            rect.x >= space_start && rect.x < next_word_start
        }));
        assert!(overlay.glyphs.iter().any(|rect| rect.x >= next_word_start));
    }
}