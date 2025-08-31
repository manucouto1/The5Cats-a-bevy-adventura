use bevy::prelude::*;

#[derive(Resource)]
pub struct CursorAssets {
    pub cursor_image: Handle<Image>,
    pub wool_image: Handle<Image>,
}

pub fn load_assets(mut commands: Commands, asset_server: Res<AssetServer>) {
    let cursor_handle: Handle<Image> = asset_server.load("cursor/point-cursor.png");
    let wool_handle: Handle<Image> = asset_server.load("projectiles/wool.png");
    commands.insert_resource(CursorAssets {
        cursor_image: cursor_handle,
        wool_image: wool_handle,
    });
}
