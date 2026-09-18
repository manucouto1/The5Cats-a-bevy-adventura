//! `levelN_lights.json` — the authored lighting for a level.
//!
//! Positions and radii are in tiles, like every other level file, so the
//! numbers line up with what Tilesetter shows. Everything has a default,
//! so a level can ship `{}` and simply be lit flat.

use bevy::prelude::*;
use serde::Deserialize;

/// Hard cap shared with the shaders — the uniform arrays are this long.
pub const MAX_LIGHTS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LightKind {
    /// Infinitely far away: the "sun". Placed far outside the viewport
    /// along `angle`, with no distance falloff.
    Sun,
    #[default]
    Point,
    /// A point light restricted to a cone around `angle`.
    Cone,
}

impl LightKind {
    /// Matches the `light_type` the shaders switch on.
    pub fn shader_id(self) -> f32 {
        match self {
            LightKind::Sun => 0.0,
            LightKind::Point => 1.0,
            LightKind::Cone => 2.0,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct LightEntry {
    #[serde(default, rename = "type")]
    pub kind: LightKind,
    /// Tile coordinates, ignored for `sun`.
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
    /// Degrees. Direction the light points for `sun` / `cone`.
    #[serde(default)]
    pub angle: f32,
    /// Half-angle of the cone, degrees.
    #[serde(default = "default_cone")]
    pub cone_angle: f32,
    #[serde(default = "default_intensity")]
    pub intensity: f32,
    /// Reach in tiles. 0 = no distance falloff (what `sun` wants).
    #[serde(default = "default_falloff")]
    pub falloff: f32,
    /// Bigger = blurrier shadow edges.
    #[serde(default = "default_softness")]
    pub softness: f32,
    /// How deep this light bleeds into solid tiles before they go dark.
    /// In px, like the original `interiorDepth`.
    #[serde(default = "default_interior")]
    pub interior_depth: f32,
    #[serde(default = "default_color")]
    pub color: [f32; 3],
}

fn default_cone() -> f32 {
    35.0
}
fn default_intensity() -> f32 {
    1.0
}
fn default_falloff() -> f32 {
    10.0
}
fn default_softness() -> f32 {
    2.0
}
fn default_interior() -> f32 {
    16.0
}
fn default_color() -> [f32; 3] {
    [1.0, 1.0, 1.0]
}

/// Global illumination settings — the Inject → Propagate(N) → Temporal →
/// Blur chain from reptile_studio.
#[derive(Debug, Clone, Deserialize)]
pub struct GiSettings {
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Ping-pong propagation steps. Each one is an off-screen pass, and the
    /// per-pass overhead dwarfs the shading of a 192x108 grid, so this is
    /// the main GI cost knob.
    #[serde(default = "default_iterations")]
    pub iterations: u32,
    /// Texels each step advances. Light reaches `iterations * step_texels`
    /// texels, so raising this spreads the bounce further without adding
    /// passes; too high and a step can jump a thin wall.
    #[serde(default = "default_step_texels")]
    pub step_texels: f32,
    /// Energy kept per propagation step; < 1 so the grid converges.
    #[serde(default = "default_decay")]
    pub energy_decay: f32,
    /// Blend the grid with last frame's to cut flicker. The grid is
    /// anchored to the viewport, so a high blend smears while the camera
    /// pans — keep it modest.
    #[serde(default = "default_true")]
    pub temporal: bool,
    #[serde(default = "default_temporal_blend")]
    pub temporal_blend: f32,
    #[serde(default = "default_true")]
    pub blur: bool,
    /// Multiplier on the indirect contribution in the final pass.
    #[serde(default = "default_gi_strength")]
    pub strength: f32,
}

fn default_true() -> bool {
    true
}
fn default_iterations() -> u32 {
    6
}
fn default_step_texels() -> f32 {
    4.0
}
fn default_decay() -> f32 {
    0.85
}
fn default_temporal_blend() -> f32 {
    0.55
}
fn default_gi_strength() -> f32 {
    1.0
}

impl Default for GiSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            iterations: default_iterations(),
            step_texels: default_step_texels(),
            energy_decay: default_decay(),
            temporal: true,
            temporal_blend: default_temporal_blend(),
            blur: true,
            strength: default_gi_strength(),
        }
    }
}

/// How the final pass shades what the lights reach.
#[derive(Debug, Clone, Deserialize)]
pub struct ShadingSettings {
    /// Light everything gets for free. 1.0 = the level looks untouched.
    #[serde(default = "default_ambient")]
    pub ambient: f32,
    /// Air thickness: tints the ambient cool and drives the in-scatter
    /// shafts and how soft the shadow edges get.
    #[serde(default)]
    pub humidity: f32,
    #[serde(default = "default_ao_strength")]
    pub ao_strength: f32,
    /// In px.
    #[serde(default = "default_ao_radius")]
    pub ao_radius: f32,
    #[serde(default = "default_haze")]
    pub haze_strength: f32,
    /// Depth in px a light ignores before tiles start darkening inward.
    #[serde(default = "default_tile_offset")]
    pub tile_offset: f32,
    #[serde(default = "default_tile_min")]
    pub tile_min_brightness: f32,
    /// linear | exp | smooth
    #[serde(default = "default_curve")]
    pub tile_curve: String,
    /// Whether lights raymarch the level for shadows at all. Off keeps the
    /// falloff and the inward tile shading but stops casting: interiors
    /// full of small ledges turn into a mess of hard-edged slivers
    /// otherwise, and a flat, soft wash reads better there.
    #[serde(default = "default_true_shading")]
    pub cast_shadows: bool,
    /// Scales every penumbra. 1.0 is the tuned default; higher is blurrier
    /// and more diffuse, lower approaches hard-edged.
    #[serde(default = "default_shadow_softness")]
    pub shadow_softness: f32,
    /// How much the painted skyline (the nearest parallax layer) shadows
    /// `sun` lights, 0..1. This is what puts bands of shade between the
    /// background buildings on a level whose foreground is a bare street.
    #[serde(default)]
    pub backdrop_shadows: f32,
}

fn default_true_shading() -> bool {
    true
}
fn default_shadow_softness() -> f32 {
    1.0
}

fn default_ambient() -> f32 {
    1.0
}
fn default_ao_strength() -> f32 {
    0.35
}
fn default_ao_radius() -> f32 {
    28.0
}
fn default_haze() -> f32 {
    0.6
}
fn default_tile_offset() -> f32 {
    2.0
}
fn default_tile_min() -> f32 {
    0.12
}
fn default_curve() -> String {
    "smooth".into()
}

impl ShadingSettings {
    pub fn curve_id(&self) -> f32 {
        match self.tile_curve.as_str() {
            "linear" => 0.0,
            "exp" => 1.0,
            _ => 2.0,
        }
    }
}

impl Default for ShadingSettings {
    fn default() -> Self {
        Self {
            ambient: default_ambient(),
            humidity: 0.0,
            ao_strength: default_ao_strength(),
            ao_radius: default_ao_radius(),
            haze_strength: default_haze(),
            tile_offset: default_tile_offset(),
            tile_min_brightness: default_tile_min(),
            tile_curve: default_curve(),
            cast_shadows: true,
            shadow_softness: default_shadow_softness(),
            backdrop_shadows: 0.0,
        }
    }
}

/// The whole file. Also the resource the runtime reads.
#[derive(Debug, Clone, Default, Deserialize, Resource)]
#[serde(default)]
pub struct LevelLighting {
    /// Set false to leave a level unlit (no passes are even spawned).
    pub enabled: Option<bool>,
    pub shading: ShadingSettings,
    pub gi: GiSettings,
    /// A light that follows Tofe. Omit for no player light.
    pub player_light: Option<LightEntry>,
    pub lights: Vec<LightEntry>,
}

impl LevelLighting {
    pub fn is_enabled(&self) -> bool {
        self.enabled.unwrap_or(true)
    }
}

/// Reads `levelN_lights.json`. A missing file means "no lighting on this
/// level", which is how levels opt out until they are authored.
pub fn load_level_lighting(path: &str) -> Option<LevelLighting> {
    let raw = std::fs::read_to_string(path).ok()?;
    match serde_json::from_str::<LevelLighting>(&raw) {
        Ok(cfg) => Some(cfg),
        Err(e) => {
            warn!("Failed to parse {path}: {e}");
            None
        }
    }
}
