use bevy::prelude::*;

/// Which screen the menu layer is currently showing. Changing this resource
/// tears down the current widgets and spawns the new page.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum MenuPage {
    #[default]
    Main,
    Controls,
    Options,
    Levels,
    Pause,
    GameOver,
    Victory,
}

#[derive(Component, Clone, Copy, Debug)]
pub enum MenuButtonAction {
    /// Start the full campaign from level 1.
    Play,
    Controls,
    Options,
    Levels,
    /// Back from Controls/Options to whichever page owns them.
    Back,
    Resume,
    /// Quit to the main menu (from pause, game over, victory).
    MainMenu,
    /// Restart the level that was being played.
    PlayAgain,
    StartLevel(u8),
    MusicUp,
    MusicDown,
    SfxUp,
    SfxDown,
    Exit,
}

/// Root marker: every menu widget lives under an entity with this so a
/// page swap can despawn the whole tree in one query.
#[derive(Component)]
pub struct MenuWidget;

/// Options screen: the numeric readouts that follow `AudioSettings`.
#[derive(Component, Clone, Copy)]
pub enum VolumeReadout {
    Music,
    Sfx,
}
