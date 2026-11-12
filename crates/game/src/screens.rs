//! Astral Echo - state machine for main menu and game-over screens.
//! Renderer-independent: the visual renderer draws the `ScreenView` returned by `view()`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen { MainMenu, Controls, Playing, GameOver }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction { Play, Controls, Exit, Retry, MainMenu }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenEvent { None, StartGame, RestartGame, ExitGame }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefeatReason { ShipDestroyed, CrewLost }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input { Up, Down, Confirm, Back }

#[derive(Debug, Clone, Copy)]
pub struct ScreenView {
    pub heading: &'static str,
    pub subtitle: &'static str,
    pub hint: &'static str,
    pub options: &'static [MenuAction],
    pub selected: usize,
    pub accent_rgb: [u8; 3],
}

/// Dorado de la pantalla de titulo (marcos, opcion seleccionada).
const GOLD: [u8; 3] = [232, 190, 32];

const MAIN_OPTIONS: &[MenuAction] = &[MenuAction::Play, MenuAction::Controls, MenuAction::Exit];
const DEATH_OPTIONS: &[MenuAction] = &[MenuAction::Retry, MenuAction::MainMenu];

impl MenuAction {
    pub fn label(self) -> &'static str {
        match self {
            Self::Play => "INICIAR PARTIDA",
            Self::Controls => "CONTROLES",
            Self::Exit => "SALIR",
            Self::Retry => "REINTENTAR",
            Self::MainMenu => "MENU PRINCIPAL",
        }
    }
}

pub struct Screens {
    pub current: Screen,
    pub defeat_reason: Option<DefeatReason>,
    selected: usize,
}

impl Default for Screens {
    fn default() -> Self {
        Self { current: Screen::MainMenu, defeat_reason: None, selected: 0 }
    }
}

impl Screens {
    pub fn view(&self) -> ScreenView {
        match self.current {
            Screen::MainMenu => ScreenView {
                heading: "ASTRAL ECHO", subtitle: "ECHO-7 // SISTEMA DE NAVEGACION",
                hint: "FLECHAS: SELECCIONAR   ENTER: CONFIRMAR",
                options: MAIN_OPTIONS, selected: self.selected, accent_rgb: GOLD,
            },
            Screen::Controls => ScreenView {
                heading: "CONTROLES",
                subtitle: "WASD: MOVER   FLECHAS: MIRAR   I: INTERACTUAR   M: MISION",
                hint: "F1: PANEL DE CONTROLES EN PARTIDA   ESC: REGRESAR",
                options: &[], selected: 0, accent_rgb: GOLD,
            },
            Screen::Playing => ScreenView {
                heading: "", subtitle: "", hint: "", options: &[],
                selected: 0, accent_rgb: [54, 240, 164],
            },
            Screen::GameOver => ScreenView {
                heading: "MISION FALLIDA",
                subtitle: match self.defeat_reason {
                    Some(DefeatReason::ShipDestroyed) => "LA NAVE ECHO-7 HA SIDO DESTRUIDA",
                    _ => "LA TRIPULACION NO PUEDE CONTINUAR",
                },
                hint: "FLECHAS: SELECCIONAR   ENTER: CONFIRMAR",
                options: DEATH_OPTIONS, selected: self.selected, accent_rgb: [255, 53, 72],
            },
        }
    }

    /// Call only when the gameplay model confirms a terminal defeat.
    /// Do not call this for a player who can still be revived.
    pub fn game_over(&mut self, reason: DefeatReason) {
        if self.current != Screen::Playing { return; }
        self.current = Screen::GameOver;
        self.defeat_reason = Some(reason);
        self.selected = 0;
    }

    pub fn input(&mut self, input: Input) -> ScreenEvent {
        let options = self.view().options;
        match input {
            Input::Up if !options.is_empty() => {
                self.selected = (self.selected + options.len() - 1) % options.len();
            }
            Input::Down if !options.is_empty() => {
                self.selected = (self.selected + 1) % options.len();
            }
            Input::Back if self.current == Screen::Controls => {
                self.current = Screen::MainMenu;
                self.selected = 0;
            }
            Input::Confirm if !options.is_empty() => {
                match options[self.selected] {
                    MenuAction::Play => {
                        self.current = Screen::Playing;
                        return ScreenEvent::StartGame;
                    }
                    MenuAction::Controls => self.current = Screen::Controls,
                    MenuAction::Exit => return ScreenEvent::ExitGame,
                    MenuAction::Retry => {
                        self.current = Screen::Playing;
                        self.defeat_reason = None;
                        return ScreenEvent::RestartGame;
                    }
                    MenuAction::MainMenu => {
                        self.current = Screen::MainMenu;
                        self.defeat_reason = None;
                        self.selected = 0;
                    }
                }
            }
            _ => {}
        }
        ScreenEvent::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn menu_starts_game() {
        let mut screens = Screens::default();
        assert_eq!(screens.input(Input::Confirm), ScreenEvent::StartGame);
        assert_eq!(screens.current, Screen::Playing);
    }
    #[test]
    fn defeat_and_retry() {
        let mut screens = Screens::default();
        screens.input(Input::Confirm);
        screens.game_over(DefeatReason::ShipDestroyed);
        assert_eq!(screens.current, Screen::GameOver);
        assert_eq!(screens.input(Input::Confirm), ScreenEvent::RestartGame);
        assert_eq!(screens.current, Screen::Playing);
    }
    #[test]
    fn controls_return_to_menu() {
        let mut screens = Screens::default();
        screens.input(Input::Down);
        screens.input(Input::Confirm);
        assert_eq!(screens.current, Screen::Controls);
        screens.input(Input::Back);
        assert_eq!(screens.current, Screen::MainMenu);
    }
    #[test]
    fn selection_wraps_in_both_directions() {
        let mut screens = Screens::default();
        screens.input(Input::Up);
        assert_eq!(screens.view().selected, MAIN_OPTIONS.len() - 1);
        screens.input(Input::Down);
        assert_eq!(screens.view().selected, 0);
    }
    #[test]
    fn exit_option_requests_exit_and_stays_on_menu() {
        let mut screens = Screens::default();
        screens.input(Input::Up); // da la vuelta hasta SALIR
        assert_eq!(screens.view().options[screens.view().selected], MenuAction::Exit);
        assert_eq!(screens.input(Input::Confirm), ScreenEvent::ExitGame);
        assert_eq!(screens.current, Screen::MainMenu);
    }
    #[test]
    fn menu_ignores_input_while_playing() {
        let mut screens = Screens::default();
        screens.input(Input::Confirm);
        assert_eq!(screens.input(Input::Down), ScreenEvent::None);
        assert_eq!(screens.input(Input::Confirm), ScreenEvent::None);
        assert_eq!(screens.current, Screen::Playing);
    }
}