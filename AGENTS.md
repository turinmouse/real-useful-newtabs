# Repository Guidelines

## Project Structure & Module Organization

This repository builds a Rust 2024 `egui`/`eframe` Chrome new-tab extension. Application code lives in `src/`: `app.rs` owns UI state and layout, `theme.rs` centralizes visual styling, `lib.rs` exposes shared modules, and `main.rs` contains native and WebAssembly entry points. Static extension files belong in `extension/`. The tasks in `mise.toml` compile the WebAssembly bundle and copy those files into generated `dist/`; do not edit or commit `dist/` or `target/`. Read `DESIGN.md` before changing UI behavior or appearance.

## egui UI Implementation

Use egui/eframe-provided widgets, containers, layout, interaction, painting, state, and persistence APIs for all UI implementation. Do not implement a custom UI capability unless egui/eframe does not provide it and the user has explicitly approved the custom implementation. The currently approved exceptions are the shared full-row tree adapter used for favicon composition and bookmark drag-and-drop, and the adapter that translates floating Window layers using the canvas ScrollArea offset; expanding either exception still requires user approval.

## Build, Test, and Development Commands

- `mise run dev` launches the native development build.
- `mise install` installs the pinned `wasm-bindgen-cli`.
- `mise run check` checks all native targets and the `wasm32-unknown-unknown` target.
- `mise run test` runs the Rust and JavaScript test suites.
- `mise run build-extension` (debug) or `mise run release` (release) produces an unpacked Chrome extension in `dist/`.
- `mise run fmt-check` verifies Rust formatting; run `mise run fmt` to apply it.
- `mise run clippy` catches common Rust issues.


After building, load `dist/` through `chrome://extensions` in Developer mode for browser verification.



## Testing Guidelines

Place focused unit tests in a local `#[cfg(test)] mod tests` block near the code under test. Name tests after observable behavior, such as `bookmarks_window_starts_closed`. Run `mise run test` and `mise run check` before submitting. There is no configured coverage threshold; add tests for state transitions, layout metrics, and regressions where practical. Manually verify extension UI changes in Chrome, including narrow-window and interaction states.

## Commit & Pull Request Guidelines

The repository currently has no commit history, so no established message convention exists. Use short, imperative subjects (for example, `Add bookmark window empty state`) and keep each commit focused. Pull requests should explain the change and validation performed, link relevant issues, and include screenshots or recordings for visible UI changes. Call out changes to `extension/manifest.json`, especially any new Chrome permissions.
