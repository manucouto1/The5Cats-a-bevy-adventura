//! `levelN_gaps.json` — the camera contract each level ships, plus the
//! level-4 "gap zones".
//!
//! The file's `name` field names one of the pygame camera classes and
//! decides how the rest of the file is read:
//!
//! * `Camera` — plain follow, clamped to the map (level 1).
//! * `CameraVerticalGap` — follow, but the camera's vertical travel is
//!   confined to the band of tile rows (`y_init`..`y_end`) the player is
//!   standing in. Crossing into the next band pans the camera over to it
//!   (levels 2 and 3; level 3 ships no bands, so it just follows).
//! * `FallingCamera` — rectangular zones carrying an `action`
//!   (`clear_mode` / `falling_mode` / `normal_mode`) that change both the
//!   camera framing and the level mode: `falling_mode` replaces gravity
//!   with a constant downward velocity so the player free-falls through a
//!   long vertical shaft (level 4).

use bevy::prelude::*;
use serde::Deserialize;

use bevy_rapier2d::prelude::{Collider, RigidBody};

use rand::Rng;

use crate::{
    enemies::components::{Active, EnemyCharacter, FinalBoss, ShaftDrift},
    game_state::CurrentLevel,
    map::{assets::GameAssets, components::LevelTile},
    parallax::systems::CameraMode,
    physics::{FallingMode, Velocity},
    player::components::{Dead, PlayerCharacter},
};

/// Current high-level gameplay mode dictated by the zone the player is in.
/// Defaults to `Normal`; gets flipped to `Falling` in level-4 shafts.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LevelMode {
    #[default]
    Normal,
    Clear,
    Falling,
}

/// Gate for level 4's falling shaft. The player only enters `FallingMode`
/// in a falling-type zone AFTER this flag is armed (set by the boss when
/// phase 1 ends). This enforces the narrative: the boss falls first, then
/// the player jumps in after.
#[derive(Resource, Default, Debug)]
pub struct BossFallArmed(pub bool);

/// Falling shaft speed — the player sinks at this rate while `LevelMode`
/// is `Falling`. Pygame replaced gravity with a slow-motion drift (the
/// camera crawled down at ~60 px/s dragging the hero); this keeps that
/// dreamy pace while W/S let the player speed up or slow down.
pub const FALLING_SPEED: f32 = 140.0;
/// Other cats sink a bit slower than the hero (pygame: 1.5 vs 2.5) so the
/// player drifts down onto them and has to steer around.
pub const ENEMY_FALLING_SPEED: f32 = 90.0;
/// How far below the player the camera looks while falling, so what is
/// coming up from below is visible.
pub const FALLING_LOOK_AHEAD: f32 = 130.0;
/// Horizontal drift speed of shaft enemies homing toward the player.
const ENEMY_DRIFT_SPEED: f32 = 55.0;
/// How far above the hero a shaft cat may still be and count as "not yet
/// passed", in px.
const PASSED_MARGIN: f32 = 32.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoneAction {
    Normal,
    Clear,
    Falling,
}

/// Which camera axes a zone pins (pygame `center` = "x" | "y" | "xy").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CenterMode {
    None,
    X,
    Y,
    Xy,
}

#[derive(Clone)]
pub struct Zone {
    pub action: ZoneAction,
    /// AABB in WORLD coordinates (not tiles).
    pub bounds: Rect,
    pub center_mode: CenterMode,
    /// Camera anchor in WORLD coordinates (from `x_center` / `y_center`).
    pub center: Vec2,
}

/// A band of tile rows the camera is not allowed to leave while the player
/// is inside it (pygame's `CameraVerticalGap`). World coordinates.
#[derive(Debug, Clone, Copy)]
pub struct VerticalBand {
    pub min_y: f32,
    pub max_y: f32,
}

impl VerticalBand {
    fn contains(&self, y: f32) -> bool {
        y >= self.min_y && y <= self.max_y
    }

    fn distance_to(&self, y: f32) -> f32 {
        (self.min_y - y).max(y - self.max_y).max(0.0)
    }
}

/// Which pygame camera class the level's gaps file asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CameraKind {
    /// `src.sprites.groups.camera.Camera`
    #[default]
    Plain,
    /// `src.sprites.groups.camera.CameraVerticalGap`
    VerticalGap,
    /// `src.sprites.groups.camera.FallingCamera`
    Falling,
}

#[derive(Resource, Default, Clone)]
pub struct LevelZones {
    pub kind: CameraKind,
    pub zones: Vec<Zone>,
    /// Vertical bands, only meaningful for `CameraKind::VerticalGap`.
    pub bands: Vec<VerticalBand>,
}

/// Band the camera is currently confined to. Sticky: while the player is in
/// the seam between two bands (levels declare `0..25` and `26..50`, leaving
/// one tile row uncovered) the camera keeps the band it already had instead
/// of flip-flopping across the gap.
#[derive(Resource, Default, Debug)]
pub struct ActiveBand(pub Option<usize>);

#[derive(Deserialize)]
struct GapsFile {
    #[serde(default)]
    name: Option<String>,
    gaps: Vec<GapEntry>,
}

#[derive(Deserialize)]
struct GapEntry {
    #[serde(default)]
    action: Option<String>,
    #[serde(default)]
    x_init: Option<u32>,
    #[serde(default)]
    x_end: Option<u32>,
    #[serde(default)]
    y_init: Option<u32>,
    #[serde(default)]
    y_end: Option<u32>,
    #[serde(default)]
    center: Option<String>,
    #[serde(default)]
    x_center: Option<f32>,
    #[serde(default)]
    y_center: Option<f32>,
}

/// Marker for the invisible wall that keeps the player (and the boss) in
/// the arena until the boss finishes phase 1.
#[derive(Component)]
pub struct ShaftSeal;

/// Parses `levelN_gaps.json` into the `LevelZones` resource: the camera
/// class asked for by `name`, the action-bearing rectangles (level 4) as
/// world-space AABBs, and the plain `y_init`/`y_end` entries as vertical
/// camera bands (level 2).
pub fn load_level_zones(
    mut commands: Commands,
    current_level: Res<CurrentLevel>,
    game_assets: Res<GameAssets>,
) {
    let paths = current_level.0.get_path();
    commands.insert_resource(ActiveBand::default());
    let Ok(raw) = std::fs::read_to_string(&paths.gaps) else {
        commands.insert_resource(LevelZones::default());
        return;
    };
    let parsed: GapsFile = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            warn!("Failed to parse gaps file {}: {e}", paths.gaps);
            commands.insert_resource(LevelZones::default());
            return;
        }
    };

    // The pygame class name is fully qualified
    // (`src.sprites.groups.camera.X`); match on the last segment.
    let kind = match parsed.name.as_deref().and_then(|n| n.rsplit('.').next()) {
        Some("FallingCamera") => CameraKind::Falling,
        Some("CameraVerticalGap") => CameraKind::VerticalGap,
        Some("Camera") => CameraKind::Plain,
        other => {
            if let Some(name) = other {
                warn!(
                    "Unknown camera class {name:?} in {}, using plain follow",
                    paths.gaps
                );
            }
            CameraKind::Plain
        }
    };

    let tile_size = game_assets.tile_size_px;
    let map_w = game_assets.map_width_tiles as f32;
    let map_h = game_assets.map_height_tiles as f32;
    let offset_x = -(map_w * tile_size / 2.0);
    let offset_y = map_h * tile_size / 2.0;

    // Tile row -> world Y of that row's top edge.
    let row_top = |row: f32| -(row * tile_size) + offset_y;

    let mut zones = Vec::new();
    let mut bands = Vec::new();
    for g in parsed.gaps {
        let action = match g.action.as_deref() {
            Some("falling_mode") => ZoneAction::Falling,
            Some("clear_mode") => ZoneAction::Clear,
            Some("normal_mode") => ZoneAction::Normal,
            // No action: a bare Y range, i.e. a `CameraVerticalGap` band.
            _ => {
                if let (Some(yi), Some(ye)) = (g.y_init, g.y_end) {
                    bands.push(VerticalBand {
                        min_y: row_top(ye as f32),
                        max_y: row_top(yi as f32),
                    });
                }
                continue;
            }
        };
        let (Some(xi), Some(xe), Some(yi), Some(ye)) = (g.x_init, g.x_end, g.y_init, g.y_end)
        else {
            continue;
        };
        // Convert tile-space AABB to world-space. The level 4 mapping:
        // tile y grows downward, world y grows upward, so y_init (smaller
        // tile y) maps to the HIGHER world y (max), and vice versa.
        let min_x = xi as f32 * tile_size + offset_x;
        let max_x = xe as f32 * tile_size + offset_x;
        let max_y = -(yi as f32 * tile_size) + offset_y;
        let min_y = -(ye as f32 * tile_size) + offset_y;
        let bounds = Rect::new(min_x, min_y, max_x, max_y);
        let center_mode = match g.center.as_deref() {
            Some("x") => CenterMode::X,
            Some("y") => CenterMode::Y,
            Some("xy") => CenterMode::Xy,
            _ => CenterMode::None,
        };
        let center = Vec2::new(
            g.x_center
                .map(|x| x * tile_size + offset_x)
                .unwrap_or(bounds.center().x),
            g.y_center
                .map(|y| -(y * tile_size) + offset_y)
                .unwrap_or(bounds.center().y),
        );
        zones.push(Zone {
            action,
            bounds,
            center_mode,
            center,
        });
    }

    info!(
        "Camera {kind:?}: {} action zones, {} vertical bands",
        zones.len(),
        bands.len()
    );
    commands.insert_resource(LevelZones { kind, zones, bands });
}

/// Spawns the invisible wall at the arena/shaft boundary on levels that
/// have a falling zone. Removed by `unseal_shaft_system` once the boss
/// dives (pygame's `block_right`).
pub fn spawn_shaft_seal(mut commands: Commands, zones: Res<LevelZones>) {
    let Some(shaft) = zones.zones.iter().find(|z| z.action == ZoneAction::Falling) else {
        return;
    };
    let height = shaft.bounds.height();
    commands.spawn((
        ShaftSeal,
        LevelTile,
        RigidBody::Fixed,
        Collider::cuboid(6.0, height / 2.0),
        Transform::from_xyz(shaft.bounds.min.x - 6.0, shaft.bounds.center().y, 0.0),
    ));
}

pub fn unseal_shaft_system(
    mut commands: Commands,
    armed: Res<BossFallArmed>,
    seals: Query<Entity, With<ShaftSeal>>,
) {
    if !armed.0 {
        return;
    }
    for seal in &seals {
        commands.entity(seal).despawn();
    }
}

/// Camera framing dictated by the zone the player stands in (pygame's
/// `FallingCamera`): the arena and the bottom pit are fixed shots, the
/// shaft locks the camera to its width and looks a little ahead of the
/// fall.
///
/// The arena keeps its shot until the player walks into the shaft. It used
/// to pan over the moment the boss armed the dive, "so the player sees
/// where it went" — but that fires while the hero is still on the arena
/// floor, so the camera left the player off-screen before the boss had even
/// jumped. The order that reads correctly is: the boss dives, the player
/// walks to the edge, and the shaft zone takes the camera when they step
/// into it.
fn camera_mode_for(zone: &Zone, shaft: Option<&Zone>) -> CameraMode {
    // Pygame framed these shots in an 800x800 window; `y_center` assumed a
    // 400 px half-height. Our view is shorter, so keep the *bottom* edge
    // where the original had it (that's where the floor is) by shifting
    // the center up by the difference.
    let fixed_y = |zone: &Zone, view_h: f32| zone.center.y - (PYGAME_VIEW - view_h) / 2.0;
    let shaft_width = shaft.map(|s| s.bounds.width());
    let zoomed_h = shaft_width.map(|w| w * 9.0 / 16.0);
    match (zone.action, zone.center_mode) {
        (ZoneAction::Falling, _) => CameraMode::LockX {
            x: zone.bounds.center().x,
            look_ahead_y: FALLING_LOOK_AHEAD,
            view_width: Some(zone.bounds.width()),
        },
        (ZoneAction::Normal, CenterMode::Xy) => CameraMode::Fixed {
            point: Vec2::new(
                shaft.map(|s| s.bounds.center().x).unwrap_or(zone.center.x),
                fixed_y(zone, zoomed_h.unwrap_or(crate::VIEW_HEIGHT)),
            ),
            view_width: shaft_width,
        },
        (_, CenterMode::Xy) => CameraMode::Fixed {
            point: Vec2::new(zone.center.x, fixed_y(zone, crate::VIEW_HEIGHT)),
            view_width: None,
        },
        (_, CenterMode::X) => CameraMode::LockX {
            x: zone.center.x,
            look_ahead_y: 0.0,
            view_width: None,
        },
        _ => CameraMode::Follow,
    }
}

/// Height (and width) of the original pygame viewport in px.
const PYGAME_VIEW: f32 = 800.0;

/// Picks the band the camera is confined to for a given player height.
/// Sticky: levels declare bands as `0..25` and `26..50`, so one tile row
/// belongs to neither — inside that seam the previously active band is
/// kept rather than letting the camera flip between the two.
fn band_for(bands: &[VerticalBand], y: f32, active: &mut ActiveBand) -> VerticalBand {
    let index = bands
        .iter()
        .position(|band| band.contains(y))
        .or(active.0.filter(|i| *i < bands.len()))
        .or_else(|| {
            bands
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| a.distance_to(y).total_cmp(&b.distance_to(y)))
                .map(|(i, _)| i)
        })
        .unwrap_or(0);
    active.0 = Some(index);
    bands[index]
}

/// Each frame, works out how the camera should frame the action and — on
/// `FallingCamera` levels — which zone the player is in, updating
/// `LevelMode` accordingly. The player's `FallingMode` component is
/// added/removed to match, but only when `BossFallArmed` is set, which the
/// boss triggers when it finishes phase 1.
pub fn level_zone_trigger_system(
    mut commands: Commands,
    zones: Option<Res<LevelZones>>,
    mut level_mode: ResMut<LevelMode>,
    mut camera_mode: ResMut<CameraMode>,
    mut active_band: ResMut<ActiveBand>,
    armed: Res<BossFallArmed>,
    player: Query<(Entity, &Transform, Has<FallingMode>), (With<PlayerCharacter>, Without<Dead>)>,
) {
    let Some(zones) = zones else { return };
    let Ok((player_entity, tf, has_falling)) = player.single() else {
        return;
    };
    let pos = tf.translation.truncate();

    // Everything but `FallingCamera` is a pure camera contract: no level
    // modes, no gravity changes.
    if zones.kind != CameraKind::Falling || zones.zones.is_empty() {
        if *level_mode != LevelMode::Normal {
            *level_mode = LevelMode::Normal;
        }
        if has_falling {
            commands.entity(player_entity).remove::<FallingMode>();
        }
        let wanted = match zones.kind {
            CameraKind::VerticalGap if !zones.bands.is_empty() => {
                let band = band_for(&zones.bands, pos.y, &mut active_band);
                CameraMode::Banded {
                    min_y: band.min_y,
                    max_y: band.max_y,
                }
            }
            _ => CameraMode::Follow,
        };
        if *camera_mode != wanted {
            *camera_mode = wanted;
        }
        return;
    }

    let shaft = zones.zones.iter().find(|z| z.action == ZoneAction::Falling);

    let current_zone = zones.zones.iter().find(|z| z.bounds.contains(pos));
    let new_mode = current_zone
        .map(|z| match z.action {
            ZoneAction::Falling => LevelMode::Falling,
            ZoneAction::Clear => LevelMode::Clear,
            ZoneAction::Normal => LevelMode::Normal,
        })
        .unwrap_or(*level_mode);
    if new_mode != *level_mode {
        *level_mode = new_mode;
    }
    if let Some(zone) = current_zone {
        let wanted = camera_mode_for(zone, shaft);
        if *camera_mode != wanted {
            *camera_mode = wanted;
        }
    }

    // Falling mode only applies to the player after the boss has armed the
    // shaft (phase 1 complete). Before that the seal wall keeps the player
    // out anyway.
    let should_fall = matches!(*level_mode, LevelMode::Falling) && armed.0;
    match (should_fall, has_falling) {
        (true, false) => {
            commands.entity(player_entity).insert(FallingMode {
                speed: FALLING_SPEED,
            });
        }
        (false, true) => {
            commands.entity(player_entity).remove::<FallingMode>();
        }
        _ => {}
    }
}

/// While the level is in falling mode, the cats the hero is falling *onto*
/// sink in slow motion too and drift sideways toward them, so the descent
/// is a gauntlet closing in rather than a static field. Leaving falling
/// mode hands them back to normal gravity.
///
/// Only cats still below the hero sink. They fall at two thirds of the
/// hero's speed, so one that keeps sinking after being passed travels
/// nearly as far as the hero does — the whole shaft used to end up stacked
/// at the bottom instead of spread along the drop. Once the hero is past,
/// the cat stays where it was left.
pub fn enemy_falling_mode_system(
    mut commands: Commands,
    time: Res<Time>,
    level_mode: Res<LevelMode>,
    armed: Res<BossFallArmed>,
    player: Query<&Transform, (With<PlayerCharacter>, Without<EnemyCharacter>)>,
    mut enemies: Query<
        (
            Entity,
            &Transform,
            &mut Velocity,
            Option<&mut FallingMode>,
            Has<Active>,
            Option<&ShaftDrift>,
        ),
        (With<EnemyCharacter>, Without<FinalBoss>),
    >,
) {
    let falling = *level_mode == LevelMode::Falling && armed.0;
    let player_pos = player.single().map(|tf| tf.translation.truncate()).ok();
    let t = time.elapsed_secs();
    let mut rng = rand::thread_rng();
    for (entity, tf, mut velocity, falling_mode, active, drift) in &mut enemies {
        if !falling {
            // Out of the shaft sequence entirely: hand everyone back to
            // normal gravity.
            if falling_mode.is_some() {
                commands
                    .entity(entity)
                    .remove::<FallingMode>()
                    .remove::<ShaftDrift>();
            }
            continue;
        }

        // A little slack so a cat level with the hero still drifts.
        let ahead = player_pos
            .map(|p| tf.translation.y < p.y + PASSED_MARGIN)
            .unwrap_or(false);
        // A cat the hero has already passed hangs where it was left. It
        // keeps `FallingMode` at zero speed rather than losing it: dropping
        // the component hands it to gravity, and a cat in free fall down an
        // empty shaft outruns the hero and ends up stacked at the bottom
        // with all the others.
        let sink = if ahead {
            ENEMY_FALLING_SPEED * rng.gen_range(0.7..1.3)
        } else {
            0.0
        };

        match falling_mode {
            Some(mut mode) => {
                // Only re-roll the speed when crossing between hanging and
                // sinking, so an approaching cat keeps a steady pace.
                if (mode.speed == 0.0) != (sink == 0.0) {
                    mode.speed = sink;
                }
            }
            None if active => {
                commands.entity(entity).insert((
                    FallingMode { speed: sink },
                    ShaftDrift {
                        phase: rng.gen_range(0.0..std::f32::consts::TAU),
                        sway: rng.gen_range(20.0..70.0),
                    },
                ));
            }
            None => {}
        }

        if active && ahead {
            if let Some(px) = player_pos.map(|p| p.x) {
                let dx = px - tf.translation.x;
                let (phase, sway) = drift.map(|d| (d.phase, d.sway)).unwrap_or((0.0, 0.0));
                let weave = (t * 1.6 + phase).sin() * sway;
                let home = if dx.abs() > 12.0 {
                    dx.signum() * ENEMY_DRIFT_SPEED
                } else {
                    0.0
                };
                velocity.velocity.x = home + weave;
            }
        } else if falling_mode_is_hanging(sink) {
            velocity.velocity.x = 0.0;
        }
    }
}

/// Reads better at the call site than comparing a float to zero inline.
fn falling_mode_is_hanging(sink: f32) -> bool {
    sink == 0.0
}

/// Returns the horizontal center of the falling-zone rectangle in world
/// coordinates (or `None` if no falling zone is defined). Used by the
/// boss to teleport to the top of the shaft when it starts falling.
pub fn falling_zone_center_x(zones: &LevelZones) -> Option<f32> {
    zones
        .zones
        .iter()
        .find(|z| z.action == ZoneAction::Falling)
        .map(|z| (z.bounds.min.x + z.bounds.max.x) / 2.0)
}
