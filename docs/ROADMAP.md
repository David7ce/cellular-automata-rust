# Roadmap

Forward-looking notes only. For a changelog of everything already shipped
(one entry per round of feedback, in detail), see
[RELEASES.md](RELEASES.md).

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

## Next up (priority order)

1. **Verify the release workflow end to end.** `.github/workflows/release.yml`
   was written and reviewed but not yet exercised by an actual CI run (no
   way to trigger GitHub Actions from this environment) — push a `v*` tag
   or run it via `workflow_dispatch` and fix whatever the first real run
   surfaces, especially the Flatpak and Windows Inno Setup steps, which are
   the most likely to need adjustment on first contact with a real runner.

2. **Android packaging — not implemented.** eframe/egui apps *can* target
   Android, but it needs real additional groundwork beyond what exists
   here: an `android_native_app_glue`-based entry point (`#[no_mangle]
   android_main`), Android-specific `Cargo.toml` dependencies, an
   `AndroidManifest.xml`, and a build pipeline (`cargo-apk` or `cargo-ndk` +
   Gradle) to produce a signed APK — none of which overlaps with the
   desktop build this project has today. Deliberately not attempted as
   part of the desktop-packaging pass, rather than shipping something
   untested that looks done but isn't; worth scoping as its own task.

3. **Pattern placement niceties.** Rotate/flip the selected pattern before
   stamping (R / F keys), since guns and spaceships are directional.

4. **Persistence.** Save/load the current board as RLE (export what's
   drawn, import a pattern file from disk) — currently patterns only come
   from the built-in library.

## Nice-to-haves (not scheduled)

- More library patterns per category, especially a second/third gun (only
  Gosper and Simkin so far).
- More starting configurations (e.g. a symmetric 4-gun crossfire, a
  same-rule "known chaotic seed" per preset).
