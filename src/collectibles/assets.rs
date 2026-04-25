use bevy::prelude::*;

#[derive(Resource)]
pub struct CollectibleAssets {
    pub kitty_point: Handle<Image>,
    pub extra_life: Handle<Image>,
    // Corazon-Sheet is a 21x1 animation strip; we render a single frame
    // via atlas so we only see one heart, not the whole strip.
    pub extra_life_atlas: Handle<TextureAtlasLayout>,
    pub maniac_mode: Handle<Image>,
}

pub fn load_collectible_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let layout = TextureAtlasLayout::from_grid(UVec2::splat(160), 21, 1, None, None);

    commands.insert_resource(CollectibleAssets {
        kitty_point: asset_server.load("treat.png"),
        extra_life: asset_server.load("player/Corazon-Sheet.png"),
        extra_life_atlas: texture_atlas_layouts.add(layout),
        maniac_mode: asset_server.load("foil_hat.png"),
    });
}
