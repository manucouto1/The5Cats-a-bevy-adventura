use bevy::{prelude::*, window::PrimaryWindow};
use bevy_rapier2d::prelude::{
    ActiveEvents, Collider, CollisionEvent, RigidBody, Sleeping, Velocity,
};

use crate::{
    cursor::{
        assets::CursorAssets,
        components::{Crosshair, Projectile, WoolBall},
    },
    physics::{AffectedByGravity, Mass},
    player::components::PlayerCharacter,
};

pub fn update_aim_assist(
    // Recursos para obtener la posición del ratón
    // mut evr_cursor: EventReader<CursorMoved>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    mut crosshair_query: Query<&mut Transform, With<Crosshair>>,
) {
    if let Some(position) = window.cursor_position() {
        if let Ok((camera, camera_transform)) = camera_query.single() {
            if let Ok(world_position) = camera.viewport_to_world_2d(camera_transform, position) {
                for mut crosshair_transform in crosshair_query.iter_mut() {
                    crosshair_transform.translation.x = world_position.x;
                    crosshair_transform.translation.y = world_position.y;
                }
            }
        }
    }
}

fn solve_ballistic_velocity(
    start: Vec2,
    target: Vec2,
    speed: f32,
    gravity: f32,
    prefer_high_arc: bool,
) -> Option<Vec2> {
    let dx = target.x - start.x;
    let dy = target.y - start.y;

    if dx.abs() < 1e-6 {
        let vy = if dy > 0.0 { speed } else { -speed };
        return Some(Vec2::new(0.0, vy));
    }

    let v2 = speed * speed;
    let dx2 = dx * dx;
    let g = gravity;

    let inside = v2 * v2 - g * (g * dx2 + 2.0 * dy * v2);
    if inside < 0.0 {
        return None;
    }

    let sqrt_val = inside.sqrt();
    let numerator = if prefer_high_arc {
        v2 + sqrt_val
    } else {
        v2 - sqrt_val
    };
    let tan_theta = numerator / (g * dx.abs());
    let theta = tan_theta.atan();

    let direction_x_sign = dx.signum();

    let vx = speed * theta.cos() * direction_x_sign;
    let vy = speed * theta.sin();

    Some(Vec2::new(vx, vy))
}

pub fn spawn_projectile_on_click(
    mut commands: Commands,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    cursor_assets: Res<CursorAssets>,
    player_query: Query<&Transform, With<PlayerCharacter>>,
    crosshair_query: Query<&Transform, With<Crosshair>>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    if mouse_button_input.just_pressed(MouseButton::Left) {
        let layout = TextureAtlasLayout::from_grid(UVec2::splat(64), 1, 1, None, None);
        let texture_atlas_layout = texture_atlas_layouts.add(layout);

        let Ok(player_transform) = player_query.single() else {
            return;
        };
        let Ok(crosshair_transform) = crosshair_query.single() else {
            return;
        };

        // let player_position = player_transform.translation.xy();
        // let cursor_position = crosshair_transform.translation.xy();

        // let direction = (cursor_position - player_position).normalize();

        // let speed = 1500.0; // Ajusta la velocidad de la bola
        // let velocity = direction * speed;

        // let offset = 32.0; // Se ajusta a la mitad del tamaño de la bola
        // let start_position = player_position + direction * offset;
        //
        let start_pos = player_transform.translation.xy();
        let target_pos = crosshair_transform.translation.xy();

        let speed = 1000.0;
        let gravity = 9.81 * 32.0; // px/s² (ajusta según tus unidades)
        let prefer_high_arc = false;

        if let Some(velocity) =
            solve_ballistic_velocity(start_pos, target_pos, speed, gravity, prefer_high_arc)
        {
            // desplazamiento hacia adelante (por ejemplo, 32 px)
            let offset_dist = 32.0;
            let offset_pos = start_pos + velocity.normalize() * offset_dist;

            commands.spawn((
                Sprite {
                    image: cursor_assets.wool_image.clone(),
                    texture_atlas: Some(TextureAtlas {
                        layout: texture_atlas_layout.clone(),
                        index: 0,
                    }),
                    ..default()
                },
                Transform::from_translation(offset_pos.extend(98.0)).with_scale(Vec3::splat(0.5)),
                WoolBall,
                Projectile {
                    velocity,
                    despawn_timer: Timer::from_seconds(2.0, TimerMode::Once),
                    has_collided: false,
                },
                Collider::ball(8.0), // Tamaño del colisionador de la bola
                RigidBody::Dynamic,
                Velocity {
                    linvel: velocity,
                    angvel: 0.0,
                },
                ActiveEvents::COLLISION_EVENTS, // Habilita la detección de colisiones
                AffectedByGravity,
                Mass { kilograms: 10.0 },
                Sleeping::default(),
            ));
        }
    }
}

/// Sistema para gestionar las colisiones de los proyectiles y el temporizador de desaparición.
pub fn handle_projectile_despawn(
    mut commands: Commands,
    mut projectile_query: Query<(Entity, &mut Projectile)>,
    mut collision_events: EventReader<CollisionEvent>,
    time: Res<Time>,
) {
    for event in collision_events.read() {
        if let CollisionEvent::Started(entity1, entity2, _) = event {
            // Revisa si alguna de las entidades es un proyectil.
            if let Ok((_, mut projectile)) = projectile_query.get_mut(*entity1) {
                if !projectile.has_collided {
                    projectile.has_collided = true;
                    println!("Projectile collided");
                    // Aquí puedes añadir efectos o sonido
                }
            }
            if let Ok((_, mut projectile)) = projectile_query.get_mut(*entity2) {
                if !projectile.has_collided {
                    projectile.has_collided = true;
                    println!("Projectile collided");
                    // Aquí puedes añadir efectos o sonido
                }
            }
        }
    }

    // Si un proyectil ha colisionado, su temporizador se inicia.
    for (entity, mut projectile) in projectile_query.iter_mut() {
        if projectile.has_collided {
            projectile.despawn_timer.tick(time.delta());
            if projectile.despawn_timer.finished() {
                commands.entity(entity).despawn();
            }
        }
    }
}
