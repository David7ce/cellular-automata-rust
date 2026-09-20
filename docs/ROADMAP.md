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

- **Performance is bounded by bookkeeping, not the rule.** Measured on a
  release build, 60 generations of B3/S23: `SimState::step` 284 gen/s on a
  200x200 soup versus 397 gen/s for `next_generation` alone — about 28% of
  each generation goes to state the simulation does not need:
  - `rebuild_chunks()` re-hashes every live cell every generation to refresh
    a spatial index. Build it inline in `next_generation`, or derive minimap
    occupancy on demand.
  - `HashSet<(i64, i64)>` hashes 16 bytes through SipHash per lookup; the
    world fits one `u32` index (4096 x 4096 is 24 bits).

  Do these two (the births/deaths passes are already merged into one),
  re-measure, stop. **Do not switch to a dense grid**: on the populations the playground actually runs the sparse set is faster
  (284 vs 206 gen/s at 12k cells) and only loses on full-world soups
  (12.5 vs 202 gen/s). Commit a benchmark (an `#[ignore]`d timing test
  avoids a dev-dependency) so "is it faster?" has an answer.
- **Split `central_canvas`** (`app.rs`, ~300-line closure handling zoom, pan,
  keyboard, minimap, tools, grid, cells, ghost preview and overlays) into
  `handle_input` / `draw_grid` / `draw_cells` / `draw_overlays` methods.
  Plain methods, no new abstractions. Prune the 3D-migration essays in the
  comments of `app.rs` and `simulation.rs` while in there.

## 2. Golly interchange — what is left

Shipped: RLE read/write with rule header, `B3/S23` / `23/3` rule strings,
clipboard copy and paste, `.rle` file import, per-rule pattern collections.

- **Save `.rle` files.** Import has a file picker (`rfd`) and export goes
  through the clipboard; add a save dialog if pasting into a file by hand
  gets tedious. Loading a whole Golly pattern *collection* is not planned.
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
