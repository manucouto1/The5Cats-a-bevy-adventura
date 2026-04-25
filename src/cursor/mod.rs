mod assets;
mod components;
mod systems;

use bevy::{prelude::*, window::PrimaryWindow};

use crate::{
    cursor::{
        assets::{CursorAssets, load_assets},
        components::Crosshair,
        systems::{
            boss_damage_system, boss_follow_hero_system, handle_projectile_despawn,
            spawn_projectile_on_click, update_aim_assist, wool_ball_damage_system,
        },
    },
    game_state::{GameState, LevelState},
    map::assets::GameAssets,
    player::assets::HeroData,
};

pub struct CursorPlugin;

impl Plugin for CursorPlugin {
    fn build(&self, app: &mut App) {
        // Cursor lifecycle is tied to LevelState: assets load on Loading
        // (so HeroData/GameAssets are ready), the crosshair spawns on
        // LevelLoaded, despawns on exit. Otherwise `OnEnter(GameState::Game)`
        // could race `OnEnter(LevelState::Loading)` and miss the resources.
        app.add_systems(OnEnter(LevelState::Loading), load_assets)
            .add_systems(
                OnEnter(LevelState::LevelLoaded),
                (spawn_aim_assist, hide_system_cursor),
            )
            .add_systems(OnEnter(GameState::MainMenu), show_system_cursor)
            .add_systems(OnEnter(GameState::GameOver), show_system_cursor)
            .add_systems(
                Update,
                (
                    update_aim_assist,
                    spawn_projectile_on_click.after(update_aim_assist),
                    // wool_ball_damage_system (filters !FinalBoss) and
                    // boss_damage_system (FinalBoss only) consume the same
                    // CollisionEvent queue but on disjoint entity sets, so
                    // they can run in parallel.
                    wool_ball_damage_system.after(spawn_projectile_on_click),
                    boss_damage_system.after(spawn_projectile_on_click),
                    boss_follow_hero_system,
                    handle_projectile_despawn
                        .after(wool_ball_damage_system)
                        .after(boss_damage_system),
                )
                    .run_if(in_state(GameState::Game)),
            )
            .add_systems(OnExit(LevelState::LevelLoaded), despawn_cursor);
    }
}

pub fn despawn_cursor(mut commands: Commands, query: Query<Entity, With<Crosshair>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}

pub fn hide_system_cursor(mut window_query: Query<&mut Window, With<PrimaryWindow>>) {
    if let Ok(mut window) = window_query.single_mut() {
        window.cursor_options.visible = false;
    }
}

pub fn show_system_cursor(mut window_query: Query<&mut Window, With<PrimaryWindow>>) {
    if let Ok(mut window) = window_query.single_mut() {
        window.cursor_options.visible = true;
    }
}

pub fn spawn_aim_assist(
    mut commands: Commands,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    game_assets: Res<GameAssets>,
    hero_data: Res<HeroData>,
    cursor_assets: Res<CursorAssets>,
) {
    let tile_size_from_json = game_assets.tile_size_px;
    let map_width_from_json = game_assets.map_width_tiles;
    let map_height_from_json = game_assets.map_height_tiles;
    let x = hero_data.x as f32;
    let y = hero_data.y as f32;

    let world_x =
        x * tile_size_from_json - (map_width_from_json as f32 * tile_size_from_json / 2.0);
    let world_y =
        -y * tile_size_from_json + (map_height_from_json as f32 * tile_size_from_json / 2.0);

    let mut transform = Transform::from_scale(Vec3::splat(0.9));

    transform.translation.x = world_x + tile_size_from_json / 2.0;
    transform.translation.y = world_y - tile_size_from_json / 2.0;
    transform.translation.z = 100.0;

    let layout = TextureAtlasLayout::from_grid(UVec2::splat(16), 5, 1, None, None);
    let texture_atlas_layout = texture_atlas_layouts.add(layout);

    commands.spawn((
        Sprite {
            image: cursor_assets.cursor_image.clone(), // Reemplaza con tu imagen
            texture_atlas: Some(TextureAtlas {
                layout: texture_atlas_layout.clone(),
                index: 4,
            }),
            ..default()
        },
        transform,
        Crosshair,
    ));
}
