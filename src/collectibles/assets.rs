use bevy::prelude::*;

#[derive(Resource)]
pub struct CollectibleAssets {
    /// Rolling-ball strip (8 frames of 64x64) — the kitty point.
    pub kitty_point: Handle<Image>,
    pub kitty_point_atlas: Handle<TextureAtlasLayout>,
    /// Corazon-Sheet is a 21x1 animation strip; we render frame 0 only.
    pub extra_life: Handle<Image>,
    pub extra_life_atlas: Handle<TextureAtlasLayout>,
    pub maniac_mode: Handle<Image>,
    pub end_game: Handle<Image>,
}

pub fn load_collectible_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    commands.insert_resource(CollectibleAssets {
        kitty_point: asset_server.load("characters/enemy/ball/enemy-ball-Sheet.png"),
        kitty_point_atlas: texture_atlas_layouts.add(TextureAtlasLayout::from_grid(
            UVec2::splat(64),
            8,
            1,
            None,
            None,
        )),
        extra_life: asset_server.load("player/Corazon-Sheet.png"),
        extra_life_atlas: texture_atlas_layouts.add(TextureAtlasLayout::from_grid(
            UVec2::splat(160),
            21,
            1,
            None,
            None,
        )),
        maniac_mode: asset_server.load("treat.png"),
        end_game: asset_server.load("foil_hat.png"),
    });
}
