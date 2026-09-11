# Roadmap

## Status as of 2026-09-06

The app is functional end to end: rule engine (presets + custom B/S),
zoom/pan, speed control, and a categorized pattern library that stamps
patterns onto an infinite sparse grid. Built clean in release mode
(`cargo build --release`, binary at `target/release/conway_life`). A
desktop shortcut exists at `~/Desktop/GameOfLife.desktop`.

Interaction model went through a few rounds of live feedback tonight and
landed here:
- Left-click = toggle one cell (or stamp the selected pattern).
- Left-click-drag = freehand paint/erase a trail (Bresenham-interpolated so
  fast drags don't leave gaps).
- Pinch / Ctrl+scroll = zoom, anchored on the gesture/cursor.
- Two-finger trackpad scroll = pan.

**Reported but not yet investigated**: the app felt like it "corrupted"
after painting for a while. Not yet reproduced/diagnosed carefully — see
"Next up" below for the leading hypothesis and how to confirm it.

## Update — 2026-09-06 (later)

Added touchpad zoom and keyboard shortcuts:
- Keyboard shortcuts: `Esc` cancels pattern placement, `Space` play/pause,
  `S` step, `C` clear, `R` random fill, `+`/`-` zoom in/out (centered).

## Update — 2026-09-07

Reworked touchpad gestures after feedback that scroll-to-zoom felt wrong —
using vertical scroll for zoom fought with wanting to pan up/down, unlike
mobile touch behavior. Now:
- Two-finger trackpad drag always pans, in whichever direction you move your
  fingers (`smooth_scroll_delta` fed straight into `View::pan`), matching
  how panning works on a phone/tablet.
- Zooming is only ever a pinch gesture or Ctrl+scroll (`zoom_delta`),
  anchored on the pinch center or cursor. This is a distinct egui input
  channel from trackpad scrolling, so pan and zoom can't fight each other or
  trigger accidentally from the wrong axis.

## Update — 2026-09-07 (later)

Fixed the painting slowdown from item 1 below: `SimState` now keeps a
spatial index (`chunks: HashMap<(i64,i64), HashSet<Cell>>`, 32x32-cell
buckets) alongside `live`, updated incrementally on every insert/remove
(`toggle_cell`, `set_cell`, `stamp`, `randomize`) and rebuilt once per
generation in `step`. `central_canvas`'s render loop now calls
`sim.cells_in_bounds(min, max)`, which only visits chunks overlapping the
viewport, instead of scanning the entire `live` set every frame. Rendering
is now `O(visible cells + overlapping chunks)` rather than `O(live cells)`,
so a long paint stroke or a large live set no longer costs more per frame
than what's actually on screen. Not independently re-verified by hand under
heavy load (per the "no testing tonight" instruction from the prior
session) — worth confirming next time the app is run for a while.

## Update — 2026-09-07 (touchpad-only + rule pack + stats)

Addressed feedback that the user has no mouse, only a touchpad, plus a
larger feature batch:

- **Rule presets expanded from 7 to 22.** Added Diamoeba, Flakes, Gnarl,
  HighLife, Inverse Life, Long Life, Maze, Mazectric, Move, Pseudo Life,
  Replicator, Seeds, Serviettes, Stains, Walled Cities — B/S strings
  verified against LifeWiki/Wikipedia via web search rather than typed from
  memory (20 rules is too many to risk misremembering). Every preset
  (including the original 7) now carries a `class: "stable" | "chaotic" |
  "explosive"` tag reflecting its documented long-term random-soup behavior,
  shown in the rule dropdown as e.g. "Diamoeba (chaotic)".
- **Generation skipping.** New "Skip" dropdown (0/5/10/50/100/500/1000,
  default 0). Step (button or `S` key) now calls `SimState::step_n`, which
  runs that many generations in one call and reports total births/deaths
  across the whole batch — useful for fast-forwarding to see a rule's
  long-term behavior without waiting through, or rendering, every
  intermediate generation.
- **Births/Deaths counters.** `SimState::step` now diffs the live set
  before/after each generation and stores `last_births`/`last_deaths`;
  shown in the top bar next to Gen/Live.
- **Show/hide grid.** Checkbox in the top bar, on by default; grid lines in
  `central_canvas` are now gated on it (still also requires `cell_size >
  4.0` as before, so it doesn't force grid lines back on when zoomed out
  past visibility).
- Two-finger trackpad panning (added in the update above) already covers
  the "no mouse" gesture requirement — reconfirmed as the intended way to
  navigate, no mouse-only interaction was reintroduced.

Not independently re-verified live (per the standing "don't test, it's
slow in this environment" guidance) — worth a hands-on pass next session,
especially confirming pinch-zoom and two-finger pan actually arrive as
`zoom_delta`/`smooth_scroll_delta` events from this specific touchpad/driver
combination, since gesture routing varies by OS and windowing backend.

## Update — 2026-09-07 (gesture fallback + pattern fixes + regression test)

The user confirmed the touchpad gestures from the update above don't
register on their hardware ("right now in touchbar does not work" — this
resolves the "worth confirming" note above: it doesn't work, at least not
on this machine). Rather than continue debugging gesture routing blind (no
way to test interactively in this environment), added on-screen controls
that work regardless of gesture support:
- Zoom: `-` button, a slider bound to `cell_size`, `+` button — all anchored
  on the canvas center via a new `App::canvas_size` field (captured each
  frame in `central_canvas`, one frame stale when used from `top_panel`,
  which doesn't matter visually).
- Pan: `<`/`^`/`v`/`>` buttons, plus arrow keys as a keyboard equivalent
  (`Key::ArrowUp/Down/Left/Right` in the existing keyboard-shortcut match
  arm), both driving `View::pan` with a fixed `PAN_STEP`.
- Gesture-based zoom/pan are unchanged and still active alongside these —
  this is a fallback, not a replacement.

Also investigated "Gosper Glider Gun does not show its form": the gun's
RLE was actually correct (independently verified, bounding box and
cell-by-cell layout match the canonical LifeWiki pattern), but the small
preview icon in the pattern-library side panel had a real bug —
`paint_pattern_preview`'s scale calculation used `.max(1.0)`, which forced
at least 1px per cell and prevented the preview from ever *shrinking* a
pattern to fit the tiny 36x36 icon box. For the 36-cell-wide gun this meant
the preview overflowed the box almost entirely, showing what looked like a
meaningless blob instead of the gun's shape — even though stamping it onto
the actual canvas placed the correct pattern all along. Fixed by clamping
to `[0.3, 6.0]` instead of flooring at `1.0`.

**Pattern library grew from 21 to 28** (Tub, Ship, Pond, Clock, Queen Bee
Shuttle, Loafer, Simkin Glider Gun, B-heptomino added), with every new RLE
string pulled from conwaylife.com's mirror at copy.sh (LifeWiki itself
returns 403 to fetches) rather than typed from memory. Also added a
`#[cfg(test)] known_population_counts` test in `patterns.rs` that checks
every single library pattern's parsed cell count against its documented
LifeWiki population — cheap, high-signal regression coverage for a format
(RLE) where a single wrong digit silently produces a different, wrong
shape.

That test immediately caught **three real pre-existing bugs**, all
predating this session (shipped in the very first build):
1. **"Boat" was defined with the Ship's 6-cell shape**, not its own 5-cell
   shape — an exact duplicate of the (now separately added) Ship pattern.
2. **Pulsar had two extra spurious rows** (`5bo3bo5b`, appearing twice)
   adding 4 phantom cells not part of the real 48-cell pattern — these
   would have shown as two extra floating dots and likely broken or altered
   the oscillation.
3. **Middleweight and Heavyweight Spaceship both had malformed tails** — the
   final two rows encoded a 4-wide (resp. 5-wide) solid block one column
   short of and shifted from the correct 5-wide (resp. 6-wide) tail, i.e. a
   different, likely non-spaceship shape rather than the real MWSS/HWSS.

All three are fixed now, sourced from copy.sh's canonical RLEs. Two of my
own *expected* population figures in the new test were also wrong on the
first pass (Beacon: guessed 6, actually 8; Pentadecathlon: guessed 10,
actually 12) — corrected against the same canonical source rather than
trusting memory either way.

## Update — 2026-09-07 (bounded plane, minimap, gesture root-cause, UX pass)

The user reported two-finger pinch/pan *still* not working on their laptop
touchpad even after the earlier gesture rework, plus asked for the plane to
have defined limits, a minimap, and general UI/UX polish.

**Root-caused the gesture issue** by reading `winit`'s source directly
(`~/.cargo/registry/.../winit-0.30.13/src/event.rs` and
`platform_impl/macos/view.rs`): `WindowEvent::PinchGesture`, `PanGesture`,
`RotationGesture`, and `DoubleTapGesture` are **only ever emitted on macOS
and iOS** — there is no X11 or Wayland code path that produces them at all,
in this winit version. `egui-winit` does correctly translate them into
egui's zoom/pan events (confirmed in `egui-winit-0.36.1/src/lib.rs`), so
this was never a bug in our code or in egui — genuine pinch-to-zoom via a
touchpad simply cannot reach a `winit`-based app on Linux today. Ctrl+scroll
zoom still works (egui synthesizes that itself, independent of the OS
gesture layer). Two-finger-scroll-to-pan is a different, ordinary
mechanism (`WindowEvent::MouseWheel`) that Linux *does* support, so it's
less clear why the user says that also doesn't work — added a "Show input
debug" checkbox that overlays live `zoom_delta`/`scroll_delta`/touch-count
values on the canvas so the next report can include actual numbers instead
of "doesn't work", which should make this diagnosable for real instead of
guessed at again.

Given the platform limitation, on-screen zoom/pan controls (added last
round) are now the *primary* navigation method on Linux touchpads, not a
fallback — a "Reset view" button was added alongside them.

**Bounded the plane.** `simulation::WORLD_MIN`/`WORLD_MAX` fix the world to
`[-512, 511]` on each axis (1024x1024 cells). `insert_cell` — the single
choke point all of `toggle_cell`/`set_cell`/`stamp`/`randomize` already
funneled through — now silently drops anything outside those bounds, and
`next_generation` filters birth candidates the same way, so the boundary
acts like a wall (no wraparound). The canvas draws a red rectangle at the
world edge whenever it's on-screen. This also caps memory/CPU cost under a
fully-saturated explosive rule, which is a nice side benefit given the
original "corruption" report.

**Added a minimap** in the canvas's bottom-right corner: the whole plane,
a green marker per *occupied spatial-index chunk* (reusing the existing
`chunks` index from the earlier perf fix — `O(occupied chunks)`, not
`O(live cells)`, so it stays cheap even on a busy board), and a yellow
outline for the current viewport. Click or drag inside it to recenter the
camera anywhere on the plane instantly (`View::center_on`) — this alone
should help a lot with the "how do I get back to where I was" problem that
comes with a touchpad-unfriendly pan story.

**UX pass**: hover tooltips on most buttons/sliders (zoom, pan, Step,
Clear, Random showing its actual density, Births/Deaths explaining what
they count), a live zoom-level readout ("16px/cell") next to the zoom
slider, and the Reset View button mentioned above. `cargo clippy` run
clean (no warnings) after all of this.

Not independently tested live — same standing limitation as before (no way
to generate real touchpad hardware events in this environment). The
input-debug overlay is specifically meant to make the *next* round of
feedback actionable without needing that.

## Update — 2026-09-07 (16:9 world + hard viewport clamp)

Two follow-ups on the plane/minimap work above:

- **World resized to 1920x1080** (`x ∈ [-960, 959]`, `y ∈ [-540, 539]`) to
  match a 16:9 "Full HD" aspect ratio instead of the previous 1024x1024
  square, per explicit request.
- **The viewport can no longer show anything outside the map.** Added
  `View::clamp_to_world`, called once per frame in `central_canvas` after
  every pan/zoom input for that frame (mouse gestures, keyboard, the
  on-screen zoom/pan controls, minimap click/drag) has been applied. It
  clamps `offset` per-axis so the visible rectangle can slide right up to
  an edge but never past it; if the viewport is wider or taller than the
  map itself (e.g. zoomed far out on a large window), that axis is
  centered on the map instead of clamped to a corner, since no in-bounds
  offset would fill the screen anyway. Covered by 3 new unit tests in
  `view.rs` (`clamp_pulls_a_far_away_offset_back_inside_the_world`,
  `clamp_leaves_an_already_inside_offset_untouched`,
  `clamp_centers_an_axis_when_the_viewport_is_wider_than_the_world`) —
  `cargo test` now has 4 tests total, `cargo clippy` still clean.

## Update — 2026-09-07 (smaller map, starting configurations, eraser, 3D-readiness)

A batch of follow-up requests after the 16:9 hard-clamp work above:

- **World shrunk to 480x270** (`x ∈ [-240, 239]`, `y ∈ [-135, 134]`) — a
  quarter-scale version of the previous 1920x1080, still 16:9. Keeps the
  minimap and Random/Start fills dense and readable at a glance instead of
  mostly empty space, while still comfortably fitting every library pattern.
  `View::clamp_to_world`'s tests read `WORLD_MIN`/`WORLD_MAX` directly so
  they needed no changes.
- **Starting configurations** (`src/starts.rs`, new module): a "Start"
  dropdown + "Load" button in a new "Board" row clears the board and lays
  out a named setup centered on the world — Empty board, Random soup,
  Single glider, Gosper glider gun, Acorn, R-pentomino, Diehard, Glider
  symphony (4 gliders), Pulsar field (3x3). Built by looking up cells from
  the existing pattern library and stamping them centered via a small
  bounding-box-midpoint helper, rather than duplicating RLE strings in a
  second place — a future fix to a pattern's shape (like the Boat/Pulsar/
  MWSS/HWSS bugs found earlier) automatically carries through to anything
  built from it.
- **Pattern library grew from 28 to 35**: Barge, Long Boat (still lifes),
  Figure Eight, Kok's Galaxy (oscillators), Copperhead (spaceship),
  Pi-heptomino, Rabbits (methuselahs). Every RLE pulled from copy.sh's
  mirror (LifeWiki itself 403s direct fetches) and hand-verified by parsing
  cell counts against documented populations before adding to the
  regression test — Long Boat (7), Barge (6), Figure Eight (12), Copperhead
  (28), and Pi-heptomino (7)/Rabbits (9) all matched known LifeWiki figures
  exactly, which is a good independent confirmation the RLEs were copied
  correctly.
- **Eraser tool**: a Draw/Eraser segmented toggle in the Board row.
  Previously erasing only ever happened implicitly (a paint stroke started
  on a live cell erased instead of drew) with no way to force-remove cells
  under a stamped pattern's overlap. Eraser mode makes every click/drag
  remove cells outright, shows a red outline over the cell it's about to
  remove, and is mutually exclusive with pattern placement (picking either
  one turns off the other).
- **Random-fill density is no longer hardcoded** — a slider next to
  Clear/Random controls it directly (this closes out item 1 from the old
  "Next up" list below).
- **UI reorganized** into labeled "Simulation" / "Board" / "View" row
  groups (with horizontal separators between them) instead of one dense
  wall of controls, now that there are Start/Eraser/density controls to fit
  in alongside everything else.
- **3D-migration readiness pass**: no functional change, but light
  refactoring + doc comments at the seams a future 3D version would need to
  cut along — see "3D migration path" below for what's actually involved.

`cargo test` (4 tests, all still passing after the new pattern-count
entries), `cargo clippy --all-targets` clean.

## 3D migration path (not scheduled — notes for whenever it's picked up)

The codebase was nudged (not rewritten) to make a future 3D version less of
a from-scratch rebuild:

- **`simulation.rs`** is dimension-agnostic in spirit already: `Cell` is a
  type alias (currently `(i64, i64)`), and the neighbor-counting loop was
  pulled out into a named `NEIGHBOR_OFFSETS: [(i64, i64); 8]` constant
  specifically so a 3D build can swap in the 26-cell 3D Moore neighborhood
  (`dx/dy/dz in -1..=1`, minus the origin) in one place. `RuleSet` is
  already generic over "how many neighbors" as a 0-8 bool array — 3D would
  widen that to 0-26, no structural change needed.
- **`app.rs`/`view.rs` are the 2D-specific half** — a `Painter`-based
  renderer and an orthographic 2D camera (`View { offset, cell_size }`). A
  3D build would replace these two wholesale with a `wgpu`/`three-d`-based
  instanced-cube renderer and a real 3D camera (position + orientation +
  perspective or ortho projection), while `simulation.rs`/`rules.rs` stay
  untouched apart from the `Cell` widening above.
- **`patterns.rs`/`rle.rs`** would need a 3D pattern format of some kind
  (standard RLE has no z-axis) — likely a custom layered-RLE ("z$$" between
  z-slices) or just plain `Vec<(i32,i32,i32)>` literals for a starting set
  of 3D patterns, since there's no equivalent of LifeWiki's 2D pattern
  archive to pull verified 3D ones from.
- **`starts.rs`** needs no change in shape — it already just stamps
  `Pattern::cells` centered on a `Cell`; only `Cell`'s width changes.
- The minimap (currently a 2D top-down `Painter` overlay) would most
  naturally become a small orthographic inset of the same 3D scene from a
  fixed top-down camera, rather than a separate drawing path.

None of this was applied speculatively beyond the neighbor-offset
extraction and these notes — no unused 3D scaffolding, generics, or trait
abstractions were added, since a real 3D renderer is a large enough
undertaking that speculative abstractions now would likely just be wrong
guesses about what the real 3D architecture needs.

## Update — 2026-09-08 (world sized to zoom multiples, icon buttons)

Two quick follow-ups:

- **World resized again, to 960x540** (`x ∈ [-480, 479]`, `y ∈ [-270, 269]`)
  — still 16:9 (the requested "1920x1080 aspect ratio format"), and chosen
  specifically so both axes are an exact integer multiple of
  `view::MIN_CELL_SIZE` (2px) and `view::MAX_CELL_SIZE` (60px): 480/16 cells
  wide and 270/9 cells tall at those two zoom extremes respectively. This
  means the red world-boundary rectangle always lands on a whole-cell grid
  line at min/max zoom instead of ever clipping a partial cell.
- **Icon buttons.** Play/Pause (▶/⏸), Step (⏭), Clear (🗑), Random (🎲), the
  four pan buttons (⬅⬆⬇➡), Reset view (⟲), and the pattern-placement Cancel
  button (✖) now show a symbol instead of a word, each with an
  `on_hover_text` tooltip carrying the full label so the meaning is never
  lost, just deferred to a hover. Verified glyph coverage against the two
  font files egui bundles by default (`epaint_default_fonts`'s
  `emoji-icon-font.ttf` and `NotoEmoji-Regular.ttf`, checked via `fc-query
  --format='%{charset}'`) rather than guessing — one initial choice (✕
  U+2715) turned out to be missing from both and was swapped for ✖ (U+2716,
  present in `emoji-icon-font.ttf`) before shipping. Left Zoom -/+, Skip,
  Start/Load, and Draw/Eraser as text/selectable-labels, since they either
  are already minimal glyphs or don't have an unambiguous universal icon.

`cargo test` (4/4) and `cargo clippy --all-targets` clean; full release
rebuild done.

## Update — 2026-09-08 (later: minimap aspect ratio fix)

The user pointed out (with a screenshot) that the minimap looked wrong —
the viewport outline didn't fill the box evenly, leaving mismatched gaps.
Root cause: `MINIMAP_SIZE` was a hardcoded square (`160.0, 160.0`), but the
world is a 16:9 rectangle (960x540) — `draw_minimap`/`minimap_to_world`
scale x and y independently (`sx`/`sy`), so a square box non-uniformly
stretched the map (and everything on it: viewport outline, occupied-chunk
markers) instead of shrinking it down evenly. Fixed by sizing the minimap
to the same 16:9 ratio as the world (`160.0, 90.0`), which makes `sx == sy`
and removes the distortion entirely — no changes needed to the drawing
logic itself, since it was already generalized to handle any rectangle.

## Update — 2026-09-10 (Google Maps-style scroll, dynamic minimum zoom)

Two UX requests: make pan/zoom feel like Google Maps, and make the minimum
zoom level show the whole map (with the minimap's viewport outline fitting
the box exactly at that point).

**Device-aware scroll.** Previously *all* scrolling (mouse wheel or
trackpad alike) panned, and zooming was pinch/Ctrl+scroll only — a
deliberate earlier choice to stop a touchpad's two-finger scroll from
fighting with zoom (see the 2026-09-07 update above), but it meant a
regular USB mouse's wheel — which most desktop users expect to zoom, à la
Google Maps — panned instead. Fixed by reading raw `egui::Event::MouseWheel`
events directly instead of the pre-merged `smooth_scroll_delta`: each event
carries a `MouseWheelUnit` egui itself assigns from the underlying
`winit::event::MouseScrollDelta` — `Line` for a physical wheel's discrete
notches, `Point` for a trackpad's continuous pixel-precise scrolling
(confirmed by reading `egui-winit`'s conversion code directly rather than
guessing). Now:
- `Line`/`Page` events (mouse wheel) zoom, anchored on the cursor —
  `KEY_ZOOM_STEP.powf(notches)` per frame, matching the feel of the
  `+`/`-` buttons.
- `Point` events (trackpad) pan, preserving the mobile-like two-finger
  behavior from before.
- Events carrying Ctrl/Cmd are skipped in this new code, since
  `zoom_delta()` (unchanged) already handles Ctrl+scroll and pinch
  gestures.

This required no OS/device detection — the distinction was already present
in every scroll event, just discarded by the time `smooth_scroll_delta`
merges everything together.

**Dynamic minimum zoom.** `view::MIN_CELL_SIZE` was a fixed `2.0`, which
turned out to be *larger* than what's needed to fit the whole 960x540 world
in a typical window (e.g. an 800px-wide canvas needs ~0.83px/cell to show
all 960 cells) — so the old fixed floor made it impossible to ever zoom out
far enough to see the entire map, no matter the window size. Replaced with
`view::min_cell_size_to_fit_world(canvas_size)`, computed fresh every frame
from the actual canvas size: `(canvas.x / world_w).min(canvas.y /
world_h)`, i.e. whichever axis is the tighter fit. `View::zoom` now takes
this as a parameter instead of reading a constant, and every call site
(pinch, wheel, keyboard, on-screen buttons/slider) passes the freshly
computed value. `central_canvas` also re-clamps `cell_size` to this bound
once per frame (not just inside `zoom()`) so a window *resize* alone — with
no explicit zoom action — keeps the invariant true; `clamp_to_world`
(unchanged) then centers whichever axis ends up looser than the world
(canvas aspect ratio rarely matches the world's exactly). Net effect: you
can zoom out exactly until the whole map is visible and no further, and at
that point the minimap's yellow viewport rectangle exactly fills the
minimap box, since the visible area and the whole world are now the same
rectangle. Two new unit tests in `view.rs` cover the fit calculation and
the zoom clamp; all 6 tests pass, `cargo clippy --all-targets` clean.

## Update — 2026-09-11 (drag-to-pan, collapsible bars)

The user flagged three related complaints in one message: the top and side
bars eat into the window before the canvas ever gets to show the world's
actual 16:9 shape; pan/zoom still didn't feel like Google Maps because
click-and-drag didn't pan (only scroll/buttons/keys did); and asked for
"some button" to clean up the interface bars.

**Drag-to-pan.** Added a `Tool` enum (`Draw` / `Pan` / `Eraser`, replacing
the old bare `eraser_mode: bool`) as a three-way, mutually-exclusive
toolbox in the Board row. `Pan` makes left-click-drag move the map
directly under the cursor (`response.drag_delta()` fed straight to
`View::pan`) — the actual Google Maps gesture, which the app couldn't
offer before since the primary button was already committed to drawing.
The cursor switches to a grab/grabbing hand icon (`egui::CursorIcon`) while
the tool is active, for a clearer affordance. Also added, independent of
whichever tool is active: a middle-mouse-button drag always pans (tracked
with its own `middle_pan_active` flag, mirroring the existing
`dragging_minimap` pattern so it keeps working if the cursor slips off the
canvas mid-drag) — the same "hold the wheel button" convention used by
Blender/Photoshop/Figma, so panning is always available without switching
tools away from Draw/Eraser.

**Collapsible bars.** Two new toggle buttons in the top bar's now
always-visible essentials strip: "☰" slides the pattern-library panel
off-screen via egui's `Panel::show_collapsible` (built-in slide animation,
still reachable by the same button to bring back), and "⚙" collapses
everything below the essentials strip (Rule/Skip/Speed/stats, the whole
Board row, the whole View row, the custom-rule checkboxes) down to just
Play/Pause, Step, and Gen/Live counts. Collapsing either or both gives the
canvas substantially more room, which is the direct fix for the aspect-
ratio complaint — the map's actual 16:9 shape (`view::clamp_to_world`
already renders it undistorted; the issue was never distortion, just how
little of the window the canvas got once both bars were expanded) is much
easier to make out with the chrome out of the way.

No new tests needed (no new pure logic beyond `Tool` equality checks and
straightforward event-driven panning) — `cargo test` still 6/6,
`cargo clippy --all-targets` clean.

## Update — 2026-09-11 (later: on-canvas zoom, real aspect-ratio fix, Pan discoverability)

Follow-up feedback on the previous round: "put zoom controls on canvas,
there is no pan always paint, you can remove view layer panel from top,
and fix aspect ratio map problem."

**The actual aspect-ratio fix.** The earlier "collapsible bars" change
made more room available but never addressed the real issue: the canvas
was still whatever oddly-shaped leftover area the window and (uncollapsed)
bars produced, so the map — even though internally still a correct
undistorted 16:9 world — only got to fill part of an arbitrarily-shaped
box. Fixed properly this time: `central_canvas` now computes `map_rect`
(via a new `fit_aspect_rect` helper) — the largest exact-16:9 rectangle
that fits centered inside the canvas — and routes every piece of map
rendering and pointer math (grid lines, cell drawing, ghost/eraser cursor,
world-boundary rectangle, `screen_to_cell`/`cell_to_screen`, click/drag
hit-testing) through `map_rect` instead of the raw canvas rect. Whatever
axis doesn't match gets a plain dark letterbox/pillarbox margin instead of
stretching or cropping the map. Draw/Eraser/pattern-placement clicks
outside `map_rect` are now explicitly ignored (they'd previously have
extrapolated to a technically-valid but visually-nonsensical cell).
3 new unit tests for `fit_aspect_rect` (pillarbox/letterbox/exact-match
cases) — 9 tests total, `cargo clippy --all-targets` clean.

**Zoom controls moved onto the canvas.** A small floating `egui::Area` in
the canvas's bottom-left corner (mirroring the minimap's bottom-right
placement) now holds `+`/`-`/`⟲` and a live "Npx/cell" readout, always
present regardless of whether the top bar is expanded or collapsed. The
old top-bar "View" row (Zoom -/slider/+, `⬅⬆⬇➡` pan buttons, Reset view,
Show input debug) was removed entirely — the pan buttons were fully
redundant with the Pan tool, middle-drag, arrow keys, and minimap
dragging; "Reset view" moved into the new overlay; "Show input debug"
moved into the Board row next to "Show grid".

**Pan tool discoverability.** The Draw/Pan/Eraser toggle was living in the
Board row, itself hidden behind the "⚙" collapse toggle — so if extra
controls were collapsed (or just not immediately found), Pan effectively
didn't exist, matching the "there is no pan, always paint" report. Moved
the toggle into the always-visible essentials strip in the top bar
(alongside Play/Pause/Step), so it's reachable no matter what else is
collapsed. The Pan tool's actual logic was already correct — the earlier
build's real problem was that it was too easy not to find, not that the
code was broken.

**Drive-by fix**: the "Random" fill button computed its fill area from
`ui.available_size()` — the top bar row's own width, not the canvas's —
a latent bug now folded in since the app already needed `canvas_size` to
correctly represent `map_rect` for this same round of work. Now uses
`self.canvas_size` directly.

## Update — 2026-09-11 (still later: on-canvas pattern library, bigger zoom buttons, merged Board row)

Follow-up feedback: "improve +/- reset buttons, also put pattern library
on top of canvas, so you know exactly aspect ratio in full screen. also
remove non necessary things from board section and integrate in first one
row simulation."

**Pattern library moved off the side `Panel` onto a floating `Area`.**
`side_panel` (an `egui::Panel::left` that shrank the `CentralPanel` by its
width whenever shown) is gone. In its place, `pattern_library_overlay`
draws the same content — heading, cancel-placement row, categorized
scrollable pattern list — inside an `egui::Area` pinned to the canvas's
top-left corner, still toggled by the same "☰" button. Because an `Area`
paints over the canvas instead of reserving space from it, showing or
hiding the library no longer changes `rect` (the canvas's own size) at
all — which is exactly what "so you know exactly aspect ratio in full
screen" was asking for: in a maximized window, the canvas (and therefore
`map_rect`, computed from it) is now always the true full available area,
never silently narrower because the library happened to be open.

**Zoom overlay buttons enlarged and restyled.** `+`/`−`/`⟲` are now
`ui.add_sized` 34x34 squares with bold 18pt glyphs (verified `−`, U+2212,
against the bundled `Ubuntu-Light.ttf` charset rather than assuming), with
tighter, deliberate spacing (`item_spacing` set explicitly) and a small
`inner_margin` on the popup frame — reads as a real map-style control
cluster now instead of default-sized text buttons crammed into a corner.

**Board row folded into Simulation.** The separate "Board" heading/row
(Start/Load, Clear, Random+density, Show grid, Show input debug) is gone;
everything except "Show input debug" now lives in one `ui.horizontal_wrapped`
under the "Simulation" heading, alongside Rule/Skip/Speed/Births/Deaths —
`horizontal_wrapped` (not `horizontal`) so it wraps to a second line on a
narrow window rather than overflowing, now that there's a lot packed into
one logical row. "Show input debug" — a diagnostic checkbox from early
gesture-debugging sessions, whose job is now covered by the on-canvas zoom
control, the Pan tool, and the device-aware wheel/trackpad split all
having settled into working, understood behavior — was removed entirely
(field, checkbox, and the debug-text overlay it drove), per "remove non
necessary things."

`cargo test` (9/9) and `cargo clippy --all-targets` still clean; full
release rebuild done.

## Update — 2026-09-12 (density-only starts, taller pattern library, robust overlay positioning)

Follow-up feedback: "make options from start all that have density and
remove the single figures or forms. Pattern library should be taller and
grows when opened items and scroll bar in lateral not next to item, also
close button in corner right. And make canvas wider and less tall when
rendering in the window 1920x1080 - is it view cutted?"

**Starting configurations are now all density-based scatters.**
`START_CONFIGS` shrank from 9 to 5: Empty board, Random soup, Glider
field, Gosper gun field, Pulsar field. Removed everything that placed a
single fixed instance (Single glider, Acorn, R-pentomino, Diehard) or a
hardcoded handful (Glider symphony's fixed 4 copies, the old Pulsar
field's fixed 3x3=9). Replaced with a generic `scatter()` helper in
`starts.rs`: it walks a lattice of candidate centers spaced across the
*entire* world (spacing chosen per pattern to keep neighboring instances
from usually overlapping — 24 cells for Glider/Pulsar, 60 for the much
bigger Gosper gun) and stamps one copy at each candidate independently
with probability equal to the same density value the "Random" button's
slider already exposes. "Random soup" itself was widened from a small
80x80 box near the world center to the whole world too, for consistency
with the other options now being true whole-map fills. `starts::apply`
dropped its now-unused `center: Cell` parameter. 3 new unit tests
(density 0.0 places nothing, density 1.0 places something, "Empty board"
clears) — 12 tests total.

**Pattern library overlay improvements**, all in `pattern_library_overlay`:
- Categories are now `default_open(true)` (previously collapsed), and the
  panel's `ScrollArea` height budget grew from `canvas_height - 100` to
  `canvas_height - 3*margin - 70` with a higher floor (240 vs 120) — the
  panel now visibly grows as more categories' contents are shown, up to
  nearly the full canvas height, rather than being capped short.
- `ScrollArea::auto_shrink([false, true])`: the scroll area now always
  claims the panel's full fixed width, so its scrollbar sits flush at the
  panel's own right edge instead of hugging whichever row's content
  happened to be widest.
- The "✖" close button moved into a `Layout::right_to_left` sub-layout in
  the heading row, pinning it to the panel's top-right corner instead of
  sitting immediately after the heading text.

**Zoom overlay's positioning made robust against clipping.** It previously
used `.fixed_pos(...)` computed from a *guessed* content height (168px) —
if the actual rendered height ever drifted from that guess (a different
font, a text-scaling setting, etc.) the overlay's bottom could in
principle render past the window edge. Switched to
`.anchor(Align2::LEFT_BOTTOM, offset)`, which lets egui measure the area's
actual size and position it from the true window corner — this can't
clip regardless of window size, so nothing is "cut" at 1920x1080 or any
other resolution. (Investigated directly rather than guessing: confirmed
`Area::anchor`'s `constrain_rect` defaults to `ctx.content_rect()`, the
full window content area, by reading `egui`'s source.)

**Default window size widened.** `main.rs`'s initial size changed from
1100x720 (aspect 1.53) to 1440x810 (aspect 1.78, exactly 16:9) — wider and
shorter, per the request. Note this doesn't eliminate letterbox/pillarbox
bars entirely: the top bar still subtracts some height from whatever the
window's total height is, which makes the *canvas* (window minus top bar)
slightly wider-than-16:9 relative to the world, so `fit_aspect_rect` will
still pillarbox a small amount depending on how tall the top bar currently
is (collapsed vs. expanded) — this is expected letterboxing to preserve
the map's true, undistorted aspect ratio, not a bug, and collapsing the
top bar ("⚙") minimizes it.

`cargo test` (12/12) and `cargo clippy --all-targets` clean; release
rebuild done.

## Update — 2026-09-12 (later: Rule dropdown grouped by behavior class)

Small follow-up: "add also in rules submenu or lateral or dropdown menu
based on type chaotic, explosive, stable." The "Rule" `ComboBox` previously
listed all 22 presets as one flat list with the class appended to each
row's label (e.g. "Diamoeba (chaotic)"). Now it's grouped: a small bold
"chaotic" / "explosive" / "stable" header per section, with that class's
presets listed indented underneath (still plain `selectable_label` rows
inside the same `ComboBox`, not actual nested sub-menus — simpler, and
egui's `ComboBox` doesn't support flyout submenus anyway). No behavior
change to preset selection itself, `preset_class` still tracks the class
label shown next to the B/S string.

`cargo test` (12/12, unaffected — this is pure UI layout) and `cargo
clippy --all-targets` clean; release rebuild done.

## Next up (priority order)

1. **Pattern placement niceties.** Rotate/flip the selected pattern before
   stamping (R / F keys), since guns and spaceships are directional.

2. **Persistence.** Save/load the current board as RLE (export what's
   drawn, import a pattern file from disk) — currently patterns only come
   from the built-in library.

3. **Windows/macOS build verification.** Only Linux has actually been
   built and run so far. The dependency set (`eframe`, `rand`) is
   cross-platform with no OS-specific code, so this should mostly be a
   matter of running `cargo build --release` on each target and fixing
   anything that comes up (packaging/icon per OS if desired). On macOS in
   particular, the pinch/pan gesture path should actually light up, unlike
   on Linux — worth confirming.

4. **Packaging.** Right now it's a raw binary + a hand-written `.desktop`
   file. Consider `cargo-bundle` or `cargo-packager` for a proper
   `.app`/`.exe`/`.AppImage` if this needs to be distributed beyond this
   machine.

## Nice-to-haves (not scheduled)

- More library patterns per category, especially a second/third gun (only
  Gosper and Simkin so far).
- More starting configurations (e.g. a symmetric 4-gun crossfire, a
  same-rule "known chaotic seed" per preset).
