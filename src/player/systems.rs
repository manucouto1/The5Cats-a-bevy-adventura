use crate::{
    game_state::GameState,
    map::assets::GameAssets,
    physics::Velocity,
    player::{
        ANIMATION_FPS,
        components::{
            AnimationIndices, CharacterIdleSprite, CharacterLeftSprite, CharacterRightSprite,
            DoubleJump, HORIZONTAL_FORCE, Health, Invincibility, JUMP_FORCE, PlayerCharacter,
            PlayerHearts,
        },
    },
};
use bevy::prelude::*;
use bevy_rapier2d::prelude::KinematicCharacterControllerOutput;

#[derive(Resource)]
pub struct GameOverTimer(Timer);

// Sistema principal de físicas del personaje
pub fn player_input_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut query: Query<
        (
            &mut Velocity,
            &KinematicCharacterControllerOutput,
            &mut DoubleJump,
        ),
        With<PlayerCharacter>,
    >,
) {
    for (mut velocity, output, mut double_jump) in &mut query {
        // Movimiento lateral (sin acumulación, directo)
        let mut horizontal = 0.0;
        if keyboard.pressed(KeyCode::ArrowLeft) || keyboard.pressed(KeyCode::KeyA) {
            horizontal -= HORIZONTAL_FORCE;
        }
        if keyboard.pressed(KeyCode::ArrowRight) || keyboard.pressed(KeyCode::KeyD) {
            horizontal += HORIZONTAL_FORCE;
        }

        velocity.velocity.x = horizontal;

        // Salto (solo una vez al presionar)
        if keyboard.just_pressed(KeyCode::ArrowUp) || keyboard.just_pressed(KeyCode::KeyW) {
            if output.grounded || double_jump.jumps_remaining > 0 {
                velocity.velocity.y = JUMP_FORCE;
                double_jump.jumps_remaining -= 1;
            }
        }
    }
}

pub fn reset_jumps(
    mut query: Query<(&KinematicCharacterControllerOutput, &mut DoubleJump), With<PlayerCharacter>>,
) {
    for (output, mut double_jump) in &mut query {
        if output.grounded {
            double_jump.jumps_remaining = double_jump.max_jumps;
        }
    }
}

pub fn update_player_life(
    mut commands: Commands,
    player_query: Query<&Health, (With<PlayerCharacter>, Changed<Health>)>,
    heart_query: Query<(Entity, &PlayerHearts)>,
) {
    // Solo se ejecuta si la vida del jugador ha cambiado
    let Ok(player_health) = player_query.single() else {
        return;
    };

    let active_heart_index = player_health.current as usize;

    for (entity, heart_data) in heart_query.iter() {
        if heart_data.idx == active_heart_index {
            commands
                .entity(entity)
                .insert(AnimationIndices::new(0, 21, ANIMATION_FPS));
        } else {
            commands.entity(entity).remove::<AnimationIndices>();
        }
    }
}

pub fn check_player_death(
    player_query: Query<&Health, (With<PlayerCharacter>, Changed<Health>)>,
    mut commands: Commands,
    game_over_timer: Option<Res<GameOverTimer>>,
) {
    let Ok(player_health) = player_query.single() else {
        return;
    };
    if player_health.current == 0 {
        // Solo inserta el temporizador si no existe ya
        if game_over_timer.is_none() {
            println!("La vida del jugador llegó a cero. Esperando 2 segundos...");
            commands.insert_resource(GameOverTimer(Timer::from_seconds(2.0, TimerMode::Once)));
        }
    }
}

pub fn handle_gameover_timer(
    mut commands: Commands,
    mut next_state: ResMut<NextState<GameState>>,
    game_over_timer: Option<ResMut<GameOverTimer>>,
    time: Res<Time>,
) {
    if let Some(mut timer) = game_over_timer {
        timer.0.tick(time.delta());

        if timer.0.finished() {
            println!("Temporizador terminado. Cambiando a GameState::GameOver.");
            next_state.set(GameState::GameOver);
            commands.remove_resource::<GameOverTimer>();
            // El temporizador se eliminará automáticamente cuando cambie el estado
        }
    }
}

pub fn animate_hearts(time: Res<Time>, mut query: Query<(&mut AnimationIndices, &mut ImageNode)>) {
    for (mut config, mut sprite) in &mut query {
        config.frame_timer.tick(time.delta());

        if config.frame_timer.just_finished() {
            if let Some(atlas) = &mut sprite.texture_atlas {
                if atlas.index != config.last - 1 {
                    atlas.index += 1;
                    config.frame_timer = AnimationIndices::timer_from_fps(config.fps);
                }
            }
        }
    }
}

// Sistema para manejar las animaciones
pub fn execute_animations(time: Res<Time>, mut query: Query<(&mut AnimationIndices, &mut Sprite)>) {
    for (mut config, mut sprite) in &mut query {
        config.frame_timer.tick(time.delta());

        if config.frame_timer.just_finished() {
            if let Some(atlas) = &mut sprite.texture_atlas {
                if atlas.index == config.last {
                    atlas.index = config.first;
                } else {
                    atlas.index += 1;
                    config.frame_timer = AnimationIndices::timer_from_fps(config.fps);
                }
            }
        }
    }
}

pub fn character_input_handling(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut visibility_queries: ParamSet<(
        Query<&mut Visibility, With<CharacterLeftSprite>>,
        Query<&mut Visibility, With<CharacterRightSprite>>,
        Query<&mut Visibility, With<CharacterIdleSprite>>,
    )>,
    player_parent_query: Query<&Children, With<PlayerCharacter>>,
) {
    let left = keyboard.pressed(KeyCode::ArrowLeft) || keyboard.pressed(KeyCode::KeyA);
    let right = keyboard.pressed(KeyCode::ArrowRight) || keyboard.pressed(KeyCode::KeyD);

    // Obtenemos los hijos del PlayerCharacter.
    let Ok(player_children) = player_parent_query.single() else {
        return;
    };
    // La lógica de visibilidad ahora se aplica a los hijos del jugador.
    for child in player_children.iter() {
        match (left, right) {
            (true, false) => {
                if let Ok(mut v) = visibility_queries.p0().get_mut(child) {
                    *v = Visibility::Visible;
                } else if let Ok(mut v) = visibility_queries.p1().get_mut(child) {
                    *v = Visibility::Hidden;
                } else if let Ok(mut v) = visibility_queries.p2().get_mut(child) {
                    *v = Visibility::Hidden;
                }
            }
            (false, true) => {
                if let Ok(mut v) = visibility_queries.p0().get_mut(child) {
                    *v = Visibility::Hidden;
                } else if let Ok(mut v) = visibility_queries.p1().get_mut(child) {
                    *v = Visibility::Visible;
                } else if let Ok(mut v) = visibility_queries.p2().get_mut(child) {
                    *v = Visibility::Hidden;
                }
            }
            _ => {
                if let Ok(mut v) = visibility_queries.p0().get_mut(child) {
                    *v = Visibility::Hidden;
                } else if let Ok(mut v) = visibility_queries.p1().get_mut(child) {
                    *v = Visibility::Hidden;
                } else if let Ok(mut v) = visibility_queries.p2().get_mut(child) {
                    *v = Visibility::Visible;
                }
            }
        }
    }
}

// Sistema para aplicar límites de mapa al player
pub fn player_bounds_system(
    game_assets: Res<GameAssets>,
    mut player_query: Query<&mut Transform, With<PlayerCharacter>>,
) {
    for mut transform in player_query.iter_mut() {
        let map_width_px = game_assets.map_width_tiles as f32 * game_assets.tile_size_px;
        let map_height_px = game_assets.map_height_tiles as f32 * game_assets.tile_size_px;

        let map_left = -(map_width_px / 2.0);
        let map_right = map_width_px / 2.0;
        let _map_bottom = -(map_height_px / 2.0);
        let _map_top = map_height_px / 2.0;

        let player_margin = 16.0; // Margen en píxeles

        if transform.translation.x < map_left + player_margin {
            transform.translation.x = map_left + player_margin;
        } else if transform.translation.x > map_right - player_margin {
            transform.translation.x = map_right - player_margin;
        }
    }
}

pub fn invincibility_system(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut Invincibility), With<PlayerCharacter>>,
) {
    for (entity, mut invincibility) in query.iter_mut() {
        println!("Hell yea i'm invincible!");
        invincibility.timer.tick(time.delta());
        if invincibility.timer.finished() {
            commands.entity(entity).remove::<Invincibility>();
        }
    }
}
