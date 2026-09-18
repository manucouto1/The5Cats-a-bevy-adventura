//! Menu layer: main menu, controls, options, level select, pause, game over
//! and victory screens. One `MenuPage` resource decides what is on screen;
//! changing it rebuilds the widget tree. All pages share the same button
//! style (the pygame button PNGs + Purisa Bold labels).

mod assets;
pub mod components;

pub use assets::MenuAssets;
pub use components::MenuPage;

use crate::{
    audio::{AudioSettings, SfxEvent, VOLUME_STEP},
    game_state::{CurrentLevel, GameState, Level, LevelState, PlayMode, PlayerStats},
    menu::components::{MenuButtonAction, MenuWidget, VolumeReadout},
};
use bevy::{prelude::*, window::PrimaryWindow};

const HOVERED_BUTTON_SCALE: f32 = 1.08;
const PRESSED_TINT: Color = Color::srgb(0.6, 0.6, 0.6);
const TITLE_COLOR: Color = Color::WHITE;
const LABEL_COLOR: Color = Color::srgb(0.05, 0.05, 0.05);
const BUTTON_W: f32 = 200.0;
const BUTTON_H: f32 = 62.0;
const TITLE_SIZE: f32 = 78.0;
const LABEL_SIZE: f32 = 26.0;

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MenuPage>()
            .add_systems(Startup, assets::load_menu_assets)
            .add_systems(OnEnter(GameState::MainMenu), set_page(MenuPage::Main))
            .add_systems(OnEnter(GameState::PauseMenu), set_page(MenuPage::Pause))
            .add_systems(OnEnter(GameState::GameOver), set_page(MenuPage::GameOver))
            .add_systems(OnEnter(GameState::Victory), set_page(MenuPage::Victory))
            .add_systems(
                Update,
                (
                    rebuild_menu_on_page_change,
                    menu_button_system,
                    menu_keyboard_system,
                    update_volume_readouts,
                )
                    .run_if(in_menu_state),
            )
            .add_systems(OnExit(GameState::MainMenu), despawn_menu)
            .add_systems(OnExit(GameState::PauseMenu), despawn_menu)
            .add_systems(OnExit(GameState::GameOver), despawn_menu)
            .add_systems(OnExit(GameState::Victory), despawn_menu);
    }
}

fn in_menu_state(state: Res<State<GameState>>) -> bool {
    matches!(
        state.get(),
        GameState::MainMenu | GameState::PauseMenu | GameState::GameOver | GameState::Victory
    )
}

fn set_page(page: MenuPage) -> impl Fn(ResMut<MenuPage>) {
    move |mut current: ResMut<MenuPage>| {
        // Deref-mut on purpose: even re-entering the same page must mark the
        // resource changed so the tree is rebuilt after a state round-trip.
        *current = page;
    }
}

fn despawn_menu(mut commands: Commands, menu_query: Query<Entity, With<MenuWidget>>) {
    for entity in menu_query.iter() {
        commands.entity(entity).despawn();
    }
}

/// The page that "Back" returns to: the main menu when we're in the main
/// menu state, the pause page when the game is paused underneath.
fn home_page(state: &GameState) -> MenuPage {
    match state {
        GameState::PauseMenu => MenuPage::Pause,
        _ => MenuPage::Main,
    }
}

fn rebuild_menu_on_page_change(
    mut commands: Commands,
    page: Res<MenuPage>,
    assets: Res<MenuAssets>,
    settings: Res<AudioSettings>,
    stats: Res<PlayerStats>,
    current_level: Res<CurrentLevel>,
    window: Query<&Window, With<PrimaryWindow>>,
    existing: Query<Entity, With<MenuWidget>>,
) {
    if !page.is_changed() {
        return;
    }
    for entity in &existing {
        commands.entity(entity).despawn();
    }
    let win_size = window
        .single()
        .map(|w| Vec2::new(w.width(), w.height()))
        .unwrap_or(Vec2::new(1280.0, 720.0));

    match *page {
        MenuPage::Main => spawn_main(&mut commands, &assets, win_size),
        MenuPage::Controls => spawn_controls(&mut commands, &assets, win_size),
        MenuPage::Options => spawn_options(&mut commands, &assets, &settings, win_size),
        MenuPage::Levels => spawn_levels(&mut commands, &assets, win_size),
        MenuPage::Pause => spawn_pause(&mut commands, &assets),
        MenuPage::GameOver => spawn_game_over(&mut commands, &assets, current_level.0, win_size),
        MenuPage::Victory => spawn_victory(&mut commands, &assets, &stats, win_size),
    }
}

// ---------------------------------------------------------------------------
// Building blocks
// ---------------------------------------------------------------------------

/// Full-window root. `background` is drawn "cover" style: scaled so it fills
/// the window while keeping its aspect ratio, cropped on the long axis.
fn spawn_root(
    commands: &mut Commands,
    background: Option<(&Handle<Image>, Vec2)>,
    overlay: Option<Color>,
    win_size: Vec2,
) -> Entity {
    let mut root = commands.spawn((
        MenuWidget,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            overflow: Overflow::clip(),
            ..default()
        },
        GlobalZIndex(50),
    ));
    if let Some(color) = overlay {
        root.insert(BackgroundColor(color));
    }
    let root_id = root.id();

    if let Some((image, image_size)) = background {
        let scale = (win_size.x / image_size.x).max(win_size.y / image_size.y);
        let size = image_size * scale;
        let offset = (win_size - size) * 0.5;
        commands.entity(root_id).with_children(|parent| {
            parent.spawn((
                ImageNode {
                    image: image.clone(),
                    ..default()
                },
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(offset.x),
                    top: Val::Px(offset.y),
                    width: Val::Px(size.x),
                    height: Val::Px(size.y),
                    ..default()
                },
            ));
        });
    }
    root_id
}

/// Centered vertical column that holds a page's content.
fn column() -> Node {
    Node {
        position_type: PositionType::Absolute,
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        row_gap: Val::Px(14.0),
        ..default()
    }
}

fn title(assets: &MenuAssets, text: &str) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font: assets.title_font.clone(),
            font_size: TITLE_SIZE,
            ..default()
        },
        TextColor(TITLE_COLOR),
        TextShadow {
            offset: Vec2::new(3.0, 3.0),
            color: Color::srgba(0.0, 0.0, 0.0, 0.6),
        },
        Node {
            margin: UiRect::bottom(Val::Px(24.0)),
            ..default()
        },
    )
}

fn label(assets: &MenuAssets, text: &str, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font: assets.text_font.clone(),
            font_size: size,
            ..default()
        },
        TextColor(color),
    )
}

fn spawn_button(
    parent: &mut ChildSpawnerCommands,
    assets: &MenuAssets,
    image: &Handle<Image>,
    text: &str,
    action: MenuButtonAction,
) {
    parent
        .spawn((
            Button,
            action,
            ImageNode {
                image: image.clone(),
                ..default()
            },
            Node {
                width: Val::Px(BUTTON_W),
                height: Val::Px(BUTTON_H),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
        ))
        .with_children(|b| {
            b.spawn(label(assets, text, LABEL_SIZE, LABEL_COLOR));
        });
}

/// Square icon button (volume up/down).
fn spawn_icon_button(
    parent: &mut ChildSpawnerCommands,
    image: &Handle<Image>,
    action: MenuButtonAction,
) {
    parent.spawn((
        Button,
        action,
        ImageNode {
            image: image.clone(),
            ..default()
        },
        Node {
            width: Val::Px(50.0),
            height: Val::Px(50.0),
            ..default()
        },
    ));
}

// ---------------------------------------------------------------------------
// Pages
// ---------------------------------------------------------------------------

const MENU_BG_SIZE: Vec2 = Vec2::new(820.0, 800.0);

fn spawn_main(commands: &mut Commands, assets: &MenuAssets, win_size: Vec2) {
    let root = spawn_root(
        commands,
        Some((&assets.background, MENU_BG_SIZE)),
        None,
        win_size,
    );
    commands.entity(root).with_children(|parent| {
        parent.spawn(column()).with_children(|col| {
            col.spawn(title(assets, "5Gatos"));
            spawn_button(
                col,
                assets,
                &assets.button_green,
                "Play",
                MenuButtonAction::Play,
            );
            spawn_button(
                col,
                assets,
                &assets.button_yellow,
                "Controls",
                MenuButtonAction::Controls,
            );
            spawn_button(
                col,
                assets,
                &assets.button_pink,
                "Options",
                MenuButtonAction::Options,
            );
            spawn_button(
                col,
                assets,
                &assets.button_green,
                "Levels",
                MenuButtonAction::Levels,
            );
            spawn_button(
                col,
                assets,
                &assets.button_lilac,
                "Exit",
                MenuButtonAction::Exit,
            );
        });
    });
}

fn spawn_controls(commands: &mut Commands, assets: &MenuAssets, win_size: Vec2) {
    let root = spawn_root(
        commands,
        Some((&assets.background, MENU_BG_SIZE)),
        None,
        win_size,
    );
    commands.entity(root).with_children(|parent| {
        parent
            .spawn(Node {
                row_gap: Val::Px(18.0),
                ..column()
            })
            .with_children(|col| {
                col.spawn(title(assets, "Controls"));

                let rows: [(&Handle<Image>, Vec2, &str); 4] = [
                    (
                        &assets.arrows,
                        Vec2::new(120.0, 90.0),
                        "Move: WASD or the arrow keys",
                    ),
                    (&assets.mouse, Vec2::new(90.0, 90.0), "Aim and shoot: mouse"),
                    (
                        &assets.click,
                        Vec2::new(90.0, 90.0),
                        "Jump: Up, W or Space bar",
                    ),
                    (
                        &assets.click,
                        Vec2::new(90.0, 90.0),
                        "Double jump: jump again mid-air",
                    ),
                ];
                for (icon, size, text) in rows {
                    col.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(24.0),
                        width: Val::Px(720.0),
                        ..default()
                    })
                    .with_children(|row| {
                        row.spawn((
                            ImageNode {
                                image: icon.clone(),
                                ..default()
                            },
                            Node {
                                width: Val::Px(size.x),
                                height: Val::Px(size.y),
                                ..default()
                            },
                        ));
                        row.spawn(label(assets, text, LABEL_SIZE, TITLE_COLOR));
                    });
                }
                col.spawn(label(
                    assets,
                    "Esc: pause      F11: fullscreen      F12: screenshot",
                    20.0,
                    TITLE_COLOR,
                ));
                col.spawn(Node {
                    height: Val::Px(10.0),
                    ..default()
                });
                spawn_button(
                    col,
                    assets,
                    &assets.button_orange,
                    "Back",
                    MenuButtonAction::Back,
                );
            });
    });
}

fn spawn_options(
    commands: &mut Commands,
    assets: &MenuAssets,
    settings: &AudioSettings,
    win_size: Vec2,
) {
    let root = spawn_root(
        commands,
        Some((&assets.background, MENU_BG_SIZE)),
        None,
        win_size,
    );
    commands.entity(root).with_children(|parent| {
        parent
            .spawn(Node {
                row_gap: Val::Px(26.0),
                ..column()
            })
            .with_children(|col| {
                col.spawn(title(assets, "Options"));

                let rows = [
                    (
                        "Music volume",
                        VolumeReadout::Music,
                        settings.music_steps(),
                        MenuButtonAction::MusicDown,
                        MenuButtonAction::MusicUp,
                    ),
                    (
                        "Sound volume",
                        VolumeReadout::Sfx,
                        settings.sfx_steps(),
                        MenuButtonAction::SfxDown,
                        MenuButtonAction::SfxUp,
                    ),
                ];
                for (text, readout, value, down, up) in rows {
                    col.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(20.0),
                        width: Val::Px(560.0),
                        justify_content: JustifyContent::SpaceBetween,
                        ..default()
                    })
                    .with_children(|row| {
                        row.spawn((
                            label(assets, text, LABEL_SIZE, TITLE_COLOR),
                            Node {
                                width: Val::Px(230.0),
                                ..default()
                            },
                        ));
                        spawn_icon_button(row, &assets.volume_down, down);
                        row.spawn((
                            label(assets, &format!("{value}"), 32.0, TITLE_COLOR),
                            readout,
                            Node {
                                width: Val::Px(50.0),
                                justify_content: JustifyContent::Center,
                                ..default()
                            },
                            TextLayout::new_with_justify(JustifyText::Center),
                        ));
                        spawn_icon_button(row, &assets.volume_up, up);
                    });
                }
                col.spawn(Node {
                    height: Val::Px(20.0),
                    ..default()
                });
                spawn_button(
                    col,
                    assets,
                    &assets.button_orange,
                    "Back",
                    MenuButtonAction::Back,
                );
            });
    });
}

fn spawn_levels(commands: &mut Commands, assets: &MenuAssets, win_size: Vec2) {
    let root = spawn_root(
        commands,
        Some((&assets.background, MENU_BG_SIZE)),
        None,
        win_size,
    );
    commands.entity(root).with_children(|parent| {
        parent.spawn(column()).with_children(|col| {
            col.spawn(title(assets, "Levels"));
            spawn_button(
                col,
                assets,
                &assets.button_lilac,
                "Level 1",
                MenuButtonAction::StartLevel(1),
            );
            spawn_button(
                col,
                assets,
                &assets.button_pink,
                "Level 2",
                MenuButtonAction::StartLevel(2),
            );
            spawn_button(
                col,
                assets,
                &assets.button_green,
                "Level 3",
                MenuButtonAction::StartLevel(3),
            );
            spawn_button(
                col,
                assets,
                &assets.button_yellow,
                "Level 4",
                MenuButtonAction::StartLevel(4),
            );
            spawn_button(
                col,
                assets,
                &assets.button_orange,
                "Back",
                MenuButtonAction::Back,
            );
        });
    });
}

fn spawn_pause(commands: &mut Commands, assets: &MenuAssets) {
    let root = spawn_root(
        commands,
        None,
        Some(Color::srgba(0.02, 0.02, 0.08, 0.62)),
        Vec2::ZERO,
    );
    commands.entity(root).with_children(|parent| {
        parent.spawn(column()).with_children(|col| {
            col.spawn((
                ImageNode {
                    image: assets.pause_icon.clone(),
                    ..default()
                },
                Node {
                    width: Val::Px(72.0),
                    height: Val::Px(72.0),
                    margin: UiRect::bottom(Val::Px(6.0)),
                    ..default()
                },
            ));
            col.spawn(title(assets, "Paused"));
            spawn_button(
                col,
                assets,
                &assets.button_green,
                "Resume",
                MenuButtonAction::Resume,
            );
            spawn_button(
                col,
                assets,
                &assets.button_yellow,
                "Controls",
                MenuButtonAction::Controls,
            );
            spawn_button(
                col,
                assets,
                &assets.button_pink,
                "Options",
                MenuButtonAction::Options,
            );
            spawn_button(
                col,
                assets,
                &assets.button_lilac,
                "Main menu",
                MenuButtonAction::MainMenu,
            );
        });
    });
}

fn spawn_game_over(commands: &mut Commands, assets: &MenuAssets, level: Level, win_size: Vec2) {
    let root = spawn_root(
        commands,
        Some((&assets.game_over_background, MENU_BG_SIZE)),
        None,
        win_size,
    );
    commands.entity(root).with_children(|parent| {
        parent.spawn(column()).with_children(|col| {
            col.spawn(title(assets, "Game Over"));
            col.spawn((
                label(
                    assets,
                    &format!("Level {}", level.number()),
                    LABEL_SIZE,
                    TITLE_COLOR,
                ),
                Node {
                    margin: UiRect::bottom(Val::Px(20.0)),
                    ..default()
                },
            ));
            spawn_button(
                col,
                assets,
                &assets.button_green,
                "Play again",
                MenuButtonAction::PlayAgain,
            );
            spawn_button(
                col,
                assets,
                &assets.button_lilac,
                "Main menu",
                MenuButtonAction::MainMenu,
            );
        });
    });
}

fn spawn_victory(
    commands: &mut Commands,
    assets: &MenuAssets,
    stats: &PlayerStats,
    win_size: Vec2,
) {
    let root = spawn_root(
        commands,
        Some((&assets.background, MENU_BG_SIZE)),
        None,
        win_size,
    );
    commands.entity(root).with_children(|parent| {
        parent
            .spawn(Node {
                row_gap: Val::Px(12.0),
                ..column()
            })
            .with_children(|col| {
                col.spawn(title(assets, "VICTORY!!"));
                col.spawn((
                    label(assets, "Anything is paw-sible for you", 36.0, TITLE_COLOR),
                    Node {
                        margin: UiRect::bottom(Val::Px(16.0)),
                        ..default()
                    },
                ));

                let ball = ImageNode {
                    image: assets.ball_sheet.clone(),
                    texture_atlas: Some(TextureAtlas {
                        layout: assets.ball_layout.clone(),
                        index: 0,
                    }),
                    ..default()
                };
                let heart = ImageNode {
                    image: assets.heart_sheet.clone(),
                    texture_atlas: Some(TextureAtlas {
                        layout: assets.heart_layout.clone(),
                        index: 0,
                    }),
                    ..default()
                };
                let treat = ImageNode {
                    image: assets.treat.clone(),
                    ..default()
                };
                let rows = [
                    (ball, stats.kitty_points, "kitty points"),
                    (heart, stats.hearts, "hearts"),
                    (treat, stats.cookies, "treats"),
                ];
                for (icon, value, what) in rows {
                    col.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(24.0),
                        width: Val::Px(460.0),
                        ..default()
                    })
                    .with_children(|row| {
                        row.spawn((
                            icon,
                            Node {
                                width: Val::Px(56.0),
                                height: Val::Px(56.0),
                                ..default()
                            },
                        ));
                        row.spawn((
                            label(assets, &format!("x{value}"), 40.0, TITLE_COLOR),
                            Node {
                                width: Val::Px(120.0),
                                ..default()
                            },
                        ));
                        row.spawn((
                            label(assets, what, LABEL_SIZE, TITLE_COLOR),
                            Node {
                                width: Val::Px(220.0),
                                ..default()
                            },
                        ));
                    });
                }
                col.spawn(Node {
                    height: Val::Px(16.0),
                    ..default()
                });
                spawn_button(
                    col,
                    assets,
                    &assets.button_lilac,
                    "Main menu",
                    MenuButtonAction::MainMenu,
                );
            });
    });
}

// ---------------------------------------------------------------------------
// Interaction
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn menu_button_system(
    mut interaction_query: Query<
        (
            &Interaction,
            &MenuButtonAction,
            &mut ImageNode,
            &mut Transform,
        ),
        (Changed<Interaction>, With<Button>),
    >,
    game_state: Res<State<GameState>>,
    mut page: ResMut<MenuPage>,
    mut app_exit_events: EventWriter<AppExit>,
    mut next_game_state: ResMut<NextState<GameState>>,
    mut next_level_state: ResMut<NextState<LevelState>>,
    mut current_level: ResMut<CurrentLevel>,
    mut play_mode: ResMut<PlayMode>,
    mut stats: ResMut<PlayerStats>,
    mut settings: ResMut<AudioSettings>,
    mut sfx: EventWriter<SfxEvent>,
) {
    for (interaction, action, mut image, mut transform) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                image.color = PRESSED_TINT;
                transform.scale = Vec3::ONE;
                sfx.write(SfxEvent::ButtonClick);

                match *action {
                    MenuButtonAction::Play => {
                        stats.reset();
                        *play_mode = PlayMode::Campaign;
                        current_level.0 = Level::Level1;
                        next_game_state.set(GameState::Game);
                        next_level_state.set(LevelState::Loading);
                    }
                    MenuButtonAction::StartLevel(n) => {
                        stats.reset();
                        *play_mode = PlayMode::SingleLevel;
                        current_level.0 = Level::from_number(n);
                        next_game_state.set(GameState::Game);
                        next_level_state.set(LevelState::Loading);
                    }
                    MenuButtonAction::PlayAgain => {
                        stats.reset();
                        next_game_state.set(GameState::Game);
                        next_level_state.set(LevelState::Loading);
                    }
                    MenuButtonAction::Resume => next_game_state.set(GameState::Game),
                    MenuButtonAction::MainMenu => next_game_state.set(GameState::MainMenu),
                    MenuButtonAction::Controls => *page = MenuPage::Controls,
                    MenuButtonAction::Options => *page = MenuPage::Options,
                    MenuButtonAction::Levels => *page = MenuPage::Levels,
                    MenuButtonAction::Back => *page = home_page(game_state.get()),
                    MenuButtonAction::MusicUp => settings.adjust_music(VOLUME_STEP),
                    MenuButtonAction::MusicDown => settings.adjust_music(-VOLUME_STEP),
                    MenuButtonAction::SfxUp => settings.adjust_sfx(VOLUME_STEP),
                    MenuButtonAction::SfxDown => settings.adjust_sfx(-VOLUME_STEP),
                    MenuButtonAction::Exit => {
                        app_exit_events.write(AppExit::Success);
                    }
                }
            }
            Interaction::Hovered => {
                image.color = Color::WHITE;
                transform.scale = Vec3::splat(HOVERED_BUTTON_SCALE);
            }
            Interaction::None => {
                image.color = Color::WHITE;
                transform.scale = Vec3::ONE;
            }
        }
    }
}

/// Keyboard shortcuts on menu pages: Esc backs out of a sub-page and
/// resumes from the pause page, and Esc or Enter leaves the end screens.
/// The end screens matter because they are the one place with no way back
/// except a single button — if the pointer is not cooperating there, the
/// run is stranded.
fn menu_keyboard_system(
    keys: Res<ButtonInput<KeyCode>>,
    game_state: Res<State<GameState>>,
    mut page: ResMut<MenuPage>,
    mut next_game_state: ResMut<NextState<GameState>>,
) {
    let escape = keys.just_pressed(KeyCode::Escape);
    let enter = keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter);
    if !escape && !enter {
        return;
    }
    match (*page, game_state.get()) {
        (MenuPage::Victory | MenuPage::GameOver, _) => next_game_state.set(GameState::MainMenu),
        (MenuPage::Pause, GameState::PauseMenu) if escape => next_game_state.set(GameState::Game),
        (MenuPage::Controls | MenuPage::Options | MenuPage::Levels, state) if escape => {
            *page = home_page(state)
        }
        _ => {}
    }
}

fn update_volume_readouts(
    settings: Res<AudioSettings>,
    mut readouts: Query<(&VolumeReadout, &mut Text)>,
) {
    if !settings.is_changed() {
        return;
    }
    for (kind, mut text) in &mut readouts {
        let value = match kind {
            VolumeReadout::Music => settings.music_steps(),
            VolumeReadout::Sfx => settings.sfx_steps(),
        };
        **text = format!("{value}");
    }
}
