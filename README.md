# Cellular Automata (Rust, cross-platform)

A native desktop sandbox for Life-like cellular automata, built with
[egui](https://github.com/emilk/egui) /
[eframe](https://github.com/emilk/egui/tree/main/crates/eframe). Runs on
Windows, macOS and Linux from the same codebase. Conway's Game of Life
(B3/S23) is one of 21 built-in rules — the whole point of the generic B/S
rule engine below is that Conway's is a single variant, not the app's
identity.

## Features

- **Generic B/S rule engine** — any Life-like rule (how many live neighbors
  are required for a dead cell to be born, and for a live cell to survive)
  is expressed as a `RuleSet { birth: [bool; 9], survive: [bool; 9] }` and
  evaluated by one generic step function. No rule is hardcoded separately.
- **Rule presets**: 21 built-in Life-like rules — Conway's Life, 2x2, 34 Life,
  Assimilation, Coagulations, Coral, Day & Night, Diamoeba, Flakes, Gnarl,
  HighLife, Long Life, Maze, Mazectric, Move, Pseudo Life,
  Replicator, Seeds, Serviettes, Stains, Walled Cities. The "Rule" dropdown
  groups them by long-term random-soup behavior — Chaotic / Explosive /
  Stable section headers, presets indented underneath — instead of one flat
  list with the class as a same-line suffix on every row. Each row
  is clickable/highlighted across its full width, not just the text. Plus 9
  Birth / 9 Survive checkboxes to build any custom rule by hand.
- **Finite 4096x4096 square plane** (Golly's universe is square too, so
  patterns and their rotations that fit there fit here): live cells are
  stored as a `HashSet<(i64, i64)>` bounded to `x, y ∈ [-2048, 2047]`
  (`simulation::WORLD_MIN/WORLD_MAX`) — no wraparound, cells simply can't
  be painted, stamped, or born past the edge (drawn as a red boundary line
  on the canvas), stepped with the standard neighbor-counting algorithm
  (`O(live cells)` per generation, so the size of the world costs nothing
  until a pattern actually grows into it).
- **Starting configurations**: a "Start" dropdown + "Load" button
  (Simulation row) clears the board and scatters a named setup across the
  *whole* world at the same density the "Random"/density slider controls
  — Random soup (individual cells), Glider field, Gosper gun field, Pulsar
  field. Nothing is pre-selected (there's no "Empty board" entry — Clear
  already does that job), so Load stays disabled until you deliberately
  pick one. Every option is density-based (a lattice of candidate spots
  across the map, each independently filled with probability = density)
  rather than a single fixed figure or a hardcoded instance count, so "how
  crowded" is always adjustable the same way for every option. Built from
  the pattern library's own cell data (`src/starts.rs`), so a fix to a
  pattern's shape automatically carries
  through to any start built from it.
- **Three-way toolbox** (top bar, always visible next to Play/Pause/Step):
  Draw / Pan / Eraser, mutually exclusive. Draw is the default
  click/drag-to-toggle-or-paint behavior. Pan makes left-click-drag move
  the map directly under the cursor — the classic Google Maps drag-to-pan
  gesture, on its own tool since the primary button is otherwise needed
  for drawing (cursor turns into a hand/grab icon while active). Eraser
  forces every click or drag stroke to remove cells regardless of their
  state, with a red outline over the cell it would remove. Selecting a
  pattern from the library switches back to Draw automatically.
  Independent of which tool is active, a middle-mouse-button drag always
  pans too (the Blender/Photoshop/Figma convention), so panning is never
  more than one button away regardless of tool.
- **Collapsible bars, without losing functionality**: a "☰" button
  (top-left, always visible) shows/hides the pattern-library overlay
  (collapsed by default), and a "⚙" button collapses the Simulation/
  custom-rule rows down to a single essentials strip (the two toggles,
  Draw/Pan/Eraser, Play/Pause, Step, Gen/Live). Both toggle back the same
  way. Neither one hides anything
  canvas navigation actually needs — zoom, pan, and the pattern library
  itself all live on/over the canvas (see below) — so collapsing both bars
  fully gives the map maximum room with nothing lost.
- **The pattern library floats over the canvas, not beside it.** It's an
  `egui::Area` overlay (top-left corner, collapsed by default) rather than
  a side `Panel`, so showing or hiding it never resizes the canvas — the
  map's aspect ratio and visible extent stay identical either way. This is
  most noticeable in a maximized/full-screen window: what you see as "the
  canvas" is always the true available area, not something that silently
  shrinks whenever the library is open. Every category is expanded by
  default, and the panel grows to show them (up to nearly the full canvas
  height) rather than staying capped short — a lateral scrollbar (pinned
  to the panel's own right edge, not hugging whichever row happens to be
  widest) only appears once the expanded content actually overflows that
  height. Its close button sits in the panel's top-right corner. Scrolling
  the list (or pinching/Ctrl-scrolling over it) only scrolls the list —
  egui resolves hover per-layer, so the map underneath doesn't also
  zoom/pan just because the pointer happens to be over the floating panel.
- **The canvas grid adapts to whatever space is actually available** —
  the window's own shape minus the top bar, nothing more — instead of
  being locked to a fixed aspect ratio, so there's no black letterbox or
  pillarbox margin above/below or beside the grid. Cells are still always
  perfect squares (`View::cell_size` is one scalar shared by both axes,
  never stretched per-axis); the axis with more canvas room than the
  world needs simply shows more of it, clamped exactly at the world's edge
  by `View::clamp_to_world` (drawn as a red boundary line), the same as
  any other axis.
- **The viewport is always fully inside the map.** Panning/zooming is
  clamped (`View::clamp_to_world`) so the visible rectangle can slide right
  up to an edge but never shows empty space beyond it — you can't scroll
  off into the void. If the viewport is ever wider/taller than the map
  itself (very zoomed out on a large window), that axis is centered on the
  map instead.
- **Minimap**: bottom-right overlay showing the whole plane, a green marker
  per occupied region, and a yellow outline for the current viewport. A
  128px square, matching the world. Click or drag inside it to jump/pan
  the camera anywhere on the plane instantly — handy since the plane is
  much bigger than what's visible at once.
- **Zoom & pan, Google Maps-style**: a physical mouse's scroll wheel zooms
  in/out anchored on the cursor, exactly like scrolling on a Google Maps
  page; a laptop trackpad's smooth two-finger scroll pans freely in any
  direction instead, like panning a map on a touchscreen. egui tags every
  scroll event with which device it came from (a mouse wheel reports
  discrete "line" steps, a trackpad reports continuous pixel deltas), so
  the app reads that tag directly rather than merging both into one
  ambiguous scroll signal — the two devices drive genuinely different,
  non-conflicting actions. Pinch gestures and Ctrl/Cmd+scroll always zoom
  regardless of device.
- **On-canvas zoom control**: a floating panel in the canvas's bottom-left
  corner (mirroring the minimap's placement in the opposite corner) with
  large, fixed-size `+`/`−` buttons, a "⟲" reset-view button, and a live
  "Npx" readout — always present regardless of whether the top bar is
  expanded or collapsed, and not tied to gesture recognition. True
  pinch-to-zoom via a laptop touchpad is a platform limitation on Linux
  (winit only wires up `PinchGesture`/`PanGesture` on macOS/iOS), so this
  on-canvas control (plus the wheel/Pan-tool/middle-drag panning above) is
  the primary way to navigate via touchpad there, not just a fallback.
- **Minimum zoom never shows empty space** — you can zoom out until the
  viewport exactly covers the plane on its tighter axis, and no further
  (`view::min_cell_size_to_cover_world`, recomputed every frame from the
  window size). On a 16:9 window over the square plane the full width is
  visible and part of the height; there is never a black margin, and the
  minimap gives the overview.
- **Speed control**: Play/Pause (▶/⏸)/Step (⏭), generations-per-second
  slider. Play/Pause, Step, Clear (🗑), Random (🎲), and the on-canvas zoom
  overlay's buttons are icon buttons with hover tooltips spelling out what
  each one does, using symbols from egui's bundled icon font rather than an
  added dependency.
- **A pattern collection per ruleset**: the library shows the collection
  for the *active* rule and nothing else — Conway's Life (32 patterns: still
  lifes, oscillators, spaceships, guns, methuselahs), HighLife (including
  its replicator), Seeds, and Day & Night. A rule with no collection says so
  instead of offering patterns that would just do something else there.
  Starting configurations that need a pattern (Glider field, ...) only
  appear when the active collection has it. Each row's full width — icon and
  name together — is a single clickable target. Every entry is simulated in
  `cargo test` under its own collection's rule and must really be the
  still life / oscillator / spaceship / gun / methuselah its category claims.
- **Golly interchange (Life-like B/S rules only)**: "Copy RLE" puts the
  board on the clipboard as Golly-format RLE (`x = .., y = .., rule = B3/S23`
  header, 70-column lines); "Import RLE" opens a `.rle` file with the native
  file picker and Ctrl+V pastes RLE copied from Golly — either way it becomes
  the pattern in hand, switching to the rule in its header. Rule text
  accepts `B3/S23`, `b3/s23` and the legacy `23/3`. Not supported, and
  rejected with a message instead of misread: B0 rules, `:T`/`:P` bounded-grid
  suffixes, `V`/`H` neighbourhoods, Generations and multi-state rules.
- **Freehand drawing**: click a single cell, or press-and-drag to paint (or
  erase, if the stroke starts on a live cell) a trail of cells.
- **Generation skipping**: a "Skip" dropdown (0/5/10/50/100/500/1000) lets
  Step (or the `S` key) advance several generations at once instead of one.
- **Live stats**: generation count, live-cell count, and cells born/died on
  the most recent step (or across a whole skip batch).
- **Show/hide grid**: checkbox in the top bar, on by default.

## Project layout

```
src/
  main.rs        - eframe bootstrap / window setup
  app.rs          - App struct (impl eframe::App), rule/RLE import-export glue
  app/canvas.rs   - canvas input (zoom, pan, tools, minimap) and drawing
  app/panels.rs   - top control bar and the pattern-library overlay
  simulation.rs   - SimState: sparse live-cell set, step/tick/randomize
  rules.rs        - RuleSet, named presets, B/S formatting
  patterns/       - Pattern/Category; one module per ruleset collection (life, highlife, seeds, daynight)
  rle.rs          - Golly-format RLE reader/writer (rule header, validation, 70-column export)
  analysis.rs     - bounded-simulation behaviour classifier (tests only; verifies the collections)
  starts.rs       - named starting configurations (Simulation row's "Start" dropdown)
  view.rs         - pan/zoom camera, cell<->screen coordinate math
```

## Controls

| Action | Input |
|---|---|
| Draw / erase a cell | Left-click (Draw tool; erases instead if the stroke starts on a live cell) |
| Freehand paint a trail | Left-click-drag (Draw tool) |
| Force-erase cells | Switch to the "Eraser" tool (top bar, always visible), then click/drag — always removes, regardless of cell state |
| Import a `.rle` file | "Import RLE" button (Simulation row) — opens a file picker, then click the canvas to place |
| Import a pattern from Golly | Copy it in Golly, press `Ctrl+V` here, click the canvas to place |
| Export the board to Golly | "Copy RLE" button (Simulation row), then paste in Golly |
| Place a pattern | Select it in the pattern library overlay (top-left), then click the canvas (switches back to the Draw tool) |
| Cancel pattern placement | Right-click, `Esc`, or the "Cancel" button in the overlay |
| Load a starting configuration | "Start" dropdown + "Load" button (Simulation row) — clears the board first |
| Zoom | Mouse scroll wheel (anchored on the cursor), pinch gesture (macOS/iOS only — see Known issues), Ctrl + scroll, `+`/`-` keys, or the `+`/`−` buttons in the on-canvas zoom overlay (bottom-left) |
| Pan by dragging the map | Switch to the "Pan" tool (top bar, always visible), then left-click-drag — like Google Maps. Or middle-click-drag with any tool active |
| Pan (other ways) | Two-finger trackpad scroll, arrow keys, or click/drag on the minimap |
| Show/hide the pattern library | "☰" button (top-left) — collapsed by default; toggling it never resizes the canvas |
| Reclaim canvas space | "⚙" collapses the Simulation/custom-rule rows |
| Jump to a distant part of the plane | Click or drag inside the minimap (bottom-right corner) |
| Reset the camera | "⟲" button in the on-canvas zoom overlay (bottom-left) |
| Play / Pause | `Space`, or the button in the top bar |
| Step (by the selected skip amount) | `S`, or the "Step" button |
| Choose how many generations Step advances | "Skip" dropdown (0, 5, 10, 50, 100, 500, 1000 — 0 behaves as 1) |
| Clear board | `C`, or the "Clear" button |
| Fill visible area randomly | `R`, or the "Random" button (density set by the adjacent slider) |
| Change generation speed | "Speed" slider (0.5–60 gen/s) |
| Switch rule preset | "Rule" dropdown (each entry tagged stable/chaotic/explosive) |
| Build a custom rule | Birth / Survive checkboxes (switches label to "Custom") |
| Show/hide the grid lines | "Show grid" checkbox (on by default) |

## Building & running

Requires a Rust toolchain (installed here via `rustup`, stable channel).

```bash
cargo build --release
./target/release/cellular_automata          # Linux/macOS
# or just:
cargo run --release
```

A desktop shortcut was created at `~/Desktop/CellularAutomata.desktop`
pointing at the release binary. Depending on your desktop environment you
may need to right-click it once and choose "Allow Launching" / "Trust"
the first time.

The code only depends on cross-platform crates (`eframe`, `rand`) with no
OS-specific APIs, so it builds equally on Windows and macOS — only Linux
has actually been built/run by hand in this environment, but
`.github/workflows/ci.yml` builds, tests, and lints on every push, and
`.github/workflows/release.yml` builds real installers for all three
desktop platforms on their native runners (see below).

## Installers

Pushing a `v*` tag (or running the release workflow manually from the
Actions tab) builds and publishes, via GitHub Releases:

| Platform | Artifacts |
|---|---|
| Linux | AppImage, `.tar.gz`, and a Flatpak bundle |
| Windows | An Inno Setup installer (`.exe`) and a portable `.zip` |
| macOS | `.dmg` and `.zip`, for Apple Silicon (Intel Macs: build from source) |

The packaging sources live under `packaging/` (icons, the Inno Setup
script, the macOS `.app`/DMG bundling script) and `flatpak/` (the Flatpak
manifest and AppStream metadata). Android is not built here — see
`docs/ROADMAP.md` for why and what it would take.

## Known issues

- **True pinch-to-zoom trackpad gestures don't reach the app on Linux.**
  This is a `winit` platform limitation, not a bug here: `winit`'s
  `PinchGesture`/`PanGesture`/`RotationGesture` events (which `egui-winit`
  does translate into zoom/pan) are only ever emitted on macOS/iOS — there's
  no code path that produces them on X11 or Wayland. Ctrl+scroll still
  zooms (egui synthesizes that itself from the scroll wheel), and the
  on-canvas zoom control plus the Pan tool/middle-drag panning work
  everywhere regardless of gesture support. Two-finger trackpad scrolling
  to pan works on Linux — it arrives as an ordinary high-resolution
  scroll-wheel event tagged `MouseWheelUnit::Point`, not a special gesture.
- See [`docs/ROADMAP.md`](docs/ROADMAP.md) for what's next, or
  [`docs/RELEASES.md`](docs/RELEASES.md) for a changelog of everything
  already shipped.
