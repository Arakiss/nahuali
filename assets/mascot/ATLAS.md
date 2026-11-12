# Mascot atlas format

The TUI loads the axolotl through a manifest, so new art replaces old art without code changes. Loader: `crates/nahuali-tui/src/mascot/atlas.rs`.

## Where the TUI looks

1. `$NAHUALI_MASCOT_DIR`
2. The v2 art compiled into the binary (`assets/mascot/v2/atlas.json` and every `<state>/frame-NN.png`)
3. The v1 sheet compiled into the binary (always works)

The first one that loads wins. A directory loads if it has an `atlas.json`, or, without one, at least one `<state>/frame-NN.png`. The binary never reads the checkout it was built from: new v2 art needs a rebuild, or `NAHUALI_MASCOT_DIR` while you work on it.

## `atlas.json`

```json
{
  "version": 1,
  "name": "free text",
  "sheets": { "v1": { "file": "spritesheet.png", "grid": [4, 2] } },
  "states": {
    "idle": {
      "frames": [
        { "file": "idle/frame-01.png", "ms": 2400, "jitter_ms": 3000 },
        { "file": "idle/frame-02.png", "ms": 120 },
        { "sheet": "v1", "cell": 3, "ms": 900 },
        { "sheet": "v1", "rect": [0, 0, 429, 458], "ms": 900 }
      ]
    }
  },
  "fallback": { "alarmed": "worried" }
}
```

- **States**: `idle`, `greeting`, `listening`, `thinking`, `writing`, `dreaming`, `worried`, `alarmed`, `proud`, `happy` (the `MascotState` names). Unknown names are an error.
- **Frame source**: `file` (PNG relative to the directory), or `sheet` plus `cell` (row-major index into `grid` [columns, rows]) or `rect` [x, y, w, h] in pixels.
- **Timing**: `ms` is how long the frame stays (default 140). `jitter_ms` adds a random `0..=jitter_ms` on every pass; use it on the hold frame before a blink so blinks never tick like a metronome. The renderer caps the sprite at 8 fps (4 fps on iTerm2 and under reduced motion) and keeps each state at least 1.5 s.
- **Fallback**: states without frames borrow another state's frames. Built-in defaults: alarmed → worried, greeting/proud/dreaming → happy, everything else → idle.

## Art rules the loader relies on

- PNG with a transparent background, any size, all frames of a state the same size.
- Each state is cropped to the union of its visible pixels, then every state is padded to one shared canvas, centred horizontally and standing on the bottom edge. Feet should touch the bottom of the drawing; empty margins cost nothing.
- Keep the v1 palette (pink body, coral gills, warm brown outline). The half-block fallback quantises to that palette (`mascot/halfblock.rs`), so new colours outside it degrade to the nearest ink.

## Without a manifest

`<dir>/<state>/frame-01.png`, `frame-02.png`, … in name order, 140 ms each, no jitter. Good for a first drop; add `atlas.json` to tune timing.
