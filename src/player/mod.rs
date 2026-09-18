// src/player/mod.rs

pub mod assets;
pub mod bundle; // Declara el submódulo bundle.rs
pub mod components; // Declara el submódulo components.rs
pub mod systems; // Declara el submódulo systems.rs // Declara el submódulo assets.rs

use crate::game_state::{GameState, LevelState};
use crate::map::assets::GameAssets;
use crate::physics::{AffectedByGravity, Mass, Velocity};
use crate::player::assets::{HeroData, load_player_assets};

use crate::player::{
    assets::PlayerAssets, // Importa PlayerAssets
    bundle::PlayerBundle, // Importa el PlayerBundle
    components::*,        // Importa todos los componentes del player
    systems::*,           // Importa todos los sistemas del player
};
use bevy::prelude::*;
use bevy_rapier2d::prelude::{
    ActiveCollisionTypes, ActiveEvents, CharacterAutostep, CharacterLength, Collider,
    CollisionGroups, Group, KinematicCharacterController, RigidBody, Velocity as RapierVelocity,
};

// Esta función añadirá todos los sistemas del jugador a la aplicación
pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        // Player lifecycle is tied to LevelState: assets load on Loading,
        // character spawns on LevelLoaded, and all of it despawns on exit —
        // so changing level mid-game cleanly tears down and rebuilds.
        app.init_resource::<PlayerHitGuard>()
            .add_systems(OnEnter(LevelState::Loading), load_player_assets)
            .add_systems(
                OnEnter(LevelState::LevelLoaded),
                (
                    // Spawn the level tiles first so their colliders are in
                    // the ECS before the player. Otherwise on level switch
                    // the player can land before Rapier registers the new
                    // tile colliders and falls straight through the map.
                    spawn_player_character.after(crate::map::spawn_level_tiles),
                ),
            )
            .add_systems(
                Update,
                (
                    character_input_handling,
                    // player_input_system is registered in main.rs as part
                    // of the strict input→gravity→movement chain.
                    execute_animations,
                    player_bounds_system,
                    reset_jumps,
                    jump_impact_dampen_system,
                    invincibility_system,
                    invincibility_blink_system,
                    knockback_system,
                    maniac_costume_system,
                    check_player_death,
                    dead_tumble_system,
                    handle_gameover_timer,
                )
                    .run_if(in_state(GameState::Game)),
            )
            .add_systems(OnExit(LevelState::LevelLoaded), despawn_player);
    }
}

fn despawn_player(mut commands: Commands, player_query: Query<Entity, With<PlayerCharacter>>) {
    for entity in player_query.iter() {
        commands.entity(entity).despawn();
    }
}

pub const PLAYER_GROUP: Group = Group::GROUP_1;
/// Membership of every enemy body collider. Character controllers (player
/// and enemies) exclude it so cats never physically block each other —
/// contact damage still fires through collision events.
pub const ENEMY_GROUP: Group = Group::GROUP_3;

pub fn spawn_player_character(
    mut commands: Commands,
    player_assets: Res<PlayerAssets>, // Ahora obtenemos los assets precargados
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    hero_data: Res<HeroData>,
    game_assets: Res<GameAssets>,
) {
    let tile_size_from_json = game_assets.tile_size_px;
    let map_width_from_json = game_assets.map_width_tiles;
    let map_height_from_json = game_assets.map_height_tiles;

    let layout = TextureAtlasLayout::from_grid(UVec2::splat(64), 8, 1, None, None);
    let texture_atlas_layout = texture_atlas_layouts.add(layout);

    // Spawn directly at the JSON's hero coordinates — trust the level data.
    let x = hero_data.x as f32;
    let y = hero_data.y as f32;

    let world_x =
        x * tile_size_from_json - (map_width_from_json as f32 * tile_size_from_json / 2.0);
    let world_y =
        -y * tile_size_from_json + (map_height_from_json as f32 * tile_size_from_json / 2.0);

    let mut transform = Transform::from_scale(Vec3::splat(0.9));

    transform.translation.x = world_x + tile_size_from_json / 2.0;
    // JSON coordinates are the sprite's bottom-left tile; the collider is a
    // 16px ball centered on the entity, so lift it so the feet rest on the
    // tile below rather than starting embedded in it.
    transform.translation.y = world_y - tile_size_from_json / 2.0 + 4.0;
    // Tiles render at z = layer.name * 0.1 (up to ~0.5 in level 2). Place
    // the player well above that so it never gets occluded by a tile layer.
    transform.translation.z = 10.0;

    let character_controller = KinematicCharacterController {
        // Allow ALL collisions on spawn; `one_way_platform_collision_system`
        // immediately swaps in the proper PLAYER_GROUP / !ONE_WAY_PLATFORM
        // filter once gravity gives velocity.y < 0. Hard-coding the
        // exclusion here meant the player fell through any one-way
        // platform on the spawn frame (level 2 starts on a row of those).
        normal_nudge_factor: 1.0e-2,
        // Small lips (tile seams, a crate edge) are stepped over; a full
        // tile still needs a jump, like the original.
        autostep: Some(CharacterAutostep {
            max_height: CharacterLength::Absolute(8.0),
            min_width: CharacterLength::Absolute(4.0),
            include_dynamic_bodies: false,
        }),
        ..default()
    };

    commands
        .spawn(PlayerBundle::new(transform))
        .with_children(|parent| {
            parent.spawn((
                Sprite {
                    image: player_assets.normal.left.clone(),
                    texture_atlas: Some(TextureAtlas {
                        layout: texture_atlas_layout.clone(),
                        index: 0,
                    }),
                    ..default()
                },
                Transform::from_scale(Vec3::splat(0.6)),
                CharacterLeftSprite,
                Visibility::Hidden,
                AnimationIndices::new(0, 7, ANIMATION_FPS),
            ));
            parent.spawn((
                Sprite {
                    image: player_assets.normal.right.clone(),
                    texture_atlas: Some(TextureAtlas {
                        layout: texture_atlas_layout.clone(),
                        index: 0,
                    }),
                    ..default()
                },
                Transform::from_scale(Vec3::splat(0.6)),
                CharacterRightSprite,
                Visibility::Hidden,
                AnimationIndices::new(0, 7, ANIMATION_FPS),
            ));
            parent.spawn((
                Sprite {
                    image: player_assets.normal.standing.clone(),
                    texture_atlas: Some(TextureAtlas {
                        layout: texture_atlas_layout.clone(),
                        index: 0,
                    }),
                    ..default()
                },
                Transform::from_scale(Vec3::splat(0.6)),
                CharacterIdleSprite,
                Visibility::Visible,
                AnimationIndices::new(0, 7, ANIMATION_FPS),
            ));
        })
        .insert(RigidBody::KinematicPositionBased)
        .insert(character_controller)
        // A rounded box instead of a ball: a ball wedged itself 20 px up
        // any one-tile step it walked into and got stuck on the corner.
        .insert(Collider::round_cuboid(10.0, 12.0, 3.0))
        .insert(CollisionGroups::new(PLAYER_GROUP, Group::ALL))
        .insert(PlayerCharacter)
        .insert(AffectedByGravity)
        .insert(RapierVelocity::zero())
        .insert(Mass::default())
        .insert(Health::default())
        .insert(DoubleJump::default())
        .insert(Velocity::default())
        .insert(ActiveEvents::COLLISION_EVENTS)
        .insert(Invincibility::new(SPAWN_GRACE_SECS))
        // Rapier's default ActiveCollisionTypes only reports pairs involving
        // a Dynamic body. The player is Kinematic, and so are every enemy,
        // projectile, and collectible — plus EndLevel tiles are Fixed. Without
        // this, none of those collisions fire `CollisionEvent::Started`, so
        // damage, pickups, and the level-complete trigger all go silent.
        .insert(ActiveCollisionTypes::all());
}

// Puedes definir constantes aquí o en un submódulo de constantes si tienes muchas
const ANIMATION_FPS: u8 = 10;
