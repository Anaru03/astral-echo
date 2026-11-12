use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufReader, Cursor};
use std::num::{NonZeroU16, NonZeroU32};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use rodio::{buffer::SamplesBuffer, Decoder, DeviceSinkBuilder, MixerDeviceSink, Player, Source};

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
    ShipBoost,
    ShipBrake,
    ShipTurn,
    DestinationSelect,
    ShieldOn,
    ShieldOff,
    ShieldHit,
    WarningAlert,
    Takeoff,
    AutopilotEngage,
    AutopilotArrival,
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
    fn clamp_volume(volume: f32) -> f32 {
        if volume.is_finite() {
            volume.clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    fn clamped(self) -> Self {
        Self {
            master_volume: Self::clamp_volume(self.master_volume),
            music_volume: Self::clamp_volume(self.music_volume),
            effects_volume: Self::clamp_volume(self.effects_volume),
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
    playlist_index: Option<usize>,
}

struct RodioOutput {
    device: MixerDeviceSink,
    music: Option<MusicPlayback>,
    effects: Vec<Player>,
}

#[derive(Clone, Copy)]
enum PendingAudioTarget {
    Music { track: MusicTrack, looping: bool },
    Playlist { index: usize },
    Effect(SoundEffect),
}

struct LoadedAudio {
    target: PendingAudioTarget,
    bytes: Result<Vec<u8>, AudioError>,
}

fn audio_asset_path(path: impl AsRef<Path>) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/audio")
        .join(path)
}

fn decode_pcm_wav(bytes: &[u8]) -> Result<SamplesBuffer, AudioError> {
    if bytes.get(0..4) != Some(&b"RIFF"[..]) || bytes.get(8..12) != Some(&b"WAVE"[..]) {
        return Err(AudioError::LoadFailed);
    }

    let mut offset = 12usize;
    let mut format = None;
    let mut data = None;
    while offset.checked_add(8).is_some_and(|end| end <= bytes.len()) {
        let chunk_id = &bytes[offset..offset + 4];
        let chunk_size = u32::from_le_bytes(
            bytes[offset + 4..offset + 8]
                .try_into()
                .map_err(|_| AudioError::LoadFailed)?,
        ) as usize;
        let chunk_start = offset + 8;
        let chunk_end = chunk_start
            .checked_add(chunk_size)
            .filter(|end| *end <= bytes.len())
            .ok_or(AudioError::LoadFailed)?;
        match chunk_id {
            b"fmt " if chunk_size >= 16 => {
                let chunk = &bytes[chunk_start..chunk_end];
                format = Some((
                    u16::from_le_bytes([chunk[0], chunk[1]]),
                    u16::from_le_bytes([chunk[2], chunk[3]]),
                    u32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]),
                    u16::from_le_bytes([chunk[14], chunk[15]]),
                ));
            }
            b"data" => data = Some(&bytes[chunk_start..chunk_end]),
            _ => {}
        }
        offset = chunk_end.saturating_add(chunk_size & 1);
    }

    let (encoding, channels, sample_rate, bits_per_sample) =
        format.ok_or(AudioError::LoadFailed)?;
    let samples = data.ok_or(AudioError::LoadFailed)?;
    if encoding != 1 || bits_per_sample != 16 || samples.len() % 2 != 0 {
        return Err(AudioError::LoadFailed);
    }
    let channels = NonZeroU16::new(channels).ok_or(AudioError::LoadFailed)?;
    let sample_rate = NonZeroU32::new(sample_rate).ok_or(AudioError::LoadFailed)?;
    if samples.len() % (channels.get() as usize * 2) != 0 {
        return Err(AudioError::LoadFailed);
    }
    let decoded = samples
        .chunks_exact(2)
        .map(|sample| i16::from_le_bytes([sample[0], sample[1]]) as f32 / 32768.0)
        .collect::<Vec<_>>();
    Ok(SamplesBuffer::new(channels, sample_rate, decoded))
}

fn discover_music_playlist() -> Vec<PathBuf> {
    let directory = audio_asset_path("music");
    let mut tracks = std::fs::read_dir(&directory)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("mp3"))
        })
        .collect::<Vec<_>>();
    let legacy_track = audio_asset_path("exploration.mp3");
    if legacy_track.is_file() && !tracks.contains(&legacy_track) {
        tracks.push(legacy_track);
    }
    tracks.sort();
    tracks
}

impl RodioOutput {
    fn open() -> Result<Self, AudioError> {
        let mut device =
            DeviceSinkBuilder::open_default_sink().map_err(|_| AudioError::Unavailable)?;
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
    playlist: Vec<PathBuf>,
    current_music_index: Option<usize>,
    playlist_loading: bool,
    config: AudioConfig,
    focused: bool,
    music_paused: bool,
    closed: bool,
    reported_errors: HashSet<SoundEffect>,
    load_tx: Sender<LoadedAudio>,
    load_rx: Receiver<LoadedAudio>,
    pub(crate) event_cursor: AudioEventCursor,
}

impl AudioManager {
    pub fn new() -> Self {
        let (load_tx, load_rx) = mpsc::channel();
        let output = match RodioOutput::open() {
            Ok(output) => Some(output),
            Err(error) => {
                eprintln!("audio: salida de audio no disponible ({error:?})");
                None
            }
        };
        Self {
            output,
            resources: AudioResources {
                music: [(MusicTrack::Exploration, audio_asset_path("exploration.mp3"))]
                    .into_iter()
                    .collect(),
                effects: [
                    (SoundEffect::PlayerAttack, "sfx/attack.wav"),
                    (SoundEffect::EnemyDefeated, "sfx/enemy_defeated.wav"),
                    (SoundEffect::Collect, "sfx/collect.wav"),
                    (SoundEffect::Scan, "sfx/scan.wav"),
                    (SoundEffect::Landing, "sfx/landing.wav"),
                    (SoundEffect::PlayerImpacted, "sfx/impact.wav"),
                    (SoundEffect::GameOver, "sfx/game_over.wav"),
                    (SoundEffect::ShipPropulsion, "sfx/ship_propulsion.wav"),
                    (SoundEffect::ShipBoost, "sfx/ship_boost.wav"),
                    (SoundEffect::ShipBrake, "sfx/ship_brake.wav"),
                    (SoundEffect::ShipTurn, "sfx/turn.wav"),
                    (SoundEffect::DestinationSelect, "sfx/destination_select.wav"),
                    (SoundEffect::ShieldOn, "sfx/shield_on.wav"),
                    (SoundEffect::ShieldOff, "sfx/shield_off.wav"),
                    (SoundEffect::ShieldHit, "sfx/shield_hit.wav"),
                    (SoundEffect::WarningAlert, "sfx/warning_alert.wav"),
                    (SoundEffect::Takeoff, "sfx/takeoff.wav"),
                    (SoundEffect::AutopilotEngage, "sfx/autopilot_engage.wav"),
                    (SoundEffect::AutopilotArrival, "sfx/autopilot_arrival.wav"),
                ]
                .into_iter()
                .map(|(effect, path)| (effect, audio_asset_path(path)))
                .collect(),
            },
            playlist: Vec::new(),
            current_music_index: None,
            playlist_loading: false,
            config: AudioConfig::default(),
            focused: true,
            music_paused: false,
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
        self.playlist = discover_music_playlist();
        if self.playlist.is_empty() {
            self.queue_music(MusicTrack::Exploration, true);
        } else {
            self.queue_playlist_track(0);
        }
    }

    pub fn toggle_music_pause(&mut self) {
        if self.music_paused {
            self.music_paused = false;
            self.resume_music();
        } else {
            self.music_paused = true;
            self.pause_music();
        }
    }

    pub fn toggle_muted(&mut self) {
        self.set_muted(!self.config.muted);
    }

    pub fn next_music(&mut self) {
        self.playlist = discover_music_playlist();
        if self.playlist.is_empty() || self.playlist_loading {
            return;
        }
        let next = self
            .current_music_index
            .map_or(0, |index| (index + 1) % self.playlist.len());
        self.queue_playlist_track(next);
    }

    pub fn register_music(&mut self, track: MusicTrack, path: impl Into<PathBuf>) {
        self.resources.music.insert(track, path.into());
    }

    pub fn register_effect(&mut self, effect: SoundEffect, path: impl Into<PathBuf>) {
        self.resources.effects.insert(effect, path.into());
    }

    pub fn set_master_volume(&mut self, volume: f32) {
        self.config.master_volume = AudioConfig::clamp_volume(volume);
        self.apply_volumes();
    }

    pub fn set_music_volume(&mut self, volume: f32) {
        self.config.music_volume = AudioConfig::clamp_volume(volume);
        self.apply_volumes();
    }

    pub fn set_effects_volume(&mut self, volume: f32) {
        self.config.effects_volume = AudioConfig::clamp_volume(volume);
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
        if let Some(previous) = output.music.replace(MusicPlayback {
            player,
            playlist_index: None,
        }) {
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
        if let Some(previous) = output.music.replace(MusicPlayback {
            player,
            playlist_index: None,
        }) {
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
        if self.focused && !self.music_paused {
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
                    if matches!(loaded.target, PendingAudioTarget::Playlist { .. }) {
                        self.playlist_loading = false;
                        self.playlist.clear();
                    }
                    let effect = match loaded.target {
                        PendingAudioTarget::Effect(effect) => Some(effect),
                        PendingAudioTarget::Music { .. } | PendingAudioTarget::Playlist { .. } => {
                            None
                        }
                    };
                    self.report_error(error, effect);
                }
            }
        }
        if let Some(output) = &mut self.output {
            output.effects.retain(|effect| !effect.empty());
        }
        if !self.playlist_loading {
            let ended_index = self.output.as_ref().and_then(|output| {
                output
                    .music
                    .as_ref()
                    .and_then(|music| music.playlist_index.filter(|_| music.player.empty()))
            });
            if let Some(index) = ended_index {
                if !self.playlist.is_empty() {
                    self.queue_playlist_track((index + 1) % self.playlist.len());
                }
            }
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

    fn queue_playlist_track(&mut self, index: usize) {
        if self.closed || self.output.is_none() || self.playlist_loading {
            return;
        }
        let Some(path) = self.playlist.get(index).cloned() else {
            return;
        };
        self.playlist_loading = true;
        self.queue_load(PendingAudioTarget::Playlist { index }, path);
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
                self.playlist_loading = false;
                let result = if looping {
                    Decoder::new_looped(Cursor::new(bytes)).map(|source| {
                        let Some(output) = self.output.as_mut() else {
                            return;
                        };
                        let player = Player::connect_new(output.device.mixer());
                        player.set_volume(self.config.music_gain());
                        player.append(source);
                        if self.music_paused || !self.focused {
                            player.pause();
                        }
                        if let Some(previous) = output.music.replace(MusicPlayback {
                            player,
                            playlist_index: None,
                        }) {
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
                        if self.music_paused || !self.focused {
                            player.pause();
                        }
                        if let Some(previous) = output.music.replace(MusicPlayback {
                            player,
                            playlist_index: None,
                        }) {
                            previous.player.stop();
                        }
                    })
                };
                if result.is_err() {
                    self.report_error(AudioError::LoadFailed, None);
                }
            }
            PendingAudioTarget::Playlist { index } => {
                self.playlist_loading = false;
                let result = Decoder::new(Cursor::new(bytes)).map(|source| {
                    let Some(output) = self.output.as_mut() else {
                        return;
                    };
                    let player = Player::connect_new(output.device.mixer());
                    player.set_volume(self.config.music_gain());
                    player.append(source);
                    if self.music_paused || !self.focused {
                        player.pause();
                    }
                    if let Some(previous) = output.music.replace(MusicPlayback {
                        player,
                        playlist_index: Some(index),
                    }) {
                        previous.player.stop();
                    }
                    self.current_music_index = Some(index);
                });
                if result.is_err() {
                    self.playlist.clear();
                    self.report_error(AudioError::LoadFailed, None);
                }
            }
            PendingAudioTarget::Effect(effect) => {
                if bytes.get(0..4) == Some(&b"RIFF"[..]) {
                    match decode_pcm_wav(&bytes) {
                        Ok(source) => self.append_effect_source(source),
                        Err(error) => self.report_error(error, Some(effect)),
                    }
                } else {
                    match Decoder::new(BufReader::new(Cursor::new(bytes))) {
                        Ok(source) => self.append_effect_source(source),
                        Err(_) => self.report_error(AudioError::LoadFailed, Some(effect)),
                    }
                }
            }
        }
    }

    fn append_effect_source<S>(&mut self, source: S)
    where
        S: Source + Send + 'static,
    {
        if let Some(output) = &mut self.output {
            let player = Player::connect_new(output.device.mixer());
            player.set_volume(self.config.effects_gain());
            player.append(source);
            output.effects.push(player);
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
    fn config_replaces_non_finite_volumes_with_silence() {
        let config = normalize_config(AudioConfig {
            master_volume: f32::NAN,
            music_volume: f32::INFINITY,
            effects_volume: f32::NEG_INFINITY,
            muted: false,
        });
        assert_eq!(config.master_volume, 0.0);
        assert_eq!(config.music_volume, 0.0);
        assert_eq!(config.effects_volume, 0.0);
    }

    #[test]
    fn default_catalog_uses_the_expected_resource_paths() {
        let audio = AudioManager::new();

        assert_eq!(
            audio.resources.music.get(&MusicTrack::Exploration),
            Some(&audio_asset_path("exploration.mp3"))
        );
        let expected_effects = [
            (SoundEffect::PlayerAttack, "sfx/attack.wav"),
            (SoundEffect::EnemyDefeated, "sfx/enemy_defeated.wav"),
            (SoundEffect::Collect, "sfx/collect.wav"),
            (SoundEffect::Scan, "sfx/scan.wav"),
            (SoundEffect::Landing, "sfx/landing.wav"),
            (SoundEffect::PlayerImpacted, "sfx/impact.wav"),
            (SoundEffect::GameOver, "sfx/game_over.wav"),
            (SoundEffect::ShipPropulsion, "sfx/ship_propulsion.wav"),
            (SoundEffect::ShipBoost, "sfx/ship_boost.wav"),
            (SoundEffect::ShipBrake, "sfx/ship_brake.wav"),
            (SoundEffect::ShipTurn, "sfx/turn.wav"),
            (SoundEffect::DestinationSelect, "sfx/destination_select.wav"),
            (SoundEffect::ShieldOn, "sfx/shield_on.wav"),
            (SoundEffect::ShieldOff, "sfx/shield_off.wav"),
            (SoundEffect::ShieldHit, "sfx/shield_hit.wav"),
            (SoundEffect::WarningAlert, "sfx/warning_alert.wav"),
            (SoundEffect::Takeoff, "sfx/takeoff.wav"),
            (SoundEffect::AutopilotEngage, "sfx/autopilot_engage.wav"),
            (SoundEffect::AutopilotArrival, "sfx/autopilot_arrival.wav"),
        ];
        for (effect, path) in expected_effects {
            assert_eq!(
                audio.resources.effects.get(&effect),
                Some(&audio_asset_path(path))
            );
        }
    }

    #[test]
    fn every_default_sound_effect_has_a_decodable_audio_file() {
        let audio = AudioManager::new();
        for (effect, path) in &audio.resources.effects {
            let bytes = std::fs::read(path).unwrap_or_else(|error| {
                panic!(
                    "could not open {effect:?} audio at {}: {error}",
                    path.display()
                )
            });
            decode_pcm_wav(&bytes).unwrap_or_else(|error| {
                panic!(
                    "could not decode {effect:?} audio at {}: {error:?}",
                    path.display()
                )
            });
        }
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
            playlist: Vec::new(),
            current_music_index: None,
            playlist_loading: false,
            config: AudioConfig::default(),
            focused: true,
            music_paused: false,
            closed: false,
            reported_errors: HashSet::new(),
            load_tx: mpsc::channel().0,
            load_rx: mpsc::channel().1,
            event_cursor: AudioEventCursor::new(),
        };
        assert_eq!(
            audio.play_music(MusicTrack::Exploration, true),
            Err(AudioError::MissingResource)
        );
        assert_eq!(
            audio.play_effect(SoundEffect::PlayerAttack),
            Err(AudioError::MissingResource)
        );
        audio.register_music(MusicTrack::Exploration, "missing.wav");
        assert_eq!(
            audio.play_music(MusicTrack::Exploration, true),
            Err(AudioError::Unavailable)
        );
    }

    #[test]
    fn lifecycle_and_focus_are_safe_without_a_device() {
        let mut audio = AudioManager {
            output: None,
            resources: AudioResources::default(),
            playlist: Vec::new(),
            current_music_index: None,
            playlist_loading: false,
            config: AudioConfig::default(),
            focused: true,
            music_paused: false,
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
