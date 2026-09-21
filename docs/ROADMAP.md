# Roadmap

Forward-looking notes only. For everything already shipped, see
[RELEASES.md](RELEASES.md).

Scope, deliberately small: *a clean Rust playground for Conway's Game of Life
and Life-like (totalistic B/S) cellular automata, that exchanges patterns and
rules with Golly*. Golly compatibility means **interchange** — its RLE
format and rule strings — not reproducing Golly's other algorithms
(Generations, WireWorld, HashLife, non-totalistic rules, multi-state,
scripting). 3D is out of scope.

Ordered by value. Each item is small enough to land as its own commit with
its own tests.

---

## 1. Core correctness

- **Rendering is the next performance cliff.** One `rect_filled` per
  visible live cell means a fully zoomed-out board with hundreds of
  thousands of cells issues that many draw calls per frame. Batch (a mesh,
  or one rect per row run) only if it shows up as a problem; the step is no
  longer the bottleneck (see RELEASES for the numbers). Do not switch to a
  dense grid: the sparse set beats it at the populations the playground
  actually runs.
- **Split `central_canvas`** (`app.rs`, ~300-line closure handling zoom, pan,
  keyboard, minimap, tools, grid, cells, ghost preview and overlays) into
  `handle_input` / `draw_grid` / `draw_cells` / `draw_overlays` methods.
  Plain methods, no new abstractions. Prune the 3D-migration essays in the
  comments of `app.rs` and `simulation.rs` while in there.

## 2. Golly interchange — what is left

Shipped: RLE read/write with rule header, `B3/S23` / `23/3` rule strings,
clipboard copy and paste, `.rle` file import and save, per-rule pattern
collections.

- Loading a whole Golly pattern *collection* (a folder of `.rle` files) is
  not planned; import is one file at a time.
- **Full-world soups are slow.** The world is 4096 x 4096, so an explosive
  rule can now grow to millions of live cells, and one generation of that
  takes seconds (the per-tick cap bounds generations per frame, not the
  cost of one). Playing now pauses at 1,000,000 live cells; moving the
  step to a worker thread would remove the remaining stall.
- **B0 rules.** Rejected today, because the sparse step only visits cells
  adjacent to a live cell. Golly emulates them by alternating the rule with
  its complement; on the bounded 4096 x 4096 world that means storing the
  inverted board on odd generations. Only worth it if a specific B0 rule is
  wanted.
- **More collections**, one rule at a time, each entry verified by
  simulation before it goes in. Seeds' and Day & Night's entries came from a
  brute-force search of small patterns. No Day & Night spaceship turned up
  in every 4 x 4 pattern, so that collection has still lifes and
  oscillators only; known larger spaceships need a source to copy from.

## 3. Analysis in the app

`analysis.rs` classifies a pattern (dies / still life / oscillator with
period / spaceship with period and displacement / unsettled) by bounded
simulation and currently backs the collection tests only, so it is compiled
under `cfg(test)`. Surface it in the UI (a "what is this?" label for the
board, plus bounding box and population) only if wanted; move it out of
`cfg(test)` at that point. No census, no soup search, no classification
database.

## 4. Release

- **Keep `flatpak/cargo-sources.json` in sync.** The Flatpak build is
  offline, so it must be regenerated whenever `Cargo.lock` gains or changes
  a dependency (v0.1.2/v0.1.3 shipped without artifacts because it was
  missing; v0.1.4 fixed that and published):

  ```sh
  python3 flatpak-cargo-generator.py Cargo.lock -o flatpak/cargo-sources.json
  ```
- **Android** is not implemented and shares nothing with the desktop build
  (`android_main`, manifest, `cargo-apk`/Gradle). Scope separately, if at
  all.
