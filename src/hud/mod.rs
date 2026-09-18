//! In-level HUD: three animated hearts (each worth two hit points, with a
//! half state), the kitty-point counter with its rolling-ball icon, a
//! clickable pause button, the boss health bar, the maniac-mode countdown
//! and the level title card shown on entry.

use bevy::prelude::*;

use crate::{
    collectibles::components::ManiacBuff,
    enemies::components::{EnemyCharacter, FinalBoss},
    game_state::{CurrentLevel, GameState, LevelState, PlayerStats},
    menu::MenuAssets,
    player::components::{Health, PlayerCharacter},
};

pub struct HudPlugin;

/// Raised when the on-screen pause button is clicked.
#[derive(Event, Default)]
pub struct PauseRequested;

/// True while the pointer is over any UI button, so gameplay systems can
/// ignore clicks that were meant for the HUD.
#[derive(Resource, Default)]
pub struct PointerOverUi(pub bool);

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<PauseRequested>()
            .init_resource::<PointerOverUi>()
            .add_systems(
                OnEnter(LevelState::LevelLoaded),
                (spawn_hud, spawn_level_title).after(crate::player::spawn_player_character),
            )
            .add_systems(
                Update,
                (
                    track_pointer_over_ui,
                    pause_button_system,
                    update_hearts,
                    animate_hearts,
                    update_score,
                    animate_score_ball,
                    update_boss_bar,
                    update_maniac_indicator,
                    animate_level_title,
                )
                    .run_if(in_state(GameState::Game))
                    .run_if(in_state(LevelState::LevelLoaded)),
            )
            .add_systems(OnExit(LevelState::LevelLoaded), despawn_hud);
    }
}

#[derive(Component)]
struct HudRoot;

#[derive(Component)]
struct HeartIcon {
    idx: u32,
    frame: usize,
    target: usize,
    timer: Timer,
}

#[derive(Component)]
struct ScoreText;

#[derive(Component)]
struct ScoreBall {
    frame: usize,
    rolling: bool,
    timer: Timer,
}

#[derive(Component)]
struct PauseButton;

#[derive(Component)]
struct BossBarRoot;

#[derive(Component)]
struct BossBarFill;

#[derive(Component)]
struct BossBarLabel;

#[derive(Component)]
struct ManiacIndicator;

#[derive(Component)]
struct ManiacText;

#[derive(Component)]
struct LevelTitle {
    timer: Timer,
}

// Corazon-Sheet.png: 21 frames, 0 = full, 10 = half, 20 = empty.
const HEART_FULL: usize = 0;
const HEART_HALF: usize = 10;
const HEART_EMPTY: usize = 20;
const HEART_SIZE: f32 = 50.0;
const HUD_MARGIN: f32 = 12.0;

fn heart_target(hp: u32, idx: u32) -> usize {
    if hp >= 2 * idx + 2 {
        HEART_FULL
    } else if hp == 2 * idx + 1 {
        HEART_HALF
    } else {
        HEART_EMPTY
    }
}

fn spawn_hud(
    mut commands: Commands,
    assets: Res<MenuAssets>,
    stats: Res<PlayerStats>,
    player: Query<&Health, With<PlayerCharacter>>,
) {
    let hp = player.single().map(|h| h.current).unwrap_or(6);
    let hearts = player.single().map(|h| h.max.div_ceil(2)).unwrap_or(3);

    let font = assets.text_font.clone();
    let shadow = TextShadow {
        offset: Vec2::new(2.0, 2.0),
        color: Color::srgba(1.0, 1.0, 1.0, 0.55),
    };

    commands
        .spawn((
            HudRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            Pickable::IGNORE,
            GlobalZIndex(10),
        ))
        .with_children(|root| {
            // --- Hearts (top-left) ---
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(HUD_MARGIN),
                    top: Val::Px(HUD_MARGIN),
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(4.0),
                    ..default()
                },
                Pickable::IGNORE,
            ))
            .with_children(|row| {
                for idx in 0..hearts {
                    let target = heart_target(hp, idx);
                    row.spawn((
                        HeartIcon {
                            idx,
                            frame: target,
                            target,
                            timer: Timer::from_seconds(1.0 / 30.0, TimerMode::Repeating),
                        },
                        ImageNode {
                            image: assets.heart_sheet.clone(),
                            texture_atlas: Some(TextureAtlas {
                                layout: assets.heart_layout.clone(),
                                index: target,
                            }),
                            ..default()
                        },
                        Node {
                            width: Val::Px(HEART_SIZE),
                            height: Val::Px(HEART_SIZE),
                            ..default()
                        },
                        Pickable::IGNORE,
                    ));
                }
            });

            // --- Maniac-mode countdown (under the hearts) ---
            root.spawn((
                ManiacIndicator,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(HUD_MARGIN + 6.0),
                    top: Val::Px(HUD_MARGIN + HEART_SIZE + 6.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(8.0),
                    ..default()
                },
                Visibility::Hidden,
                Pickable::IGNORE,
            ))
            .with_children(|row| {
                row.spawn((
                    ImageNode {
                        image: assets.treat.clone(),
                        ..default()
                    },
                    Node {
                        width: Val::Px(28.0),
                        height: Val::Px(28.0),
                        ..default()
                    },
                ));
                row.spawn((
                    ManiacText,
                    Text::new("10s"),
                    TextFont {
                        font: font.clone(),
                        font_size: 22.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.1, 0.1, 0.1)),
                    shadow,
                ));
            });

            // --- Score + pause (top-right) ---
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(HUD_MARGIN),
                    top: Val::Px(HUD_MARGIN),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(10.0),
                    ..default()
                },
                Pickable::IGNORE,
            ))
            .with_children(|row| {
                row.spawn((
                    ScoreText,
                    Text::new(format!("x{}", stats.kitty_points)),
                    TextFont {
                        font: font.clone(),
                        font_size: 44.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.05, 0.05, 0.05)),
                    shadow,
                    Pickable::IGNORE,
                ));
                row.spawn((
                    ScoreBall {
                        frame: 0,
                        rolling: false,
                        timer: Timer::from_seconds(1.0 / 14.0, TimerMode::Repeating),
                    },
                    ImageNode {
                        image: assets.ball_sheet.clone(),
                        texture_atlas: Some(TextureAtlas {
                            layout: assets.ball_layout.clone(),
                            index: 0,
                        }),
                        ..default()
                    },
                    Node {
                        width: Val::Px(56.0),
                        height: Val::Px(56.0),
                        ..default()
                    },
                    Pickable::IGNORE,
                ));
                row.spawn((
                    PauseButton,
                    Button,
                    ImageNode {
                        image: assets.pause_icon.clone(),
                        ..default()
                    },
                    Node {
                        width: Val::Px(44.0),
                        height: Val::Px(44.0),
                        margin: UiRect::left(Val::Px(8.0)),
                        ..default()
                    },
                ));
            });

            // --- Boss bar (bottom-center), hidden unless a boss is alive ---
            root.spawn((
                BossBarRoot,
                Node {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(18.0),
                    left: Val::Percent(50.0),
                    width: Val::Px(420.0),
                    margin: UiRect::left(Val::Px(-210.0)),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(4.0),
                    ..default()
                },
                Visibility::Hidden,
                Pickable::IGNORE,
            ))
            .with_children(|bar| {
                bar.spawn((
                    BossBarLabel,
                    Text::new("KIDD CAT"),
                    TextFont {
                        font: font.clone(),
                        font_size: 20.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    TextShadow::default(),
                ));
                bar.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(16.0),
                        border: UiRect::all(Val::Px(2.0)),
                        padding: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
                    BorderColor(Color::WHITE),
                ))
                .with_children(|outer| {
                    outer.spawn((
                        BossBarFill,
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.9, 0.2, 0.2)),
                    ));
                });
            });
        });
}

fn spawn_level_title(mut commands: Commands, assets: Res<MenuAssets>, level: Res<CurrentLevel>) {
    let (main, sub) = if level.0.is_final() {
        ("Final level".to_string(), "Kidd Cat is waiting".to_string())
    } else {
        (format!("Level {}", level.0.number()), String::new())
    };
    commands
        .spawn((
            HudRoot,
            LevelTitle {
                timer: Timer::from_seconds(2.6, TimerMode::Once),
            },
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(6.0),
                ..default()
            },
            Pickable::IGNORE,
            GlobalZIndex(11),
        ))
        .with_children(|col| {
            col.spawn((
                Text::new(main),
                TextFont {
                    font: assets.title_font.clone(),
                    font_size: 72.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                TextShadow {
                    offset: Vec2::new(4.0, 4.0),
                    color: Color::srgba(0.0, 0.0, 0.0, 0.7),
                },
            ));
            if !sub.is_empty() {
                col.spawn((
                    Text::new(sub),
                    TextFont {
                        font: assets.text_font.clone(),
                        font_size: 28.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    TextShadow::default(),
                ));
            }
        });
}

fn despawn_hud(mut commands: Commands, q: Query<Entity, With<HudRoot>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

fn track_pointer_over_ui(
    buttons: Query<&Interaction, With<Button>>,
    mut over: ResMut<PointerOverUi>,
) {
    let hovering = buttons
        .iter()
        .any(|i| matches!(i, Interaction::Hovered | Interaction::Pressed));
    if over.0 != hovering {
        over.0 = hovering;
    }
}

fn pause_button_system(
    mut q: Query<(&Interaction, &mut ImageNode), (Changed<Interaction>, With<PauseButton>)>,
    mut pause: EventWriter<PauseRequested>,
    mut sfx: EventWriter<crate::audio::SfxEvent>,
) {
    for (interaction, mut image) in &mut q {
        match interaction {
            Interaction::Pressed => {
                image.color = Color::srgb(0.7, 0.7, 0.7);
                sfx.write(crate::audio::SfxEvent::ButtonClick);
                pause.write(PauseRequested);
            }
            Interaction::Hovered => image.color = Color::srgb(1.0, 0.95, 0.8),
            Interaction::None => image.color = Color::WHITE,
        }
    }
}

fn update_hearts(
    player: Query<&Health, (With<PlayerCharacter>, Changed<Health>)>,
    mut hearts: Query<&mut HeartIcon>,
) {
    let Ok(health) = player.single() else {
        return;
    };
    for mut heart in &mut hearts {
        heart.target = heart_target(health.current, heart.idx);
    }
}

/// Steps each heart one frame per tick toward its target, so losing a
/// half-heart plays the crack animation and healing plays it in reverse.
fn animate_hearts(time: Res<Time>, mut hearts: Query<(&mut HeartIcon, &mut ImageNode)>) {
    for (mut heart, mut image) in &mut hearts {
        if heart.frame == heart.target {
            continue;
        }
        heart.timer.tick(time.delta());
        if !heart.timer.just_finished() {
            continue;
        }
        if heart.frame < heart.target {
            heart.frame += 1;
        } else {
            heart.frame -= 1;
        }
        if let Some(atlas) = image.texture_atlas.as_mut() {
            atlas.index = heart.frame;
        }
    }
}

fn update_score(
    stats: Res<PlayerStats>,
    mut text: Query<&mut Text, With<ScoreText>>,
    mut ball: Query<&mut ScoreBall>,
) {
    if !stats.is_changed() {
        return;
    }
    for mut t in &mut text {
        **t = format!("x{}", stats.kitty_points);
    }
    for mut b in &mut ball {
        b.rolling = true;
    }
}

/// Rolls the ball icon through its 8 frames once per point gained.
fn animate_score_ball(time: Res<Time>, mut balls: Query<(&mut ScoreBall, &mut ImageNode)>) {
    for (mut ball, mut image) in &mut balls {
        if !ball.rolling {
            continue;
        }
        ball.timer.tick(time.delta());
        if !ball.timer.just_finished() {
            continue;
        }
        ball.frame += 1;
        if ball.frame >= 8 {
            ball.frame = 0;
            ball.rolling = false;
        }
        if let Some(atlas) = image.texture_atlas.as_mut() {
            atlas.index = ball.frame;
        }
    }
}

fn update_boss_bar(
    bosses: Query<(&Health, &FinalBoss), With<EnemyCharacter>>,
    mut root: Query<&mut Visibility, With<BossBarRoot>>,
    mut fill: Query<(&mut Node, &mut BackgroundColor), With<BossBarFill>>,
    mut label: Query<&mut Text, With<BossBarLabel>>,
) {
    let Ok(mut vis) = root.single_mut() else {
        return;
    };
    let Some((health, boss)) = bosses.iter().next() else {
        *vis = Visibility::Hidden;
        return;
    };
    *vis = Visibility::Visible;
    let ratio = if health.max == 0 {
        0.0
    } else {
        health.current as f32 / health.max as f32
    };
    for (mut node, mut color) in &mut fill {
        node.width = Val::Percent(ratio * 100.0);
        color.0 = match boss.phase {
            1 => Color::srgb(0.9, 0.2, 0.2),
            2 => Color::srgb(1.0, 0.6, 0.1),
            _ => Color::srgb(0.7, 0.2, 0.9),
        };
    }
    for mut t in &mut label {
        let phase = match boss.phase {
            1 => "",
            2 => "  -  free fall",
            _ => "  -  last stand",
        };
        **t = format!("KIDD CAT{phase}");
    }
}

fn update_maniac_indicator(
    player: Query<Option<&ManiacBuff>, With<PlayerCharacter>>,
    mut root: Query<&mut Visibility, With<ManiacIndicator>>,
    mut text: Query<&mut Text, With<ManiacText>>,
) {
    let Ok(mut vis) = root.single_mut() else {
        return;
    };
    match player.single().ok().flatten() {
        Some(buff) => {
            *vis = Visibility::Visible;
            for mut t in &mut text {
                **t = format!("{:.0}s", buff.0.remaining_secs().ceil());
            }
        }
        None => *vis = Visibility::Hidden,
    }
}

/// Fades the title card in, holds it, fades it out and despawns it.
fn animate_level_title(
    mut commands: Commands,
    time: Res<Time>,
    mut titles: Query<(Entity, &mut LevelTitle, &Children)>,
    mut texts: Query<&mut TextColor>,
) {
    for (entity, mut title, children) in &mut titles {
        title.timer.tick(time.delta());
        let elapsed = title.timer.elapsed_secs();
        let total = title.timer.duration().as_secs_f32();
        let alpha = if elapsed < 0.3 {
            elapsed / 0.3
        } else if elapsed > total - 0.8 {
            ((total - elapsed) / 0.8).max(0.0)
        } else {
            1.0
        };
        for child in children.iter() {
            if let Ok(mut color) = texts.get_mut(child) {
                color.0 = color.0.with_alpha(alpha);
            }
        }
        if title.timer.finished() {
            commands.entity(entity).despawn();
        }
    }
}
