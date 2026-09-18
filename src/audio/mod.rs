//! Audio: one-shot SFX via events, per-state looping music, and user
//! volume settings.
//!
//! Gameplay systems emit `SfxEvent`s — they do not know about asset handles
//! or playback settings. `play_sfx_events` is the single consumer that maps
//! a kind to an `AudioSource` and spawns a `PlaybackSettings::DESPAWN` entity
//! so the audio node cleans itself up once the clip ends.
//!
//! Music is a single entity tagged `MusicTrack`. State transitions do not
//! swap it directly — they only record which `MusicCue` the game wants, and
//! `apply_music_cue` makes the running track match on the next frame. The
//! indirection matters because the initial `OnEnter(MainMenu)` fires before
//! `Startup`, i.e. before `AudioAssets` exists: swapping there silently did
//! nothing and the menu stayed mute until the first level loaded. It also
//! means replaying a level, or moving from level 1 to 2, keeps the same
//! track rolling instead of restarting it. Changing `AudioSettings.music`
//! re-applies the volume to the running sink.

use bevy::audio::{AudioSink, AudioSinkPlayback, Volume};
use bevy::prelude::*;

use crate::game_state::{CurrentLevel, GameState, Level, LevelState};

/// Pygame `Mixer` caps: music max 0.8, sounds max 1.0, both in 0.1 steps.
pub const MAX_MUSIC_VOLUME: f32 = 0.8;
pub const MAX_SFX_VOLUME: f32 = 1.0;
pub const VOLUME_STEP: f32 = 0.1;

/// User-facing volume settings (edited from the Options screen).
#[derive(Resource, Debug, Clone, Copy)]
pub struct AudioSettings {
    pub music: f32,
    pub sfx: f32,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            music: MAX_MUSIC_VOLUME,
            sfx: MAX_SFX_VOLUME,
        }
    }
}

impl AudioSettings {
    /// Options screen shows `int(volume * 10)` like the original.
    pub fn music_steps(&self) -> u8 {
        (self.music * 10.0).round() as u8
    }
    pub fn sfx_steps(&self) -> u8 {
        (self.sfx * 10.0).round() as u8
    }
    pub fn adjust_music(&mut self, delta: f32) {
        self.music = snap_step(self.music + delta, MAX_MUSIC_VOLUME);
    }
    pub fn adjust_sfx(&mut self, delta: f32) {
        self.sfx = snap_step(self.sfx + delta, MAX_SFX_VOLUME);
    }
}

fn snap_step(v: f32, max: f32) -> f32 {
    let v = (v * 10.0).round() / 10.0;
    v.clamp(0.0, max)
}

#[derive(Resource)]
pub struct AudioAssets {
    pub jump: Handle<AudioSource>,
    pub shoot: Handle<AudioSource>,
    pub hero_hit: Handle<AudioSource>,
    pub enemy_hit: Handle<AudioSource>,
    pub destroy_enemy: Handle<AudioSource>,
    pub button_click: Handle<AudioSource>,
    pub point: Handle<AudioSource>,
    pub one_up: Handle<AudioSource>,
    pub cookie: Handle<AudioSource>,
    pub die: Handle<AudioSource>,
    pub menu_music: Handle<AudioSource>,
    pub game_music: Handle<AudioSource>,
    pub final_music: Handle<AudioSource>,
    pub credits_music: Handle<AudioSource>,
}

/// One-shot sound effect request. Emit from any gameplay system — the
/// consumer (`play_sfx_events`) resolves the handle and spawns the player.
#[derive(Event, Copy, Clone, Debug)]
pub enum SfxEvent {
    Jump,
    Shoot,
    /// Enemy shot: same clip as the hero's, played quieter so a 16-ray fan
    /// doesn't drown everything else.
    EnemyShoot,
    HeroHit,
    EnemyHit,
    DestroyEnemy,
    ButtonClick,
    Point,
    OneUp,
    Cookie,
    Die,
}

/// Marker on the single looping-music entity so we can despawn it before
/// swapping tracks.
#[derive(Component)]
pub struct MusicTrack;

/// Which loop the game wants playing. `Silent` is only the startup value —
/// nothing ever asks for silence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MusicCue {
    #[default]
    Silent,
    Menu,
    Level,
    FinalLevel,
    Credits,
}

/// The cue the game asked for, and the one actually playing. They differ
/// between a state transition and the frame `apply_music_cue` catches up —
/// or for as long as `AudioAssets` is still missing.
#[derive(Resource, Default, Debug)]
pub struct MusicState {
    wanted: MusicCue,
    playing: MusicCue,
}

pub struct GameAudioPlugin;

impl Plugin for GameAudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<SfxEvent>()
            .init_resource::<AudioSettings>()
            .init_resource::<MusicState>()
            .add_systems(Startup, load_audio_assets)
            .add_systems(
                Update,
                (play_sfx_events, apply_music_cue, apply_music_volume),
            )
            .add_systems(OnEnter(GameState::MainMenu), want_music(MusicCue::Menu))
            .add_systems(OnEnter(GameState::GameOver), want_music(MusicCue::Menu))
            .add_systems(OnEnter(GameState::Victory), want_music(MusicCue::Credits))
            .add_systems(OnEnter(LevelState::LevelLoaded), want_level_music);
    }
}

fn load_audio_assets(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(AudioAssets {
        jump: asset_server.load("sounds/jump.ogg"),
        shoot: asset_server.load("sounds/ball_throw.ogg"),
        hero_hit: asset_server.load("sounds/dog_hit.ogg"),
        enemy_hit: asset_server.load("sounds/cat_meow.ogg"),
        destroy_enemy: asset_server.load("sounds/cat_purr.ogg"),
        button_click: asset_server.load("sounds/button_click.ogg"),
        point: asset_server.load("sounds/point.ogg"),
        one_up: asset_server.load("sounds/one_up.ogg"),
        cookie: asset_server.load("sounds/cookie.ogg"),
        die: asset_server.load("sounds/die.ogg"),
        menu_music: asset_server.load("sounds/menu.ogg"),
        game_music: asset_server.load("sounds/game.ogg"),
        final_music: asset_server.load("sounds/final_level.ogg"),
        credits_music: asset_server.load("sounds/credits.ogg"),
    });
}

fn play_sfx_events(
    mut commands: Commands,
    mut events: EventReader<SfxEvent>,
    assets: Option<Res<AudioAssets>>,
    settings: Res<AudioSettings>,
) {
    let Some(assets) = assets else {
        return;
    };
    for ev in events.read() {
        // Pygame plays `jump` at 0.7x; everything else at the sfx volume.
        let (handle, gain) = match ev {
            SfxEvent::Jump => (&assets.jump, 0.7),
            SfxEvent::Shoot => (&assets.shoot, 1.0),
            SfxEvent::EnemyShoot => (&assets.shoot, 0.35),
            SfxEvent::HeroHit => (&assets.hero_hit, 1.0),
            SfxEvent::EnemyHit => (&assets.enemy_hit, 1.0),
            SfxEvent::DestroyEnemy => (&assets.destroy_enemy, 1.0),
            SfxEvent::ButtonClick => (&assets.button_click, 1.0),
            SfxEvent::Point => (&assets.point, 1.0),
            SfxEvent::OneUp => (&assets.one_up, 1.0),
            SfxEvent::Cookie => (&assets.cookie, 1.0),
            SfxEvent::Die => (&assets.die, 1.0),
        };
        commands.spawn((
            AudioPlayer::new(handle.clone()),
            PlaybackSettings::DESPAWN.with_volume(Volume::Linear(settings.sfx * gain)),
        ));
    }
}

fn swap_music(
    commands: &mut Commands,
    tracks: &Query<Entity, With<MusicTrack>>,
    source: Handle<AudioSource>,
    volume: f32,
) {
    for entity in tracks.iter() {
        commands.entity(entity).despawn();
    }
    commands.spawn((
        AudioPlayer::new(source),
        PlaybackSettings::LOOP.with_volume(Volume::Linear(volume)),
        MusicTrack,
    ));
}

/// Pushes `AudioSettings.music` into the live music sink whenever the
/// settings change (Options screen buttons).
fn apply_music_volume(
    settings: Res<AudioSettings>,
    mut sinks: Query<&mut AudioSink, With<MusicTrack>>,
) {
    if !settings.is_changed() {
        return;
    }
    for mut sink in &mut sinks {
        sink.set_volume(Volume::Linear(settings.music));
    }
}

/// Records the cue a state transition wants. Nothing here touches assets,
/// so it is safe to run before `Startup` has loaded them.
fn want_music(cue: MusicCue) -> impl Fn(ResMut<MusicState>) {
    move |mut music: ResMut<MusicState>| music.wanted = cue
}

/// The boss level (Level 4) gets `final_level.ogg`, everything else
/// `game.ogg`.
fn want_level_music(mut music: ResMut<MusicState>, current: Res<CurrentLevel>) {
    music.wanted = match current.0 {
        Level::Level4 => MusicCue::FinalLevel,
        _ => MusicCue::Level,
    };
}

/// Makes the playing track match the wanted cue, as soon as the assets are
/// there. A cue that is already playing is left alone, so the loop is never
/// restarted from the top by a transition that does not change the track.
fn apply_music_cue(
    mut commands: Commands,
    tracks: Query<Entity, With<MusicTrack>>,
    assets: Option<Res<AudioAssets>>,
    settings: Res<AudioSettings>,
    mut music: ResMut<MusicState>,
) {
    if music.wanted == music.playing {
        return;
    }
    let Some(assets) = assets else {
        return; // Startup has not run yet; retry next frame.
    };
    let source = match music.wanted {
        MusicCue::Silent => None,
        MusicCue::Menu => Some(assets.menu_music.clone()),
        MusicCue::Level => Some(assets.game_music.clone()),
        MusicCue::FinalLevel => Some(assets.final_music.clone()),
        MusicCue::Credits => Some(assets.credits_music.clone()),
    };
    let Some(source) = source else {
        return;
    };
    swap_music(&mut commands, &tracks, source, settings.music);
    music.playing = music.wanted;
}
