use bevy::prelude::*;

/// Every image/font the menus use. Loaded once at startup; handles stay
/// valid for the life of the app so any screen can be rebuilt instantly.
#[derive(Resource)]
pub struct MenuAssets {
    pub background: Handle<Image>,
    pub game_over_background: Handle<Image>,
    pub title_font: Handle<Font>,
    pub text_font: Handle<Font>,
    pub button_green: Handle<Image>,
    pub button_yellow: Handle<Image>,
    pub button_pink: Handle<Image>,
    pub button_lilac: Handle<Image>,
    pub button_orange: Handle<Image>,
    pub arrows: Handle<Image>,
    pub mouse: Handle<Image>,
    pub click: Handle<Image>,
    pub pause_icon: Handle<Image>,
    pub volume_up: Handle<Image>,
    pub volume_down: Handle<Image>,
    /// Rolling-ball strip (8 frames of 64x64) used as the kitty-point icon.
    pub ball_sheet: Handle<Image>,
    pub ball_layout: Handle<TextureAtlasLayout>,
    /// Heart strip (21 frames of 160x160), frame 0 = full heart.
    pub heart_sheet: Handle<Image>,
    pub heart_layout: Handle<TextureAtlasLayout>,
    pub treat: Handle<Image>,
}

pub fn load_menu_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    commands.insert_resource(MenuAssets {
        background: asset_server.load("menu/menu_background.png"),
        game_over_background: asset_server.load("menu/game_over_background.png"),
        title_font: asset_server.load("fonts/yukari.ttf"),
        text_font: asset_server.load("fonts/Purisa Bold.ttf"),
        button_green: asset_server.load("menu/button_green.png"),
        button_yellow: asset_server.load("menu/button_yellow.png"),
        button_pink: asset_server.load("menu/button_pink.png"),
        button_lilac: asset_server.load("menu/button_lilac.png"),
        button_orange: asset_server.load("menu/button_orange.png"),
        arrows: asset_server.load("menu/arrows.png"),
        mouse: asset_server.load("menu/mouse.png"),
        click: asset_server.load("menu/click.png"),
        pause_icon: asset_server.load("menu/pause.png"),
        volume_up: asset_server.load("menu/volume-up.png"),
        volume_down: asset_server.load("menu/volume-down.png"),
        ball_sheet: asset_server.load("characters/enemy/ball/enemy-ball-Sheet.png"),
        ball_layout: layouts.add(TextureAtlasLayout::from_grid(
            UVec2::splat(64),
            8,
            1,
            None,
            None,
        )),
        heart_sheet: asset_server.load("player/Corazon-Sheet.png"),
        heart_layout: layouts.add(TextureAtlasLayout::from_grid(
            UVec2::splat(160),
            21,
            1,
            None,
            None,
        )),
        treat: asset_server.load("treat.png"),
    });
}
