use bevy::prelude::*;

#[derive(Clone, Copy, Default, Eq, PartialEq, Debug, Hash, States)]
pub enum MenuLoadingState {
    #[default]
    Loading,
    Ready,
}

#[derive(Component)]
pub struct OnMainMenuScreen;

#[derive(Component)]
pub enum MenuButtonAction {
    Play,
    Controls,
    Options,
    Levels,
    GoToMainMenu,
    PlayAgain,
    Quit,
}

#[derive(Component, Clone)]
pub struct OriginalColor(pub BackgroundColor);

#[derive(Component)]
pub struct MenuWidget;
