use crate::{
    collectibles::components::ManiacBuff,
    game_state::GameState,
    map::assets::GameAssets,
    physics::{FallingMode, Velocity},
    player::{
        assets::PlayerAssets,
        components::{
            AnimationIndices, CharacterIdleSprite, CharacterLeftSprite, CharacterRightSprite, Dead,
            DoubleJump, HORIZONTAL_FORCE, Health, Invincibility, JUMP_FORCE, Knockback,
            PlayerCharacter,
        },
    },
};
use bevy::prelude::*;
use bevy_rapier2d::prelude::{
    CollisionGroups, Group, KinematicCharacterController, KinematicCharacterControllerOutput,
};

#[derive(Resource)]
pub struct GameOverTimer(Timer);

pub fn player_input_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut query: Query<
        (
            &mut Velocity,
            // Optional: Rapier inserts this component on the first physics
            // step after spawn. Filtering on `&` instead of `Option<&>` made
            // the system silently skip the player on the spawn frame, leaving
            // velocity at its last value — and any time Rapier momentarily
            // dropped the component the input would freeze.
            Option<&KinematicCharacterControllerOutput>,
            &mut DoubleJump,
            Option<&mut Knockback>,
            Has<FallingMode>,
        ),
        (With<PlayerCharacter>, Without<Dead>),
    >,
    mut sfx: EventWriter<crate::audio::SfxEvent>,
) {
    for (mut velocity, output, mut double_jump, knockback, falling) in &mut query {
        // Knockback overrides input for the whole stun so the shove from a
        // hit reads clearly (pygame: forced jump + push away for 0.5 s).
        if let Some(mut kb) = knockback {
            if kb.push != Vec2::ZERO {
                velocity.velocity.x = kb.push.x;
                if !kb.applied {
                    velocity.velocity.y = kb.push.y;
                    kb.applied = true;
                }
            }
            continue;
        }

        // Movimiento lateral (sin acumulación, directo). Free-fall gives a
        // little extra lateral speed so the shaft gauntlet is dodgeable.
        let lateral = if falling {
            HORIZONTAL_FORCE * 1.3
        } else {
            HORIZONTAL_FORCE
        };
        let mut horizontal = 0.0;
        if keyboard.pressed(KeyCode::ArrowLeft) || keyboard.pressed(KeyCode::KeyA) {
            horizontal -= lateral;
        }
        if keyboard.pressed(KeyCode::ArrowRight) || keyboard.pressed(KeyCode::KeyD) {
            horizontal += lateral;
        }

        velocity.velocity.x = horizontal;

        // No jumping while free-falling down the shaft (W/S steer instead,
        // see `falling_mode_system`).
        if falling {
            continue;
        }

        // Salto (solo una vez al presionar)
        if keyboard.just_pressed(KeyCode::ArrowUp)
            || keyboard.just_pressed(KeyCode::KeyW)
            || keyboard.just_pressed(KeyCode::Space)
        {
            let grounded = output.is_some_and(|o| o.grounded);
            if grounded || double_jump.jumps_remaining > 0 {
                velocity.velocity.y = JUMP_FORCE;
                double_jump.jumps_remaining -= 1;
                sfx.write(crate::audio::SfxEvent::Jump);
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

/// Cancel upward velocity when the player hits a ceiling and dampen
/// horizontal velocity on wall contact, so a jump that bonks the head
/// stops climbing instead of holding the character pinned to the ceiling.
/// Detects "blocked" axes by comparing desired vs effective translation
/// from the kinematic controller output each frame.
pub fn jump_impact_dampen_system(
    mut query: Query<(&KinematicCharacterControllerOutput, &mut Velocity), With<PlayerCharacter>>,
) {
    const EPS: f32 = 0.5;
    const WALL_DAMPEN: f32 = 0.0;
    for (output, mut velocity) in &mut query {
        let desired = output.desired_translation;
        let effective = output.effective_translation;
        // Ceiling hit: wanted to move up but didn't.
        if desired.y > EPS && effective.y < desired.y - EPS && velocity.velocity.y > 0.0 {
            velocity.velocity.y = 0.0;
        }
        // Wall hit: wanted to move horizontally but didn't.
        if desired.x.abs() > EPS && (effective.x - desired.x).abs() > EPS {
            velocity.velocity.x *= WALL_DAMPEN;
        }
    }
}

/// Starts the death sequence the moment HP hits zero: the player stops
/// responding to input, its controller stops colliding with the level so
/// it drops out of the world, and the game-over timer starts.
pub fn check_player_death(
    mut player_query: Query<
        (
            Entity,
            &Health,
            &mut Velocity,
            &mut KinematicCharacterController,
        ),
        (With<PlayerCharacter>, Without<Dead>),
    >,
    mut commands: Commands,
    game_over_timer: Option<Res<GameOverTimer>>,
    mut sfx: EventWriter<crate::audio::SfxEvent>,
) {
    let Ok((entity, player_health, mut velocity, mut controller)) = player_query.single_mut()
    else {
        return;
    };
    if player_health.current != 0 {
        return;
    }
    if game_over_timer.is_none() {
        commands.insert_resource(GameOverTimer(Timer::from_seconds(2.2, TimerMode::Once)));
        sfx.write(crate::audio::SfxEvent::Die);
    }
    // Classic platformer death: hop up, then fall through everything.
    velocity.velocity = Vec2::new(0.0, 360.0);
    controller.filter_groups = Some(CollisionGroups::new(Group::NONE, Group::NONE));
    commands
        .entity(entity)
        .insert(Dead)
        .remove::<Invincibility>()
        .remove::<Knockback>()
        .remove::<FallingMode>();
}

/// Spins the dead player's sprite while it falls.
pub fn dead_tumble_system(
    time: Res<Time>,
    mut players: Query<&Children, (With<PlayerCharacter>, With<Dead>)>,
    mut sprites: Query<(&mut Transform, &mut Sprite), Without<PlayerCharacter>>,
) {
    for children in &mut players {
        for child in children.iter() {
            if let Ok((mut tf, mut sprite)) = sprites.get_mut(child) {
                tf.rotate_z(-6.0 * time.delta_secs());
                sprite.color = Color::WHITE;
            }
        }
    }
}

/// Swaps the hero's sprite strips for the foil-hat costume while maniac
/// mode is active, and back when it expires.
pub fn maniac_costume_system(
    assets: Res<PlayerAssets>,
    gained: Query<&Children, (With<PlayerCharacter>, Added<ManiacBuff>)>,
    mut lost: RemovedComponents<ManiacBuff>,
    players: Query<&Children, With<PlayerCharacter>>,
    mut sprites: Query<(
        &mut Sprite,
        Has<CharacterLeftSprite>,
        Has<CharacterRightSprite>,
        Has<CharacterIdleSprite>,
    )>,
) {
    let mut apply = |children: &Children, hat: bool| {
        let sheets = if hat { &assets.hat } else { &assets.normal };
        for child in children.iter() {
            if let Ok((mut sprite, left, right, idle)) = sprites.get_mut(child) {
                sprite.image = if left {
                    sheets.left.clone()
                } else if right {
                    sheets.right.clone()
                } else if idle {
                    sheets.standing.clone()
                } else {
                    continue;
                };
            }
        }
    };
    for children in &gained {
        apply(children, true);
    }
    for entity in lost.read() {
        if let Ok(children) = players.get(entity) {
            apply(children, false);
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
    mut player_query: Query<(&mut Transform, &mut Health, Has<Dead>), With<PlayerCharacter>>,
) {
    for (mut transform, mut health, dead) in player_query.iter_mut() {
        let map_width_px = game_assets.map_width_tiles as f32 * game_assets.tile_size_px;
        let map_height_px = game_assets.map_height_tiles as f32 * game_assets.tile_size_px;

        let map_left = -(map_width_px / 2.0);
        let map_right = map_width_px / 2.0;
        let map_bottom = -(map_height_px / 2.0);

        let player_margin = 16.0; // Margen en píxeles

        if !dead {
            if transform.translation.x < map_left + player_margin {
                transform.translation.x = map_left + player_margin;
            } else if transform.translation.x > map_right - player_margin {
                transform.translation.x = map_right - player_margin;
            }
            // Falling out of the world is fatal (pygame clamped the hero to
            // the screen instead, but levels have no open pits so this only
            // matters for glitches through the floor).
            if transform.translation.y < map_bottom - 160.0 && health.current > 0 {
                health.current = 0;
            }
        }
    }
}

pub fn invincibility_system(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut Invincibility), With<PlayerCharacter>>,
) {
    for (entity, mut invincibility) in query.iter_mut() {
        invincibility.timer.tick(time.delta());
        if invincibility.timer.finished() {
            commands.entity(entity).remove::<Invincibility>();
        }
    }
}

/// Flickers the hero's sprites while invincible so the player can read the
/// grace window; restores full opacity when it ends.
pub fn invincibility_blink_system(
    time: Res<Time>,
    players: Query<(&Children, Option<&Invincibility>), (With<PlayerCharacter>, Without<Dead>)>,
    mut removed: RemovedComponents<Invincibility>,
    all_players: Query<&Children, With<PlayerCharacter>>,
    mut sprites: Query<&mut Sprite>,
) {
    for (children, invincibility) in &players {
        let Some(inv) = invincibility else { continue };
        let phase = ((inv.timer.elapsed_secs() * 14.0) as u32) % 2 == 0;
        let alpha = if phase { 1.0 } else { 0.25 };
        let _ = time.delta();
        for child in children.iter() {
            if let Ok(mut sprite) = sprites.get_mut(child) {
                sprite.color = Color::WHITE.with_alpha(alpha);
            }
        }
    }
    for entity in removed.read() {
        if let Ok(children) = all_players.get(entity) {
            for child in children.iter() {
                if let Ok(mut sprite) = sprites.get_mut(child) {
                    sprite.color = Color::WHITE;
                }
            }
        }
    }
}

pub fn knockback_system(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut Knockback), With<PlayerCharacter>>,
) {
    for (entity, mut knockback) in query.iter_mut() {
        knockback.timer.tick(time.delta());
        if knockback.timer.finished() {
            commands.entity(entity).remove::<Knockback>();
        }
    }
}
