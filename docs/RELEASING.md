# Releasing

1. **Gates green on `main`**: `cargo fmt --check`, `cargo clippy --all-targets
   -- -D warnings`, `cargo test` (CI runs all three on every push).
2. **Bump the version** in `Cargo.toml` (and let `Cargo.lock` follow).
3. **Add a `<release version="X.Y.Z" date="..."/>` entry** (newest first) to
   `flatpak/io.github.David7ce.CellularAutomata.metainfo.xml`. The Flatpak
   job fails the whole release if the tagged version is missing there.
4. **Write the changelog** in `docs/RELEASES.md` (rename `Unreleased` to the
   version and date).
5. **If `Cargo.lock` gained or changed a dependency**, regenerate the offline
   Flatpak sources, otherwise the Flatpak build fails with
   `no matching package named ...` and, since publishing needs every job,
   the release ships with no artifacts:

   ```sh
   python3 flatpak-cargo-generator.py Cargo.lock -o flatpak/cargo-sources.json
   ```

   (`flatpak-cargo-generator.py` is from flatpak-builder-tools; it needs
   `pip install aiohttp tomlkit`.)
6. **Commit, push, wait for CI**, then tag and push the tag:

   ```sh
   git tag vX.Y.Z && git push origin vX.Y.Z
   ```

   The `Release` workflow builds Linux (AppImage, tar.gz, Flatpak), Windows
   (installer, portable ZIP) and macOS Apple Silicon (DMG, ZIP), then
   publishes them with `SHA256SUMS`. Publishing needs every job to pass.

To check that every platform still builds without publishing, run the
workflow by hand with no tag: `gh workflow run Release --ref main`.
