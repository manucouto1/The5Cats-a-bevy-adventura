//! Audio: one-shot SFX via events, and per-state looping music.
//!
//! Gameplay systems emit `SfxEvent`s — they do not know about asset handles
//! or playback settings. `play_sfx_events` is the single consumer that maps
//! a kind to an `AudioSource` and spawns a `PlaybackSettings::DESPAWN` entity
//! so the audio node cleans itself up once the clip ends.
//!
//! Music is a single entity tagged `MusicTrack`: state-transition systems
//! despawn it and spawn a new one with the appropriate loop.

use bevy::prelude::*;

use crate::game_state::{CurrentLevel, GameState, Level, LevelState};

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
    pub gameover_music: Handle<AudioSource>,
}

/// One-shot sound effect request. Emit from any gameplay system — the
/// consumer (`play_sfx_events`) resolves the handle and spawns the player.
#[derive(Event, Copy, Clone, Debug)]
pub enum SfxEvent {
    Jump,
    Shoot,
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
/// swapping tracks on state transitions.
#[derive(Component)]
pub struct MusicTrack;

pub struct GameAudioPlugin;

impl Plugin for GameAudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<SfxEvent>()
            .add_systems(Startup, load_audio_assets)
            .add_systems(Update, play_sfx_events)
            .add_systems(OnEnter(GameState::MainMenu), play_menu_music)
            .add_systems(OnEnter(GameState::GameOver), play_gameover_music)
            .add_systems(OnEnter(LevelState::LevelLoaded), play_level_music);
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
        gameover_music: asset_server.load("sounds/credits.ogg"),
    });
}

fn play_sfx_events(
    mut commands: Commands,
    mut events: EventReader<SfxEvent>,
    assets: Option<Res<AudioAssets>>,
) {
    let Some(assets) = assets else {
        return;
    };
    for ev in events.read() {
        let handle = match ev {
            SfxEvent::Jump => &assets.jump,
            SfxEvent::Shoot => &assets.shoot,
            SfxEvent::HeroHit => &assets.hero_hit,
            SfxEvent::EnemyHit => &assets.enemy_hit,
            SfxEvent::DestroyEnemy => &assets.destroy_enemy,
            SfxEvent::ButtonClick => &assets.button_click,
            SfxEvent::Point => &assets.point,
            SfxEvent::OneUp => &assets.one_up,
            SfxEvent::Cookie => &assets.cookie,
            SfxEvent::Die => &assets.die,
        };
        commands.spawn((
            AudioPlayer::new(handle.clone()),
            PlaybackSettings::DESPAWN,
        ));
    }
}

fn swap_music(
    commands: &mut Commands,
    tracks: &Query<Entity, With<MusicTrack>>,
    source: Handle<AudioSource>,
) {
    for entity in tracks.iter() {
        commands.entity(entity).despawn();
    }
    commands.spawn((
        AudioPlayer::new(source),
        PlaybackSettings::LOOP,
        MusicTrack,
    ));
}

fn play_menu_music(
    mut commands: Commands,
    tracks: Query<Entity, With<MusicTrack>>,
    assets: Option<Res<AudioAssets>>,
) {
    let Some(assets) = assets else {
        return;
    };
    swap_music(&mut commands, &tracks, assets.menu_music.clone());
}

fn play_gameover_music(
    mut commands: Commands,
    tracks: Query<Entity, With<MusicTrack>>,
    assets: Option<Res<AudioAssets>>,
) {
    let Some(assets) = assets else {
        return;
    };
    swap_music(&mut commands, &tracks, assets.gameover_music.clone());
}

/// Picks the per-level track when the level finishes loading: the boss
/// level (Level 4) gets `final_level.ogg`, everything else gets `game.ogg`.
fn play_level_music(
    mut commands: Commands,
    tracks: Query<Entity, With<MusicTrack>>,
    assets: Option<Res<AudioAssets>>,
    current: Res<CurrentLevel>,
) {
    let Some(assets) = assets else {
        return;
    };
    let handle = match current.0 {
        Level::Level4 => assets.final_music.clone(),
        _ => assets.game_music.clone(),
    };
    swap_music(&mut commands, &tracks, handle);
}
