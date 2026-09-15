# Roadmap

Forward-looking notes only. For a changelog of everything already shipped,
see [RELEASES.md](RELEASES.md).

Scope, deliberately small: *a clean Rust implementation and playground for
Conway's Game of Life and Life-like cellular automata*. Explicitly not a
Golly clone, not a general CA framework, not an ALife framework, not a
multi-dimensional engine. 3D is out of scope for this repository — if it
ever happens it is a separate experiment, not a feature here.

Ordered by value. Each item is small enough to land as its own commit with
its own tests.

---

## 0. Get the build gates honest (do this first)

Nothing else should land on top of a red gate.

- **`cargo clippy --all-targets -- -D warnings` currently fails** with two
  `redundant_pattern_matching` errors (`src/app.rs`, the two
  `if let Some(_) = self.selected_pattern` arms). CI runs exactly this
  command, so CI is red on HEAD.
- **`cargo fmt --check` produces a ~1000-line diff.** There is no
  `rustfmt.toml` and CI never checks formatting, so the tree has drifted
  from any single style. Pick a config (the existing code favours wide
  lines, so `max_width` around 110 keeps the diff small), commit it, apply
  it once, and add `cargo fmt --check` to CI so it can't drift again.

## 1. Core quality

The generic B/S architecture is right and stays. `RuleSet { birth: [bool;
9], survive: [bool; 9] }` with one `next_generation` is genuinely generic
over Life-like rules; no rule is special-cased. This section is about
testing and tightening that core, not redesigning it.

- **Test the rule engine.** `simulation.rs`, `rules.rs` and `rle.rs` have
  no tests at all today. The behavioural core of the program is unverified.
  Start with:
  - Conway B3/S23: blinker has period 2, block is a still life, glider
    returns to its own shape displaced by (1, 1) after 4 generations.
  - Rule genericity: Seeds (B2/S) leaves nothing alive that wasn't just
    born; HighLife's B6 replicator behaves differently from Conway's on the
    same input.
  - World boundary: nothing is ever born outside `WORLD_MIN`/`WORLD_MAX`;
    a pattern stamped across an edge is clipped, not wrapped or panicking.
  - `step_n(n)` is equivalent to calling `step()` n times, and `step_n(0)`
    advances exactly one generation (its documented behaviour).
  - `RuleSet::to_bs_string` round-trips (needs the parser in the next item).
- **Bound the catch-up loop in `SimState::tick`.** It currently drains the
  accumulator with an unbounded `while`. This is reachable, not
  theoretical: a full-world random soup steps at roughly 12 gen/s while the
  speed slider goes to 60 gen/s, so the accumulator grows faster than it
  drains and the window stops responding. Cap the steps per tick, drop the
  backlog beyond the cap, and test that a large `dt` performs a bounded
  amount of work.
- **Add `RuleSet::from_bs_string`.** `to_bs_string` has no inverse, so a
  rule can be displayed but never typed in or read back from a file. Needed
  by RLE import anyway (item 2), and it makes the B/S round-trip testable.
- **Make `Preset::class` an enum.** It is a `&'static str` compared against
  a hardcoded `["chaotic", "explosive", "stable"]` list in `app.rs`; a typo
  in a preset silently drops it from the dropdown with no compile error.
  Three variants with a `label()`, same shape as `patterns::Category`.
- **Split `central_canvas`.** `app.rs` is 936 lines and `central_canvas` is
  a single ~300-line closure handling zoom, scroll, middle-drag panning,
  keyboard shortcuts, minimap hit-testing, tool dispatch, grid painting,
  cell painting, ghost preview, the world border and the zoom overlay.
  Extract `handle_input` / `draw_grid` / `draw_cells` / `draw_overlays` as
  plain methods. No traits, no new abstractions — just smaller functions.
- **Prune stale prose.** `simulation.rs` cites `app::fit_aspect_rect`,
  which no longer exists. `app.rs` carries 198 comment lines, many of them
  multi-paragraph justifications of UI decisions that `RELEASES.md` already
  records in full. Drop the 3D migration essays in `simulation.rs`,
  `app.rs` and this file — 3D is out of scope, and speculative notes about
  a renderer nobody is writing are guesses, not documentation.

## 2. Pattern interoperability

`rle.rs` is 44 lines and adequate for the trusted static strings in
`patterns.rs`, but it is also the designated import path, and it is not
ready for input it did not write itself. Verified against the current
parser:

| Input | Current result |
|---|---|
| `"zq3o!"` | 3 cells — unknown letters are silently ignored |
| `"3o!\n2o!"` | 5 cells — `!` breaks the char loop, not the line loop |
| `"99999999999o!"` | panics (`multiply with overflow`) in debug; wraps silently in release |
| `"3o"` | 3 cells — a missing terminator is not an error |

- **Return `Result`.** `parse` returns a bare `Vec`, so nothing above it can
  distinguish a valid pattern from garbage. Add a small error type naming
  the offending line and character.
- **Reject what should be rejected**: unknown characters, a missing `!`,
  and run counts that overflow (use checked arithmetic).
- **Stop at `!`** rather than continuing into later lines.
- **Parse the header instead of skipping it.** Validate the decoded pattern
  against the declared `x` / `y`, and surface `rule = B3/S23` so importing
  a pattern can offer to switch to the rule it was designed for.
- **Add an exporter** — `to_rle(cells) -> String`, with the standard
  header — so a board drawn in the app can leave it. This is the missing
  half of "Persistence" from the old roadmap.
- **Round-trip tests**: every pattern in the built-in library survives
  `parse -> to_rle -> parse` unchanged, plus a table of malformed inputs
  that must each produce a specific error rather than a wrong pattern or a
  panic.

Scope limit: RLE in and RLE out, correctly. No `.lif`, no `.mc`, no
macrocell, no pattern-collection loader, no network fetching.

## 3. Life analysis

None of this exists today; all of it is cheap and lives entirely in the
core with no UI dependency, which makes it easy to test.

- **Bounding box and population** over the live set — a few lines each, and
  a prerequisite for everything below.
- **Still-life detection**: next generation equals the current one.
- **Oscillator detection and period**: step up to some small bound, looking
  for a return to a previously seen generation. Keep the bound small and
  explicit rather than open-ended.
- **Spaceship detection**: the same cycle search, but comparing normalised
  (translated-to-origin) generations, reporting the displacement and period
  when the shape returns shifted rather than in place.

That is the whole of it — a `analysis.rs` with a handful of pure functions
over `&HashSet<Cell>`, plus tests using the existing library patterns as
fixtures (blinker: period 2; glider: period 4, displacement (1, 1); block:
still life). No classification database, no census, no soup search.

## 4. Performance

Measured first, on this machine, release build, 60 generations of B3/S23:

| Scenario | `SimState::step` | `next_generation` alone | naive dense `Vec<u8>` |
|---|---|---|---|
| 200x200 soup (~12k live) | 284 gen/s | 397 gen/s | 206 gen/s |
| Full-world soup (~138k live) | 12.5 gen/s | 16.1 gen/s | 202 gen/s |

Two conclusions, and the second matters more than the first:

**Do not switch to a dense grid.** The sparse `HashSet` is *faster* than a
dense whole-world scan at the populations this playground actually runs at,
and only loses on full-world soups. Swapping representations would trade
the common case for the rare one.

**The bookkeeping costs more than it returns.** `step()` runs at 284 gen/s
where `next_generation` alone runs at 397 — roughly 28% of every single
generation is spent maintaining things the simulation itself does not need:

- `rebuild_chunks()` re-hashes every live cell **every generation** to
  refresh an index that mirrors the live set. The minimap needs coarse
  occupancy; `cells_in_bounds` mostly does not, since a 960x480 world is
  small enough to filter directly. Build the index inline in
  `next_generation`, or derive minimap occupancy on demand.
- `last_births` / `last_deaths` cost two extra full set-difference passes
  per step, always, to populate two labels.
- `HashSet<(i64, i64)>` hashes 16 bytes through SipHash per lookup. The
  world fits comfortably in a single `u32` index (960 x 480), so packing
  cells cuts that to 4 bytes with no new dependency.

Do these three, re-measure, and stop. Only if full-world soups turn out to
be a case worth caring about does a dense or bitboard backend become
justified — and that decision should be made against a committed benchmark,
not a guess.

**Commit the benchmark.** The numbers above came from a throwaway harness.
A small `benches/` (or an `#[ignore]`d timing test, to avoid adding a
dev-dependency) makes them reproducible and turns "is this faster?" into a
question with an answer.

Rendering has the same cliff and is lower priority: one `rect_filled` per
visible cell means a fully-zoomed-out full world issues ~460k draw calls
per frame. Worth batching only once the simulation is no longer the
bottleneck.

## 5. Optional experiment — HashLife

Only after 0-4 are done, and only as a **separate, optional backend**
behind the existing `SimState` API, chosen explicitly rather than by
default. It is worth attempting for the fun of it, not because the project
needs it. If it does not stay comfortably self-contained, drop it — it must
not become the reason the core gets harder to read.

## Still open from before

- **Release workflow — exercised, and the Flatpak blocker is fixed.**
  Correcting an earlier claim here that it "has never been exercised by a
  real run": it has run three times. v0.1.0 succeeded; v0.1.2 and v0.1.3
  both failed, which is why those two tags have GitHub releases with no
  artifacts attached. On v0.1.3 the five other jobs (Linux, Windows, both
  macOS) all passed and only the Flatpak job failed, at
  `cargo build --release --offline`: `no matching package named eframe`.
  Flatpak build sandboxes have no network, so `--offline` needs the
  dependencies vendored up front, and the `.cargo/config.toml` that
  arranged that was deleted in `e15028e`. Because the `release` job is
  `needs: [... linux-flatpak ...]` with `if: success()`, that one failure
  skipped publishing entirely.

  Fixed for v0.1.4 with `flatpak/cargo-sources.json` (generated from
  `Cargo.lock` by flatpak-builder-tools' `flatpak-cargo-generator.py`),
  which lists every crate as a Flatpak source instead of vendoring their
  code into the repo. Regenerate it whenever `Cargo.lock` changes:

  ```sh
  python3 flatpak-cargo-generator.py Cargo.lock -o flatpak/cargo-sources.json
  ```

  Still unverified: whether the Flatpak job now passes on a real runner.
  That is what tagging v0.1.4 tests.
- **Android packaging — not implemented.** eframe can target Android, but
  it needs an `android_main` entry point, Android-specific dependencies, an
  `AndroidManifest.xml`, and a `cargo-apk`/`cargo-ndk` + Gradle pipeline.
  None of it overlaps with the desktop build. Worth scoping separately, if
  at all.

## Nice-to-haves (not scheduled)

- A few more library patterns per category, especially a third gun.
- More starting configurations (a symmetric 4-gun crossfire; a known
  interesting seed per preset).
