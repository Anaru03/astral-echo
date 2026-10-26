use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufReader, Cursor};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MusicTrack {
    Exploration,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SoundEffect {
    PlayerAttack,
    EnemyDefeated,
    Collect,
    Scan,
    Landing,
    PlayerImpacted,
    GameOver,
    ShipPropulsion,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AudioEvent {
    pub id: u64,
    pub effect: SoundEffect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AudioError {
    Unavailable,
    MissingResource,
    LoadFailed,
    Closed,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AudioConfig {
    pub master_volume: f32,
    pub music_volume: f32,
    pub effects_volume: f32,
    pub muted: bool,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            master_volume: 1.0,
            music_volume: 1.0,
            effects_volume: 1.0,
            muted: false,
        }
    }
}

impl AudioConfig {
    fn clamped(self) -> Self {
        Self {
            master_volume: self.master_volume.clamp(0.0, 1.0),
            music_volume: self.music_volume.clamp(0.0, 1.0),
            effects_volume: self.effects_volume.clamp(0.0, 1.0),
            ..self
        }
    }

    fn music_gain(self) -> f32 {
        if self.muted {
            0.0
        } else {
            self.master_volume * self.music_volume
        }
    }

    fn effects_gain(self) -> f32 {
        if self.muted {
            0.0
        } else {
            self.master_volume * self.effects_volume
        }
    }
}

#[derive(Default)]
struct AudioResources {
    music: HashMap<MusicTrack, PathBuf>,
    effects: HashMap<SoundEffect, PathBuf>,
}

struct MusicPlayback {
    player: Player,
}

struct RodioOutput {
    device: MixerDeviceSink,
    music: Option<MusicPlayback>,
    effects: Vec<Player>,
}

#[derive(Clone, Copy)]
enum PendingAudioTarget {
    Music { track: MusicTrack, looping: bool },
    Effect(SoundEffect),
}

struct LoadedAudio {
    target: PendingAudioTarget,
    bytes: Result<Vec<u8>, AudioError>,
}

impl RodioOutput {
    fn open() -> Result<Self, AudioError> {
        let mut device = DeviceSinkBuilder::open_default_sink()
            .map_err(|_| AudioError::Unavailable)?;
        device.log_on_drop(false);
        Ok(Self {
            device,
            music: None,
            effects: Vec::new(),
        })
    }

    fn stop(&mut self) {
        if let Some(music) = self.music.take() {
            music.player.stop();
        }
        for effect in &self.effects {
            effect.stop();
        }
        self.effects.clear();
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AudioEventCursor {
    last_id: u64,
}

impl AudioEventCursor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn observe(&mut self, event_id: u64) -> bool {
        if event_id <= self.last_id {
            return false;
        }
        self.last_id = event_id;
        true
    }

    pub fn discard_through(&mut self, event_id: u64) {
        self.last_id = self.last_id.max(event_id);
    }
}

pub struct AudioManager {
    output: Option<RodioOutput>,
    resources: AudioResources,
    config: AudioConfig,
    focused: bool,
    closed: bool,
    reported_errors: HashSet<SoundEffect>,
    load_tx: Sender<LoadedAudio>,
    load_rx: Receiver<LoadedAudio>,
    pub(crate) event_cursor: AudioEventCursor,
}

impl AudioManager {
    pub fn new() -> Self {
        let (load_tx, load_rx) = mpsc::channel();
        Self {
            output: RodioOutput::open().ok(),
            resources: AudioResources {
                music: [(MusicTrack::Exploration, PathBuf::from("assets/audio/exploration.ogg"))]
                    .into_iter()
                    .collect(),
                effects: [
                    (SoundEffect::PlayerAttack, "assets/audio/player_attack.wav"),
                    (SoundEffect::EnemyDefeated, "assets/audio/enemy_defeated.wav"),
                    (SoundEffect::Collect, "assets/audio/collect.wav"),
                    (SoundEffect::Scan, "assets/audio/scan.wav"),
                    (SoundEffect::Landing, "assets/audio/landing.wav"),
                    (SoundEffect::PlayerImpacted, "assets/audio/player_impacted.wav"),
                    (SoundEffect::GameOver, "assets/audio/game_over.wav"),
                    (SoundEffect::ShipPropulsion, "assets/audio/ship_propulsion.wav"),
                ]
                .into_iter()
                .map(|(effect, path)| (effect, PathBuf::from(path)))
                .collect(),
            },
            config: AudioConfig::default(),
            focused: true,
            closed: false,
            reported_errors: HashSet::new(),
            load_tx,
            load_rx,
            event_cursor: AudioEventCursor::new(),
        }
    }

    pub fn available(&self) -> bool {
        self.output.is_some() && !self.closed
    }

    pub fn config(&self) -> AudioConfig {
        self.config
    }

    pub fn request_exploration_music(&mut self) {
        self.queue_music(MusicTrack::Exploration, true);
    }

    pub fn register_music(&mut self, track: MusicTrack, path: impl Into<PathBuf>) {
        self.resources.music.insert(track, path.into());
    }

    pub fn register_effect(&mut self, effect: SoundEffect, path: impl Into<PathBuf>) {
        self.resources.effects.insert(effect, path.into());
    }

    pub fn set_master_volume(&mut self, volume: f32) {
        self.config.master_volume = volume.clamp(0.0, 1.0);
        self.apply_volumes();
    }

    pub fn set_music_volume(&mut self, volume: f32) {
        self.config.music_volume = volume.clamp(0.0, 1.0);
        self.apply_volumes();
    }

    pub fn set_effects_volume(&mut self, volume: f32) {
        self.config.effects_volume = volume.clamp(0.0, 1.0);
        self.apply_volumes();
    }

    pub fn set_muted(&mut self, muted: bool) {
        self.config.muted = muted;
        self.apply_volumes();
    }

    pub fn play_music(&mut self, track: MusicTrack, looping: bool) -> Result<(), AudioError> {
        if self.closed {
            return Err(AudioError::Closed);
        }
        let path = self
            .resources
            .music
            .get(&track)
            .ok_or(AudioError::MissingResource)?
            .clone();
        let output = self.output.as_mut().ok_or(AudioError::Unavailable)?;
        let file = File::open(path).map_err(|_| AudioError::LoadFailed)?;
        let reader = BufReader::new(file);
        let source = if looping {
            Decoder::new_looped(reader).map_err(|_| AudioError::LoadFailed)?
        } else {
            return Self::play_non_looping_music(output, reader, self.config.music_gain());
        };
        let player = Player::connect_new(output.device.mixer());
        player.set_volume(self.config.music_gain());
        player.append(source);
        if let Some(previous) = output.music.replace(MusicPlayback { player }) {
            previous.player.stop();
        }
        Ok(())
    }

    fn play_non_looping_music(
        output: &mut RodioOutput,
        reader: BufReader<File>,
        volume: f32,
    ) -> Result<(), AudioError> {
        let source = Decoder::new(reader).map_err(|_| AudioError::LoadFailed)?;
        let player = Player::connect_new(output.device.mixer());
        player.set_volume(volume);
        player.append(source);
        if let Some(previous) = output.music.replace(MusicPlayback { player }) {
            previous.player.stop();
        }
        Ok(())
    }

    pub fn stop_music(&mut self) {
        if let Some(output) = &mut self.output {
            if let Some(music) = output.music.take() {
                music.player.stop();
            }
        }
    }

    pub fn pause_music(&mut self) {
        if let Some(output) = &self.output {
            if let Some(music) = &output.music {
                music.player.pause();
            }
        }
    }

    pub fn resume_music(&mut self) {
        if self.focused {
            if let Some(output) = &self.output {
                if let Some(music) = &output.music {
                    music.player.play();
                }
            }
        }
    }

    pub fn play_effect(&mut self, effect: SoundEffect) -> Result<(), AudioError> {
        if self.closed {
            return Err(AudioError::Closed);
        }
        let path = self
            .resources
            .effects
            .get(&effect)
            .ok_or(AudioError::MissingResource)?
            .clone();
        let output = self.output.as_mut().ok_or(AudioError::Unavailable)?;
        let file = File::open(path).map_err(|_| AudioError::LoadFailed)?;
        let source = Decoder::new(BufReader::new(file)).map_err(|_| AudioError::LoadFailed)?;
        let player = Player::connect_new(output.device.mixer());
        player.set_volume(self.config.effects_gain());
        player.append(source);
        output.effects.push(player);
        Ok(())
    }

    pub fn dispatch_event(&mut self, event: AudioEvent) {
        if !self.event_cursor.observe(event.id) {
            return;
        }
        self.request_effect(event.effect);
    }

    pub fn request_effect(&mut self, effect: SoundEffect) {
        if self.closed {
            return;
        }
        let Some(path) = self.resources.effects.get(&effect).cloned() else {
            self.report_error(AudioError::MissingResource, Some(effect));
            return;
        };
        self.queue_load(PendingAudioTarget::Effect(effect), path);
    }

    pub fn on_focus(&mut self, focused: bool) {
        self.focused = focused;
        if focused {
            self.resume_music();
        } else {
            self.pause_music();
        }
    }

    pub fn poll(&mut self) {
        while let Ok(loaded) = self.load_rx.try_recv() {
            match loaded.bytes {
                Ok(bytes) => self.finish_load(loaded.target, bytes),
                Err(error) => {
                    let effect = match loaded.target {
                        PendingAudioTarget::Effect(effect) => Some(effect),
                        PendingAudioTarget::Music { .. } => None,
                    };
                    self.report_error(error, effect);
                }
            }
        }
        if let Some(output) = &mut self.output {
            output.effects.retain(|effect| !effect.empty());
        }
    }

    pub fn shutdown(&mut self) {
        if self.closed {
            return;
        }
        if let Some(output) = &mut self.output {
            output.stop();
        }
        self.output = None;
        self.closed = true;
    }

    fn apply_volumes(&mut self) {
        if let Some(output) = &mut self.output {
            if let Some(music) = &output.music {
                music.player.set_volume(self.config.music_gain());
            }
            for effect in &output.effects {
                effect.set_volume(self.config.effects_gain());
            }
        }
    }

    fn queue_music(&mut self, track: MusicTrack, looping: bool) {
        if self.closed {
            return;
        }
        let Some(path) = self.resources.music.get(&track).cloned() else {
            self.report_error(AudioError::MissingResource, None);
            return;
        };
        self.queue_load(PendingAudioTarget::Music { track, looping }, path);
    }

    fn queue_load(&self, target: PendingAudioTarget, path: PathBuf) {
        if self.output.is_none() {
            return;
        }
        let sender = self.load_tx.clone();
        thread::spawn(move || {
            let bytes = std::fs::read(path).map_err(|_| AudioError::LoadFailed);
            let _ = sender.send(LoadedAudio { target, bytes });
        });
    }

    fn finish_load(&mut self, target: PendingAudioTarget, bytes: Vec<u8>) {
        match target {
            PendingAudioTarget::Music { looping, .. } => {
                let result = if looping {
                    Decoder::new_looped(Cursor::new(bytes)).map(|source| {
                        let Some(output) = self.output.as_mut() else {
                            return;
                        };
                        let player = Player::connect_new(output.device.mixer());
                        player.set_volume(self.config.music_gain());
                        player.append(source);
                        if let Some(previous) = output.music.replace(MusicPlayback { player }) {
                            previous.player.stop();
                        }
                    })
                } else {
                    Decoder::new(Cursor::new(bytes)).map(|source| {
                        let Some(output) = self.output.as_mut() else {
                            return;
                        };
                        let player = Player::connect_new(output.device.mixer());
                        player.set_volume(self.config.music_gain());
                        player.append(source);
                        if let Some(previous) = output.music.replace(MusicPlayback { player }) {
                            previous.player.stop();
                        }
                    })
                };
                if result.is_err() {
                    self.report_error(AudioError::LoadFailed, None);
                }
            }
            PendingAudioTarget::Effect(effect) => {
                match Decoder::new(BufReader::new(Cursor::new(bytes))) {
                    Ok(source) => {
                        if let Some(output) = &mut self.output {
                            let player = Player::connect_new(output.device.mixer());
                            player.set_volume(self.config.effects_gain());
                            player.append(source);
                            output.effects.push(player);
                        }
                    }
                    Err(_) => self.report_error(AudioError::LoadFailed, Some(effect)),
                }
            }
        }
    }

    fn report_error(&mut self, error: AudioError, effect: Option<SoundEffect>) {
        if let Some(effect) = effect {
            if !self.reported_errors.insert(effect) {
                return;
            }
        }
        eprintln!("audio: {:?}", error);
    }
}

impl Drop for AudioManager {
    fn drop(&mut self) {
        self.shutdown();
    }
}

pub(crate) fn normalize_config(config: AudioConfig) -> AudioConfig {
    config.clamped()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_clamps_volumes_and_applies_mute() {
        let mut config = normalize_config(AudioConfig {
            master_volume: 2.0,
            music_volume: -1.0,
            effects_volume: 0.25,
            muted: false,
        });
        assert_eq!(config.master_volume, 1.0);
        assert_eq!(config.music_volume, 0.0);
        assert_eq!(config.effects_gain(), 0.25);
        config.muted = true;
        assert_eq!(config.music_gain(), 0.0);
        assert_eq!(config.effects_gain(), 0.0);
    }

    #[test]
    fn cursor_delivers_each_event_only_once() {
        let mut cursor = AudioEventCursor::new();
        assert!(cursor.observe(3));
        assert!(!cursor.observe(3));
        assert!(!cursor.observe(2));
        assert!(cursor.observe(4));
        cursor.discard_through(8);
        assert!(!cursor.observe(8));
        assert!(cursor.observe(9));
    }

    #[test]
    fn missing_resources_are_reported_without_audio_output() {
        let mut audio = AudioManager {
            output: None,
            resources: AudioResources::default(),
            config: AudioConfig::default(),
            focused: true,
            closed: false,
            reported_errors: HashSet::new(),
            load_tx: mpsc::channel().0,
            load_rx: mpsc::channel().1,
            event_cursor: AudioEventCursor::new(),
        };
        assert_eq!(audio.play_music(MusicTrack::Exploration, true), Err(AudioError::MissingResource));
        assert_eq!(audio.play_effect(SoundEffect::PlayerAttack), Err(AudioError::MissingResource));
        audio.register_music(MusicTrack::Exploration, "missing.wav");
        assert_eq!(audio.play_music(MusicTrack::Exploration, true), Err(AudioError::Unavailable));
    }

    #[test]
    fn lifecycle_and_focus_are_safe_without_a_device() {
        let mut audio = AudioManager {
            output: None,
            resources: AudioResources::default(),
            config: AudioConfig::default(),
            focused: true,
            closed: false,
            reported_errors: HashSet::new(),
            load_tx: mpsc::channel().0,
            load_rx: mpsc::channel().1,
            event_cursor: AudioEventCursor::new(),
        };
        audio.on_focus(false);
        audio.on_focus(true);
        audio.poll();
        audio.shutdown();
        audio.shutdown();
        assert!(!audio.available());
    }
}