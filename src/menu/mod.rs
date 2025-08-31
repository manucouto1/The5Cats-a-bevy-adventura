mod assets;
mod components;

use crate::{
    game_state::{GameState, LevelState},
    menu::{
        assets::{MenuAssets, load_menu_assets},
        components::{MenuButtonAction, MenuLoadingState, MenuWidget, OriginalColor},
    },
};
use bevy::prelude::*;

const HOVERED_BUTTON_SCALE: f32 = 1.1; // La escala que aplicaremos al botón en hover
const PRESSED_BUTTON_COLOR: Color = Color::srgb(0.5, 0.5, 0.5);

const TITLE_COLOR: Color = Color::srgb(0.9, 0.9, 0.9);
const TEXT_COLOR: Color = Color::srgb(0.1, 0.1, 0.1);

// Definimos nuevos colores específicos para cada botón
const PLAY_BUTTON_COLOR: Color = Color::srgb(0.4, 1.0, 0.0); // Verde brillante
const CONTROLS_BUTTON_COLOR: Color = Color::srgb(1.0, 1.0, 0.0); // Amarillo brillante
const OPTIONS_BUTTON_COLOR: Color = Color::srgb(1.0, 0.0, 0.6); // Rosa brillante
const LEVELS_BUTTON_COLOR: Color = Color::srgb(0.4, 1.0, 0.0); // Verde brillante
const QUIT_BUTTON_COLOR: Color = Color::srgb(0.6, 0.2, 0.6); // Morado brillante

// Un componente marcador para identificar las entidades del menú

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<MenuLoadingState>()
            .init_resource::<MenuAssets>()
            .add_systems(OnEnter(GameState::MainMenu), load_menu_assets)
            .add_systems(OnEnter(GameState::PauseMenu), load_menu_assets)
            .add_systems(OnEnter(GameState::GameOver), load_menu_assets)
            .add_systems(
                Update,
                check_menu_assets_loaded
                    .run_if(in_state(MenuLoadingState::Loading))
                    .run_if(
                        in_state(GameState::MainMenu)
                            .or(in_state(GameState::PauseMenu))
                            .or(in_state(GameState::GameOver)),
                    ),
            )
            .add_systems(
                OnEnter(MenuLoadingState::Ready),
                spawn_main_menu_setup.run_if(in_state(GameState::MainMenu)),
            )
            .add_systems(
                OnEnter(MenuLoadingState::Ready),
                spawn_pause_menu_setup.run_if(in_state(GameState::PauseMenu)),
            )
            .add_systems(
                OnEnter(MenuLoadingState::Ready),
                spawn_gameover_menu_setup.run_if(in_state(GameState::GameOver)),
            )
            .add_systems(
                Update,
                menu_button_system
                    .run_if(in_state(MenuLoadingState::Ready))
                    .run_if(
                        in_state(GameState::MainMenu)
                            .or(in_state(GameState::PauseMenu).or(in_state(GameState::GameOver))),
                    ),
            )
            .add_systems(OnExit(GameState::MainMenu), despawn_menu)
            .add_systems(OnExit(GameState::PauseMenu), despawn_menu)
            .add_systems(OnExit(GameState::GameOver), despawn_menu);
    }
}

fn menu_button_system(
    mut interaction_query: Query<
        (
            &Interaction,
            &MenuButtonAction,
            &mut BackgroundColor,
            &OriginalColor,
            &mut Transform, // Ahora también consultamos el Transform del botón
        ),
        (Changed<Interaction>, With<Button>),
    >,
    mut app_exit_events: EventWriter<AppExit>,
    mut next_game_state: ResMut<NextState<GameState>>,
    mut next_level_state: ResMut<NextState<LevelState>>,
) {
    for (interaction, menu_button_action, mut background_color, original_color, mut transform) in
        &mut interaction_query
    {
        match *interaction {
            Interaction::Pressed => {
                // Al presionar, cambiamos el color y reseteamos la escala a la original
                *background_color = PRESSED_BUTTON_COLOR.into();
                transform.scale = Vec3::ONE;

                // Llama a la acción correspondiente del botón
                match menu_button_action {
                    MenuButtonAction::Play => {
                        next_game_state.set(GameState::Game);
                        next_level_state.set(LevelState::Loading);
                    }
                    MenuButtonAction::PlayAgain => {
                        next_game_state.set(GameState::Game);
                        next_level_state.set(LevelState::Loading);
                    }
                    MenuButtonAction::GoToMainMenu => next_game_state.set(GameState::MainMenu),
                    MenuButtonAction::Controls => info!("Controls button pressed!"),
                    MenuButtonAction::Options => info!("Options button pressed!"),
                    MenuButtonAction::Levels => info!("Levels button pressed!"),
                    MenuButtonAction::Quit => {
                        app_exit_events.write(AppExit::Success);
                    }
                }
                print!("Pressed button!");
            }
            Interaction::Hovered => {
                // Al hacer hover, restauramos el color original y aplicamos la transformación de escala
                *background_color = original_color.0;
                transform.scale = Vec3::splat(HOVERED_BUTTON_SCALE);
                println!("Hovered button!");
            }
            Interaction::None => {
                // Sin interacción, restauramos el color y la escala a sus valores originales
                *background_color = original_color.0;
                transform.scale = Vec3::ONE;
                println!("None button!");
            }
        }
    }
}

fn check_menu_assets_loaded(
    asset_server: Res<AssetServer>,
    menu_assets: Res<MenuAssets>,
    mut next_menu_state: ResMut<NextState<MenuLoadingState>>,
) {
    println!("Checking...");
    if asset_server.is_loaded_with_dependencies(&menu_assets.background)
        && asset_server.is_loaded_with_dependencies(&menu_assets.text_font)
        && asset_server.is_loaded_with_dependencies(&menu_assets.title_font)
    {
        println!("Menu assets loaded!");
        next_menu_state.set(MenuLoadingState::Ready);
    }
}

fn spawn_main_menu_setup(mut commands: Commands, menu_assets: Res<MenuAssets>) {
    let button_node = Node {
        width: Val::Px(200.0),
        height: Val::Px(65.0),
        margin: UiRect::all(Val::Px(20.0)),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        border: UiRect::all(Val::Px(4.0)),
        ..default()
    };

    let button_text_font = TextFont {
        font_size: 33.0,
        font: menu_assets.text_font.clone(),
        ..default()
    };
    commands.spawn((
        MenuWidget,
        ImageNode {
            image: menu_assets.background.clone(),
            ..default()
        },
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        children![(
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                ..default()
            },
            children![
                (
                    Text::new("The5Cats"),
                    TextFont {
                        font_size: 87.0,
                        font: menu_assets.title_font.clone(),
                        ..default()
                    },
                    TextColor(TITLE_COLOR),
                    Node {
                        margin: UiRect::all(Val::Px(50.0)),
                        ..default()
                    },
                ),
                (
                    Button,
                    button_node.clone(),
                    BackgroundColor(PLAY_BUTTON_COLOR),
                    OriginalColor(BackgroundColor(PLAY_BUTTON_COLOR)),
                    BorderColor::from(Color::BLACK),
                    MenuButtonAction::Play,
                    children![(
                        Text::new("Play"),
                        button_text_font.clone(),
                        TextColor(TEXT_COLOR),
                    ),]
                ),
                (
                    Button,
                    button_node.clone(),
                    BackgroundColor(CONTROLS_BUTTON_COLOR),
                    OriginalColor(BackgroundColor(CONTROLS_BUTTON_COLOR)),
                    BorderColor::from(Color::BLACK),
                    MenuButtonAction::Controls,
                    children![(
                        Text::new("Controls"),
                        button_text_font.clone(),
                        TextColor(TEXT_COLOR),
                    ),]
                ),
                (
                    Button,
                    button_node.clone(),
                    BackgroundColor(OPTIONS_BUTTON_COLOR),
                    OriginalColor(BackgroundColor(OPTIONS_BUTTON_COLOR)),
                    BorderColor::from(Color::BLACK),
                    MenuButtonAction::Options,
                    children![(
                        Text::new("Options"),
                        button_text_font.clone(),
                        TextColor(TEXT_COLOR),
                    ),]
                ),
                (
                    Button,
                    button_node.clone(),
                    BackgroundColor(LEVELS_BUTTON_COLOR),
                    OriginalColor(BackgroundColor(LEVELS_BUTTON_COLOR)),
                    BorderColor::from(Color::BLACK),
                    MenuButtonAction::Levels,
                    children![(
                        Text::new("Levels"),
                        button_text_font.clone(),
                        TextColor(TEXT_COLOR),
                    ),]
                ),
                (
                    Button,
                    button_node,
                    BackgroundColor(QUIT_BUTTON_COLOR),
                    OriginalColor(BackgroundColor(QUIT_BUTTON_COLOR)),
                    BorderColor::from(Color::BLACK),
                    MenuButtonAction::Quit,
                    children![(Text::new("Quit"), button_text_font, TextColor(TEXT_COLOR),),]
                ),
            ]
        )],
    ));
}
fn spawn_pause_menu_setup(mut commands: Commands, menu_assets: Res<MenuAssets>) {
    let button_node = Node {
        width: Val::Px(200.0),
        height: Val::Px(65.0),
        margin: UiRect::all(Val::Px(20.0)),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        border: UiRect::all(Val::Px(4.0)),
        ..default()
    };

    let button_text_font = TextFont {
        font_size: 33.0,
        font: menu_assets.text_font.clone(),
        ..default()
    };
    commands.spawn((
        MenuWidget,
        ImageNode {
            image: menu_assets.background.clone(),
            ..default()
        },
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        children![(
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                ..default()
            },
            children![
                (
                    Text::new("Pause"),
                    TextFont {
                        font_size: 87.0,
                        font: menu_assets.title_font.clone(),
                        ..default()
                    },
                    TextColor(TITLE_COLOR),
                    Node {
                        margin: UiRect::all(Val::Px(50.0)),
                        ..default()
                    },
                ),
                (
                    Button,
                    button_node.clone(),
                    BackgroundColor(PLAY_BUTTON_COLOR),
                    OriginalColor(BackgroundColor(PLAY_BUTTON_COLOR)),
                    BorderColor::from(Color::BLACK),
                    MenuButtonAction::Play,
                    children![(
                        Text::new("Play"),
                        button_text_font.clone(),
                        TextColor(TEXT_COLOR),
                    ),]
                ),
                (
                    Button,
                    button_node.clone(),
                    BackgroundColor(CONTROLS_BUTTON_COLOR),
                    OriginalColor(BackgroundColor(CONTROLS_BUTTON_COLOR)),
                    BorderColor::from(Color::BLACK),
                    MenuButtonAction::Controls,
                    children![(
                        Text::new("Controls"),
                        button_text_font.clone(),
                        TextColor(TEXT_COLOR),
                    ),]
                ),
                (
                    Button,
                    button_node.clone(),
                    BackgroundColor(OPTIONS_BUTTON_COLOR),
                    OriginalColor(BackgroundColor(OPTIONS_BUTTON_COLOR)),
                    BorderColor::from(Color::BLACK),
                    MenuButtonAction::Options,
                    children![(
                        Text::new("Options"),
                        button_text_font.clone(),
                        TextColor(TEXT_COLOR),
                    ),]
                ),
                (
                    Button,
                    button_node.clone(),
                    BackgroundColor(LEVELS_BUTTON_COLOR),
                    OriginalColor(BackgroundColor(LEVELS_BUTTON_COLOR)),
                    BorderColor::from(Color::BLACK),
                    MenuButtonAction::Levels,
                    children![(
                        Text::new("Levels"),
                        button_text_font.clone(),
                        TextColor(TEXT_COLOR),
                    ),]
                ),
                (
                    Button,
                    button_node,
                    BackgroundColor(QUIT_BUTTON_COLOR),
                    OriginalColor(BackgroundColor(QUIT_BUTTON_COLOR)),
                    BorderColor::from(Color::BLACK),
                    MenuButtonAction::Quit,
                    children![(Text::new("Quit"), button_text_font, TextColor(TEXT_COLOR),),]
                ),
            ]
        )],
    ));
}
fn spawn_gameover_menu_setup(mut commands: Commands, menu_assets: Res<MenuAssets>) {
    let button_node = Node {
        width: Val::Px(220.0),
        height: Val::Px(65.0),
        margin: UiRect::all(Val::Px(20.0)),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        border: UiRect::all(Val::Px(4.0)),
        ..default()
    };

    let button_text_font = TextFont {
        font_size: 33.0,
        font: menu_assets.text_font.clone(),
        ..default()
    };
    commands.spawn((
        MenuWidget,
        ImageNode {
            image: menu_assets.background.clone(),
            ..default()
        },
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        children![(
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                ..default()
            },
            children![
                (
                    Text::new("Game Over"),
                    TextFont {
                        font_size: 87.0,
                        font: menu_assets.title_font.clone(),
                        ..default()
                    },
                    TextColor(TITLE_COLOR),
                    Node {
                        margin: UiRect::all(Val::Px(50.0)),
                        ..default()
                    },
                ),
                (
                    Button,
                    button_node.clone(),
                    BackgroundColor(PLAY_BUTTON_COLOR),
                    OriginalColor(BackgroundColor(PLAY_BUTTON_COLOR)),
                    BorderColor::from(Color::BLACK),
                    MenuButtonAction::PlayAgain,
                    children![(
                        Text::new("Play Again"),
                        button_text_font.clone(),
                        TextColor(TEXT_COLOR),
                    ),]
                ),
                (
                    Button,
                    button_node,
                    BackgroundColor(QUIT_BUTTON_COLOR),
                    OriginalColor(BackgroundColor(QUIT_BUTTON_COLOR)),
                    BorderColor::from(Color::BLACK),
                    MenuButtonAction::GoToMainMenu,
                    children![(Text::new("Back"), button_text_font, TextColor(TEXT_COLOR),),]
                ),
            ]
        )],
    ));
}
// Sistema que elimina todas las entidades del menú al salir del estado MainMenu
fn despawn_menu(mut commands: Commands, menu_query: Query<Entity, With<MenuWidget>>) {
    for entity in menu_query.iter() {
        commands.entity(entity).despawn();
    }
}
