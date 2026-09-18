# itch.io page copy

The page: <https://manucouto1.itch.io/the5cats>

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
- Source: <https://github.com/manucouto1/The5Cats-a-bevy-adventura> (GPLv3; art and music
  CC-BY-NC 4.0)
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

One script per platform, each writing into `dist/`:

| Script | Upload | itch channel | Tick |
|---|---|---|---|
| `scripts/package-macos.sh` | `5Gatos-macos.zip` | `osx` | macOS |
| `scripts/package-windows.sh` | `5Gatos-windows.zip` | `windows` | Windows |
| `scripts/package-linux.sh` | `5Gatos-linux-x86_64.tar.gz` | `linux` | Linux |

Drag them into the page's "Uploads" and tick the platform on each, or use
[butler](https://itch.io/docs/butler/):

```bash
butler push dist/5Gatos-macos.zip        manucouto1/the5cats:osx
butler push dist/5Gatos-windows.zip      manucouto1/the5cats:windows
butler push dist/5Gatos-linux-x86_64.tar.gz manucouto1/the5cats:linux
```

Worth saying on the page: neither the macOS nor the Windows build is
**signed**. macOS needs right-click → Open the first time (or
`xattr -dr com.apple.quarantine 5Gatos.app`), and Windows shows an
"unknown publisher" warning — More info → Run anyway.

There is also `scripts/package-web.sh`, which produces a browser build
(`dist/5Gatos-web.zip`, upload as HTML with "This file will be played in
the browser" ticked). It is **not ready to publish**: the game runs but the
parallax backgrounds do not draw in the browser.
