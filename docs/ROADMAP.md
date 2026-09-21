# Roadmap

Forward-looking notes only. For everything already shipped, see
[RELEASES.md](RELEASES.md); for how to cut a release, see
[RELEASING.md](RELEASING.md).

Scope, deliberately small: *a clean Rust playground for Conway's Game of Life
and Life-like (totalistic B/S) cellular automata, that exchanges patterns and
rules with Golly*. Golly compatibility means **interchange** — its RLE
format and rule strings — not reproducing Golly's other algorithms.

## Next

Nothing is queued. Every item that was on this list has either shipped or
been decided against (below). Add an item here only with a concrete reason
to do it now.

## Decided against

Each of these was considered and dropped on purpose, so it does not get
re-litigated. Reopen one only if the reason no longer holds.

- **B0 rules** (birth on zero neighbours). The step only visits cells next
  to a live cell, so they cannot be simulated; the parser rejects them with
  a message instead. Golly emulates them by alternating the rule with its
  complement and inverting the display, which on a sparse board would mean
  storing the complement of the pattern on every other generation.
- **Running the step on a worker thread.** Playing pauses at 500,000 live
  cells (`SimState::population_limit`), which keeps a generation well under
  a frame or two. A thread is only worth its complexity if that limit is
  raised.
- **Batching draw calls further.** Live cells already go out as one mesh per
  60,000 vertices. Revisit only if a profile shows drawing, not the step,
  as the bottleneck.
- **A dense or bitboard backend, and HashLife.** The sparse set is faster at
  the populations the playground runs (see the benchmark in `simulation.rs`).
- **Analysis in the UI.** `analysis.rs` classifies patterns (still life /
  oscillator / spaceship / unsettled) and backs the collection tests; it is
  compiled under `cfg(test)`. Surfacing it as a "what is this?" label is not
  wanted. No census, no soup search, no classification database.
- **More pattern collections for their own sake.** Add a collection when a
  rule has patterns worth shipping and each one can be verified by
  simulation. Day & Night has no spaceship because none turned up in every
  4 x 4 pattern; known larger ones need a source to copy from.
- **Loading a folder of Golly patterns.** Import is one `.rle` file at a
  time.
- **Golly's other algorithms**: Generations, WireWorld, non-totalistic,
  multi-state, `:T`/`:P` bounded grids, scripting.
- **3D.** A separate experiment if it ever happens, not a feature here.
- **Android.** Shares nothing with the desktop build (`android_main`,
  manifest, `cargo-apk`/Gradle); scope separately, if at all.
