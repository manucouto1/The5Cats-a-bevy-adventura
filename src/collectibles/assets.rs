use bevy::prelude::*;

#[derive(Resource)]
pub struct CollectibleAssets {
    pub treat: Handle<Image>,
    pub heart: Handle<Image>,
}
