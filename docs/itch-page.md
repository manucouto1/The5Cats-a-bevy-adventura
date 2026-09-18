# itch.io page copy

Paste-ready text for the store page, plus what to upload and how.

## Title

5Gatos

## Short description (one line, shows in listings)

A cat with a bag of wool balls against Kidd Cat, four levels and a
slow-motion fall through a shaft full of cats.

## Description

You are Tofe, a cat with a bag of wool balls. Four levels of jumping,
throwing and getting out of the way, ending in a three-part showdown with
Kidd Cat: an arena fight, a slow-motion free fall down a shaft crawling
with cats, and a last stand at the bottom.

Pick up kitty points, hearts and treats — a treat turns every shot into a
sixteen-way burst for ten seconds and Tofe puts on the foil hat.

**Controls**

- Move: A / D or the arrow keys
- Jump: W, Up or Space — press it again in mid-air to double jump
- While free-falling: W to slow down, S to dive
- Aim with the mouse, left click throws a wool ball; point-blank, Tofe
  swipes instead
- Esc pauses, F11 is fullscreen

Originally a Pygame game made with classmates for a Master's assignment in
Computer Engineering, rebuilt from scratch in Rust with the
[Bevy](https://bevyengine.org) engine.

## Credits / Built with

- Engine: [Bevy 0.16](https://bevyengine.org) — ECS, 2D renderer, UI, audio
  and asset pipeline
- Physics: [bevy_rapier2d](https://rapier.rs)
- Tilemaps: [Tilesetter](https://led.itch.io/tilesetter)
- Source: <https://github.com/…/The5Cats-a-bevy-adventura> (GPLv3; art and
  music CC-BY-NC 4.0)
- Original Pygame version made with classmates

## Metadata to set on the page

- Genre: Platformer · Tags: 2d, pixel-art, cats, bevy, rust, boss-fight
- Kind of project: Downloadable · Release status: Released
- Pricing: free (or "name your own price" — the assets are CC-BY-NC, so no
  commercial use without permission)
- Screenshots: `snapshots/level1.png`, `level2.png`, `level3.png`,
  `boss.png`, `fall.png`
- Cover image: itch wants 630x500; crop one of the screenshots

## Uploading

`scripts/package-macos.sh` builds the release and writes
`dist/5Gatos-macos.zip` (a `.app` bundle, so players double-click it).

Then either drag the zip into the page's "Uploads" and tick **macOS**, or
use [butler](https://itch.io/docs/butler/):

```bash
butler push dist/5Gatos-macos.zip <your-itch-user>/5gatos:osx
```

Two things worth saying on the page: the build is **unsigned**, so the
first launch needs right-click → Open (or
`xattr -dr com.apple.quarantine 5Gatos.app`), and it is **macOS only** for
now — Windows and Linux builds need to be cross-compiled, and a browser
build needs the level loading moved off `std::fs` first.
