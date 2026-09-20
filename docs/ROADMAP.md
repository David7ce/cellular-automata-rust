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

## 1. Build gate: formatting

`cargo clippy --all-targets -- -D warnings` and `cargo test` are green and CI
runs both. Formatting is not gated: there is no `rustfmt.toml`, CI never runs
`cargo fmt --check`, and it currently reports 94 diffs. Pick a config (the
code favours wide lines; `max_width` around 110 keeps the diff small), apply
it once in its own commit, and add `cargo fmt --check` to CI.

## 2. Core correctness

- **Performance is bounded by bookkeeping, not the rule.** Measured on a
  release build, 60 generations of B3/S23: `SimState::step` 284 gen/s on a
  200x200 soup versus 397 gen/s for `next_generation` alone — about 28% of
  each generation goes to state the simulation does not need:
  - `rebuild_chunks()` re-hashes every live cell every generation to refresh
    a spatial index. Build it inline in `next_generation`, or derive minimap
    occupancy on demand.
  - `last_births` / `last_deaths` cost two extra set-difference passes per
    step to fill two labels.
  - `HashSet<(i64, i64)>` hashes 16 bytes through SipHash per lookup; the
    world fits one `u32` index (960 x 480).

  Do these three, re-measure, stop. **Do not switch to a dense grid**: on
  the populations the playground actually runs the sparse set is faster
  (284 vs 206 gen/s at 12k cells) and only loses on full-world soups
  (12.5 vs 202 gen/s). Commit a benchmark (an `#[ignore]`d timing test
  avoids a dev-dependency) so "is it faster?" has an answer.
- **`Preset::class` is a `&'static str`** compared against a hardcoded
  `["chaotic", "explosive", "stable"]` list in `app.rs`; a typo silently
  drops a preset from the dropdown. Make it a three-variant enum with a
  `label()`, same shape as `patterns::Category`.
- **Split `central_canvas`** (`app.rs`, ~300-line closure handling zoom, pan,
  keyboard, minimap, tools, grid, cells, ghost preview and overlays) into
  `handle_input` / `draw_grid` / `draw_cells` / `draw_overlays` methods.
  Plain methods, no new abstractions. Prune the 3D-migration essays in the
  comments of `app.rs` and `simulation.rs` while in there.

## 3. Golly interchange — what is left

Shipped: RLE read/write with rule header, `B3/S23` / `23/3` rule strings,
clipboard copy and paste, per-rule pattern collections.

- **Open / save `.rle` files** through a native dialog (`rfd`). Deliberately
  deferred: clipboard covers the Golly workflow without a dependency, and a
  file dialog adds Flatpak portal surface. Do it when someone needs to load
  a Golly pattern *collection* rather than one pattern at a time.
- **B0 rules.** Rejected today, because the sparse step only visits cells
  adjacent to a live cell. Golly emulates them by alternating the rule with
  its complement; on a bounded 960 x 480 world that means storing the
  inverted board on odd generations. Only worth it if a specific B0 rule is
  wanted.
- **More collections**, one rule at a time, each entry verified by
  simulation before it goes in. Seeds' and Day & Night's entries came from a
  brute-force search of small patterns. No Day & Night spaceship turned up
  in every 4 x 4 pattern, so that collection has still lifes and
  oscillators only; known larger spaceships need a source to copy from.

## 4. Analysis in the app

`analysis.rs` classifies a pattern (dies / still life / oscillator with
period / spaceship with period and displacement / unsettled) by bounded
simulation and currently backs the collection tests only, so it is compiled
under `cfg(test)`. Surface it in the UI (a "what is this?" label for the
board, plus bounding box and population) only if wanted; move it out of
`cfg(test)` at that point. No census, no soup search, no classification
database.

## 5. Release

- **Flatpak job unverified on a real runner.** v0.1.2 and v0.1.3 published
  no artifacts because the Flatpak job failed offline
  (`no matching package named eframe`), which skips publishing by design.
  Fixed in v0.1.4 with `flatpak/cargo-sources.json`; tagging v0.1.4 is the
  test. Regenerate it whenever `Cargo.lock` changes:

  ```sh
  python3 flatpak-cargo-generator.py Cargo.lock -o flatpak/cargo-sources.json
  ```
- **Android** is not implemented and shares nothing with the desktop build
  (`android_main`, manifest, `cargo-apk`/Gradle). Scope separately, if at
  all.
