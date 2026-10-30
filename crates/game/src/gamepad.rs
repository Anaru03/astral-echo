use std::time::{Duration, Instant};

use gilrs::ff::{BaseEffect, BaseEffectType, Effect, EffectBuilder, Replay, Repeat, Ticks};
use gilrs::{Axis, Button, EventType, GamepadId, Gilrs};

pub const DEFAULT_DEADZONE: f32 = 0.18;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HapticEvent {
    AttackExecuted,
    EnemyDefeated,
    ItemCollected,
    Impact,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HapticPattern {
    pub weak: u16,
    pub strong: u16,
    pub duration_ms: u32,
    pub priority: u8,
}

pub fn haptic_pattern(event: HapticEvent) -> HapticPattern {
    match event {
        HapticEvent::AttackExecuted => HapticPattern {
            weak: 26_000,
            strong: 9_000,
            duration_ms: 100,
            priority: 1,
        },
        HapticEvent::EnemyDefeated => HapticPattern {
            weak: 45_000,
            strong: 30_000,
            duration_ms: 160,
            priority: 2,
        },
        HapticEvent::ItemCollected => HapticPattern {
            weak: 18_000,
            strong: 0,
            duration_ms: 80,
            priority: 1,
        },
        HapticEvent::Impact => HapticPattern {
            weak: 50_000,
            strong: 60_000,
            duration_ms: 260,
            priority: 3,
        },
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct HapticScheduler {
    active: Option<HapticPattern>,
}

impl HapticScheduler {
    fn request(&mut self, pattern: HapticPattern) -> bool {
        if self
            .active
            .is_some_and(|active| active.priority > pattern.priority)
        {
            return false;
        }
        self.active = Some(pattern);
        true
    }

    fn clear(&mut self) {
        self.active = None;
    }
}

struct HapticOutput {
    enabled: bool,
    unsupported: bool,
    effect: Option<Effect>,
    scheduler: HapticScheduler,
    active_until: Option<Instant>,
}

impl Default for HapticOutput {
    fn default() -> Self {
        Self {
            enabled: true,
            unsupported: false,
            effect: None,
            scheduler: HapticScheduler::default(),
            active_until: None,
        }
    }
}

impl HapticOutput {
    fn request(&mut self, gilrs: &mut Gilrs, gamepad: GamepadId, event: HapticEvent) {
        let pattern = haptic_pattern(event);
        let now = Instant::now();
        if self.active_until.is_some_and(|until| until <= now) {
            self.effect = None;
            self.active_until = None;
            self.scheduler.clear();
        }
        if !self.enabled || self.unsupported || !self.scheduler.request(pattern) {
            return;
        }
        if let Some(effect) = self.effect.take() {
            let _ = effect.stop();
        }
        let duration = Ticks::from_ms(pattern.duration_ms);
        let mut builder = EffectBuilder::new();
        if pattern.weak > 0 {
            builder.add_effect(BaseEffect {
                kind: BaseEffectType::Weak {
                    magnitude: pattern.weak,
                },
                scheduling: Replay {
                    after: Ticks::from_ms(0),
                    play_for: duration,
                    with_delay: Ticks::from_ms(0),
                },
                ..BaseEffect::default()
            });
        }
        if pattern.strong > 0 {
            builder.add_effect(BaseEffect {
                kind: BaseEffectType::Strong {
                    magnitude: pattern.strong,
                },
                scheduling: Replay {
                    after: Ticks::from_ms(0),
                    play_for: duration,
                    with_delay: Ticks::from_ms(0),
                },
                ..BaseEffect::default()
            });
        }
        let result = builder
            .gamepads(&[gamepad])
            .repeat(Repeat::For(duration))
            .finish(gilrs)
            .and_then(|effect| {
                effect.play()?;
                Ok(effect)
            });
        match result {
            Ok(effect) => {
                self.effect = Some(effect);
                self.active_until = Some(now + Duration::from_millis(pattern.duration_ms as u64));
            }
            Err(error) => {
                self.unsupported = true;
                self.scheduler.clear();
                eprintln!("vibración no disponible: {error}");
            }
        }
    }

    fn stop(&mut self) {
        if let Some(effect) = self.effect.take() {
            let _ = effect.stop();
        }
        self.active_until = None;
        self.scheduler.clear();
    }

    fn reset_for_reconnect(&mut self) {
        self.stop();
        self.unsupported = false;
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stick {
    pub x: f32,
    pub y: f32,
}

pub fn radial_deadzone(x: f32, y: f32, deadzone: f32) -> Stick {
    let x = if x.is_finite() { x } else { 0.0 };
    let y = if y.is_finite() { y } else { 0.0 };
    let magnitude = (x * x + y * y).sqrt();
    let deadzone = deadzone.clamp(0.0, 0.99);
    if magnitude <= deadzone {
        return Stick::default();
    }
    let normalized_magnitude = magnitude.min(1.0);
    let scale = ((normalized_magnitude - deadzone) / (1.0 - deadzone)) / magnitude;
    Stick {
        x: (x * scale).clamp(-1.0, 1.0),
        y: (y * scale).clamp(-1.0, 1.0),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControllerButton {
    South,
    East,
    North,
    West,
    RightTrigger,
    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ControllerFrame {
    pub connected: bool,
    pub left_stick: Stick,
    pub right_stick: Stick,
    pub pressed: Vec<ControllerButton>,
    pub released: Vec<ControllerButton>,
}

#[derive(Clone, Debug, Default)]
struct ControllerState {
    connected: bool,
    left_stick: Stick,
    right_stick: Stick,
    buttons: Vec<ControllerButton>,
    pressed: Vec<ControllerButton>,
    released: Vec<ControllerButton>,
}

impl ControllerState {
    fn begin_frame(&mut self) {
        self.pressed.clear();
        self.released.clear();
    }

    fn set_button(&mut self, button: ControllerButton, active: bool, emit_edge: bool) {
        let was_active = self.buttons.contains(&button);
        if active == was_active {
            return;
        }
        if active {
            self.buttons.push(button);
            if emit_edge {
                self.pressed.push(button);
            }
        } else {
            self.buttons.retain(|value| *value != button);
            if emit_edge {
                self.released.push(button);
            }
        }
    }

    fn neutral(&mut self) {
        self.connected = false;
        self.left_stick = Stick::default();
        self.right_stick = Stick::default();
        self.buttons.clear();
        self.pressed.clear();
        self.released.clear();
    }

    fn frame(&self) -> ControllerFrame {
        ControllerFrame {
            connected: self.connected,
            left_stick: self.left_stick,
            right_stick: self.right_stick,
            pressed: self.pressed.clone(),
            released: self.released.clone(),
        }
    }
}

pub struct GamepadInput {
    gilrs: Option<Gilrs>,
    active: Option<GamepadId>,
    state: ControllerState,
    deadzone: f32,
    haptics: HapticOutput,
}

impl GamepadInput {
    pub fn new() -> Self {
        match Gilrs::new() {
            Ok(gilrs) => Self {
                gilrs: Some(gilrs),
                active: None,
                state: ControllerState::default(),
                deadzone: DEFAULT_DEADZONE,
                haptics: HapticOutput::default(),
            },
            Err(error) => {
                eprintln!("mando no disponible: {error}");
                Self {
                    gilrs: None,
                    active: None,
                    state: ControllerState::default(),
                    deadzone: DEFAULT_DEADZONE,
                    haptics: HapticOutput::default(),
                }
            }
        }
    }

    pub fn poll(&mut self) -> ControllerFrame {
        self.state.begin_frame();
        let Some(gilrs) = self.gilrs.as_mut() else {
            self.state.neutral();
            return self.state.frame();
        };

        while let Some(event) = gilrs.next_event() {
            match event.event {
                EventType::Connected => {
                    let _ = event.id;
                }
                EventType::Disconnected if self.active == Some(event.id) => {
                    self.haptics.stop();
                    self.active = None;
                    self.state.neutral();
                }
                EventType::ButtonPressed(button, _) if self.active == Some(event.id) => {
                    if let Some(button) = map_button(button) {
                        self.state.set_button(button, true, true);
                    }
                }
                EventType::ButtonReleased(button, _) if self.active == Some(event.id) => {
                    if let Some(button) = map_button(button) {
                        self.state.set_button(button, false, true);
                    }
                }
                EventType::ButtonChanged(button, value, _)
                    if self.active == Some(event.id) && button == Button::RightTrigger =>
                {
                    self.state.set_button(
                        ControllerButton::RightTrigger,
                        value.is_finite() && value > 0.5,
                        true,
                    );
                }
                EventType::AxisChanged(axis, value, _) if self.active == Some(event.id) => {
                    Self::set_axis(&mut self.state, self.deadzone, axis, value);
                }
                _ => {}
            }
        }

        if self.active.is_none() {
            if let Some(id) = gilrs
                .gamepads()
                .map(|(id, _)| id)
                .min_by_key(|id| usize::from(*id))
            {
                self.active = Some(id);
                self.haptics.reset_for_reconnect();
                Self::sync_gamepad(&mut self.state, self.deadzone, gilrs, id, false);
            }
        }
        if let Some(id) = self.active {
            Self::sync_axes(&mut self.state, self.deadzone, gilrs, id);
            self.state.connected = gilrs.gamepad(id).is_connected();
            if !self.state.connected {
                self.haptics.stop();
                self.active = None;
                self.state.neutral();
            }
        } else {
            self.state.neutral();
        }
        self.state.frame()
    }

    pub fn neutralize(&mut self) {
        self.haptics.stop();
        self.active = None;
        self.state.neutral();
    }

    pub fn request_haptic(&mut self, event: HapticEvent) {
        let Some(id) = self.active else {
            return;
        };
        let Some(gilrs) = self.gilrs.as_mut() else {
            return;
        };
        self.haptics.request(gilrs, id, event);
    }

    #[allow(dead_code)]
    pub fn set_haptics_enabled(&mut self, enabled: bool) {
        self.haptics.enabled = enabled;
        if !enabled {
            self.haptics.stop();
        }
    }

    fn sync_gamepad(
        state: &mut ControllerState,
        deadzone: f32,
        gilrs: &Gilrs,
        id: GamepadId,
        emit_edges: bool,
    ) {
        let gamepad = gilrs.gamepad(id);
        state.connected = gamepad.is_connected();
        state.left_stick = radial_deadzone(
            gamepad.value(Axis::LeftStickX),
            gamepad.value(Axis::LeftStickY),
            deadzone,
        );
        state.right_stick = radial_deadzone(
            gamepad.value(Axis::RightStickX),
            gamepad.value(Axis::RightStickY),
            deadzone,
        );
        for (button, controller_button) in mapped_buttons() {
            state.set_button(controller_button, gamepad.is_pressed(button), emit_edges);
        }
    }

    fn sync_axes(state: &mut ControllerState, deadzone: f32, gilrs: &Gilrs, id: GamepadId) {
        let gamepad = gilrs.gamepad(id);
        state.left_stick = radial_deadzone(
            gamepad.value(Axis::LeftStickX),
            gamepad.value(Axis::LeftStickY),
            deadzone,
        );
        state.right_stick = radial_deadzone(
            gamepad.value(Axis::RightStickX),
            gamepad.value(Axis::RightStickY),
            deadzone,
        );
    }

    fn set_axis(state: &mut ControllerState, deadzone: f32, axis: Axis, value: f32) {
        let value = if value.is_finite() { value.clamp(-1.0, 1.0) } else { 0.0 };
        match axis {
            Axis::LeftStickX => state.left_stick.x = value,
            Axis::LeftStickY => state.left_stick.y = value,
            Axis::RightStickX => state.right_stick.x = value,
            Axis::RightStickY => state.right_stick.y = value,
            _ => {}
        }
        state.left_stick = radial_deadzone(
            state.left_stick.x,
            state.left_stick.y,
            deadzone,
        );
        state.right_stick = radial_deadzone(
            state.right_stick.x,
            state.right_stick.y,
            deadzone,
        );
    }
}

impl Default for GamepadInput {
    fn default() -> Self {
        Self::new()
    }
}

fn map_button(button: Button) -> Option<ControllerButton> {
    mapped_buttons()
        .into_iter()
        .find_map(|(source, target)| (source == button).then_some(target))
}

fn mapped_buttons() -> [(Button, ControllerButton); 9] {
    [
        (Button::South, ControllerButton::South),
        (Button::East, ControllerButton::East),
        (Button::North, ControllerButton::North),
        (Button::West, ControllerButton::West),
        (Button::RightTrigger, ControllerButton::RightTrigger),
        (Button::DPadUp, ControllerButton::DPadUp),
        (Button::DPadDown, ControllerButton::DPadDown),
        (Button::DPadLeft, ControllerButton::DPadLeft),
        (Button::DPadRight, ControllerButton::DPadRight),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn haptic_patterns_are_finite_and_moderate() {
        for event in [
            HapticEvent::AttackExecuted,
            HapticEvent::EnemyDefeated,
            HapticEvent::ItemCollected,
            HapticEvent::Impact,
        ] {
            let pattern = haptic_pattern(event);
            assert!(pattern.duration_ms > 0 && pattern.duration_ms <= 300);
            assert!(pattern.weak <= 60_000 && pattern.strong <= 60_000);
        }
    }

    #[test]
    fn higher_priority_pattern_replaces_lower_priority_pattern() {
        let mut scheduler = HapticScheduler::default();
        assert!(scheduler.request(haptic_pattern(HapticEvent::EnemyDefeated)));
        assert!(!scheduler.request(haptic_pattern(HapticEvent::ItemCollected)));
        assert!(scheduler.request(haptic_pattern(HapticEvent::Impact)));
        scheduler.clear();
        assert!(scheduler.request(haptic_pattern(HapticEvent::ItemCollected)));
    }

    #[test]
    fn haptics_can_be_disabled_without_hardware() {
        let mut input = GamepadInput::new();
        input.set_haptics_enabled(false);
        input.set_haptics_enabled(true);
    }

    #[test]
    fn radial_deadzone_removes_drift_and_preserves_direction() {
        assert_eq!(radial_deadzone(0.1, 0.1, 0.18), Stick::default());
        let value = radial_deadzone(0.0, -1.0, 0.18);
        assert!(value.y < -0.99);
        assert_eq!(value.x, 0.0);
    }

    #[test]
    fn non_finite_and_out_of_range_values_are_safe() {
        let value = radial_deadzone(f32::NAN, f32::INFINITY, 0.18);
        assert_eq!(value, Stick::default());
        let value = radial_deadzone(4.0, 0.0, 0.18);
        assert!(value.x <= 1.0);
    }

    #[test]
    fn diagonal_input_is_normalized_to_the_unit_circle() {
        let value = radial_deadzone(1.0, 1.0, 0.18);
        let magnitude = (value.x * value.x + value.y * value.y).sqrt();
        assert!(magnitude <= 1.0);
        assert!((value.x - value.y).abs() < f32::EPSILON);
    }

    #[test]
    fn button_edges_are_emitted_once_and_release_only_their_button() {
        let mut state = ControllerState::default();
        state.connected = true;
        state.set_button(ControllerButton::South, true, true);
        state.set_button(ControllerButton::East, true, true);
        assert_eq!(state.pressed, vec![ControllerButton::South, ControllerButton::East]);
        state.begin_frame();
        state.set_button(ControllerButton::South, false, true);
        assert_eq!(state.released, vec![ControllerButton::South]);
        assert!(state.buttons.contains(&ControllerButton::East));
    }

    #[test]
    fn neutral_state_clears_axes_buttons_and_edges() {
        let mut state = ControllerState::default();
        state.connected = true;
        state.left_stick = Stick { x: 1.0, y: 0.0 };
        state.set_button(ControllerButton::South, true, true);
        state.neutral();
        assert_eq!(state.frame(), ControllerFrame::default());
    }
}
