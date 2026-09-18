# 5Gatos — Bevy edition

[![Play on itch.io](https://img.shields.io/badge/itch.io-play-fa5c5c?logo=itchdotio&logoColor=white)](https://manucouto1.itch.io/the5cats)

![Game Screenshot](snapshots/Level%20one.png)

---

## 📖 About the Project

A 2D platformer built with the [Bevy 0.16](https://bevyengine.org) game
engine and `bevy_rapier2d`. It is a Rust port of **5Gatos**, a Pygame game
made with classmates for a Master's degree assignment in Computer
Engineering — started as a personal challenge to learn Rust.

You play Tofe, a cat armed with wool balls, through three platforming
levels and a final showdown against Kidd Cat: an arena fight, a slow-motion
free fall down a shaft full of cats, and a last stand at the bottom.

---

## ▶️ Play it

Play it on itch.io: **<https://manucouto1.itch.io/the5cats>**

Packaging scripts, one per platform, all writing into `dist/`:

| Script | Output | Notes |
|---|---|---|
| `scripts/package-macos.sh` | `5Gatos-macos.zip` | Universal `.app` (Apple Silicon + Intel). Unsigned, so the first launch needs right-click → Open |
| `scripts/package-windows.sh` | `5Gatos-windows.zip` | Cross-built from macOS with `cargo-xwin` |
| `scripts/package-linux.sh` | `5Gatos-linux-x86_64.tar.gz` | Built in a container (`build-linux-docker.sh`); Bevy needs Linux headers |
| `scripts/package-web.sh` | `5Gatos-web.zip` | wasm + `index.html`, the layout itch expects for HTML games |

The store-page copy lives in [`docs/itch-page.md`](docs/itch-page.md).

---

## 🎥 Pygame Version

The original version made with Pygame:
[![Pygame Demo](https://img.youtube.com/vi/92Le6aYf9X4/hqdefault.jpg)](https://www.youtube.com/watch?v=92Le6aYf9X4&list=PLS2-ezTAFpKkTXsgmpI7NpgLZUm-lQ-qJ)

---

## 🎮 Controls

| Input | Action |
|---|---|
| `A` / `D` or `←` / `→` | Move |
| `W`, `↑` or `Space` | Jump (press again mid-air to double jump) |
| `W` / `S` while free-falling | Slow down / dive |
| Mouse | Aim; left click throws a wool ball (5 in flight, 8 in maniac mode) |
| `Esc` | Pause / back |
| `F11` | Toggle fullscreen |
| `F12` | Save a screenshot to `snapshots/` |

Pickups: rolling balls are kitty points, hearts heal half a heart, treats
give 10 s of maniac mode (every shot also fires a 16-way burst and Tofe
wears the foil hat). The tallies show up on the victory screen.

---

## 🚀 Running the Game

Install [Rust](https://www.rust-lang.org/), then:

```bash
cargo run
```

### Developer switches

Environment variables read at startup:

| Variable | Effect |
|---|---|
| `THE5CATS_LEVEL=<1-4>` | Start the campaign on that level |
| `THE5CATS_AUTOPLAY=1` | Skip the main menu |
| `THE5CATS_SHOT_AFTER=<secs>` | Take a screenshot after N seconds and quit (`THE5CATS_SHOT_PATH` sets the file) |
| `THE5CATS_SCRIPT="…"` | Scripted input for headless checks — see below |
| `THE5CATS_LIGHTING=1` | Build the (off by default) lighting pipeline; F10 toggles it from there |

`THE5CATS_SCRIPT` runs a `;`-separated list of steps, which is how the
levels get checked without a human at the keyboard:

| Step | Effect |
|---|---|
| `wait <secs>` / `hold <Key> <secs>` / `tap <Key>` | Idle, hold a key, or press one for a frame |
| `mouse <x> <y>` / `click` | Place the pointer at those logical window coords and click there (this is what drives menu buttons) |
| `press <n>` | Force the nth menu button's `Interaction`, for cases with no pointer |
| `tp <tile_x> <tile_y>` | Teleport the hero, in tile coordinates |
| `shot <name>` | Save `snapshots/<name>.png` |
| `where` | Log hero, camera, level mode, wool balls in flight, nearby enemies and collectibles |
| `god` / `kill` / `goal` / `hat` | Invincibility, die, finish the level, drop the end-game hat on the hero |
| `boss <phase>` / `bosshit` / `killall` | Drive the boss fight |
| `quit` | Exit |
| `RAPIER_DEBUG=1` | Draw physics colliders |

### Level data

Each `assets/levels/levelN/` folder holds the tilemap (`levelN.json`, layer
`path` = `ground` / `damage` / `falling` / `bouncy` / `pipe_left` /
`pipe_right` / `end_level` / `decoration`), the hero and enemy placements
(`*_active_object.json`), level events (`*_events.json`), camera / mode
zones (`*_gaps.json`: `Camera`, `CameraVerticalGap` or `FallingCamera`, plus
the bands or action rectangles that class needs) and the lighting
(`*_lights.json`).

### Lighting (off by default)

The levels are hand-painted with their light already in the art, and a
physically-shaded pass on top fights that rather than adding to it, so the
game ships unlit — nothing below runs unless you ask for it with
`THE5CATS_LIGHTING=1`.

What is there, if you want to pick it up again: a port of the multi-pass 2D
lighting from Reptile Studio. The occluder mask and its signed-distance
field are rasterized from the tilemap at load, then every frame runs
Inject → Propagate(N) → Temporal → Blur over a small directional light
grid, and a half-resolution shading pass builds a lightmap the composite
multiplies over the scene: raymarched soft shadows, ambient occlusion,
in-scattered haze and light decaying inward through solid tiles. It costs
about 1 ms per frame at 2560x1440 on an M4 Pro. `src/lighting/`,
`assets/shaders/lighting_*.wgsl` and the per-level `levelN_lights.json`
(ambient, GI settings, and `sun` / `point` / `cone` sources in tile
coordinates) are all it touches.

---

## 🙏 Built with

* [**Bevy 0.16**](https://bevyengine.org) — the engine this port is written
  against: ECS, 2D renderer, UI, audio and asset pipeline. Everything in
  `src/` is Bevy systems and components, and the custom shaders under
  `assets/shaders/` are `Material2d` implementations.
* [**bevy_rapier2d**](https://rapier.rs) — colliders, the kinematic character
  controller and collision events.
* [**Tilesetter**](https://led.itch.io/tilesetter) — the tilemaps in
  `assets/levels/`.
* The classmates who built the original Pygame **5Gatos** with me.

---

## 📝 License

* Source code: [GPLv3](LICENSE)
* Assets (art, music, etc.): [CC-BY-NC 4.0](LICENSE)

Commercial use is not allowed without explicit permission.
