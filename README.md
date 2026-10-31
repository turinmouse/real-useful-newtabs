<div align="center">

# real-useful-newtabs

**A new tab that works like a workspace.**

Organize bookmarks and tab groups across tiled canvases, right inside Chrome.

[English](README.md) · [简体中文](README.zh-CN.md)

[Get started](#get-started) · [Features](#features) · [Development](#development) · [Report an issue](https://github.com/turinmouse/real-useful-newtabs/issues)

[![License](https://img.shields.io/badge/License-GPL--3.0--only-blue)](LICENSE)
![Rust](https://img.shields.io/badge/Rust-2024_edition-orange?logo=rust)
![UI](https://img.shields.io/badge/UI-egui-blue)
![Platform](https://img.shields.io/badge/Chrome-Manifest_V3-green?logo=googlechrome&logoColor=white)

</div>

## Why this project?

Your new tab can be a place to organize ongoing work. Keep bookmark folders and saved tab groups side by side, arrange them into separate canvases, and return to the layout you left behind.

Built with Rust, egui/eframe, and WebAssembly. The Chrome extension is the primary experience; a native app is available for UI development.

## Features

| Feature | What you can do |
| --- | --- |
| Multiple canvases | Create, rename, and switch between workspaces with independent layouts. |
| Tiled windows | Drag titles to rearrange tools, drag dividers to resize them, and scroll oversized content within each tile. |
| Bookmarks | Search titles and URLs, open folders in their own windows, rename or delete entries, and reorder with drag-and-drop. |
| Saved tab groups | Browse saved groups, search their URLs, and reopen recorded pages in the corresponding Chrome group. |
| Tab group enhancement | Optionally associate pages or domains with named groups and manage custom URL rules. |
| New-tab behavior | Optionally keep new-tab pages at the far right or reuse an existing new tab in the same window. |
| Appearance | Switch between dark and light themes. |

Bookmark reordering is available when no bookmark filter is active. Tab group enhancement, rightmost positioning, and new-tab reuse are disabled by default.

## Get started

### 1. Prepare the tools

Install Git and [mise](https://mise.jdx.dev/). The project requires Rust **1.95 or later**; mise manages Rust, its WebAssembly target, Node.js, and the wasm-bindgen CLI.

```sh
git clone https://github.com/turinmouse/real-useful-newtabs.git
cd real-useful-newtabs
mise trust
mise install
```

The CLI version in `mise.toml` must match the `wasm-bindgen` package in `Cargo.lock` (currently `0.2.129`). Update both together when that dependency changes.

### 2. Build the extension

```sh
mise run release
```

The unpacked extension is generated in `dist/`.

### 3. Load it into Chrome

1. Open `chrome://extensions`.
2. Enable **Developer mode**.
3. Click **Load unpacked** and select this project's `dist/` directory.
4. Open a new tab.

After rebuilding, click **Reload** on the extension card and open a fresh new tab. If you moved or renamed the project directory, check that Chrome is loading the current `dist/` path.

## Make it yours

- Open **Bookmarks**, **Tab Groups**, or **Settings** from the right navigation.
- Drag a window title to another window's edge to split the layout, or to its center to swap positions.
- Create canvases for separate contexts, such as work and personal browsing.
- In **Settings**, choose whether links opened in new tabs activate those tabs, and enable the optional browser behaviors you want.
- For tab group enhancement, use named Chrome groups. Where Chrome supports the tab context menu, use **Tab group enhancement → Pin domain / Pin full path** to record an association. Rules can also be managed in the Tab Groups tool.

Global settings are shared between open new-tab pages. Each page keeps its active canvas and layout independent while saving them for later use.

## Permissions and data

| Chrome permission | Purpose |
| --- | --- |
| `bookmarks` | Read, rename, delete, and move bookmarks at your request. |
| `contextMenus` | Provide tab group association actions where supported. |
| `favicon` | Display website icons beside entries. |
| `tabs` | Open and manage tabs for navigation and optional new-tab behavior. |
| `tabGroups` | Read and manage Chrome tab groups. |
| `storage` | Persist settings, layouts, saved groups, and session bindings. |

Settings, layouts, and saved groups are stored in the browser; temporary group bindings use session storage. Live settings updates between pages are not cross-device synchronization. Bookmark edits affect your actual Chrome bookmarks.

## Development

| Command | Purpose |
| --- | --- |
| `mise run dev` | Run the native UI for development. Chrome APIs require the extension. |
| `mise run build-extension` | Build a debug extension in `dist/`. |
| `mise run release` | Build an optimized extension in `dist/`. |
| `mise run check` | Check native targets and WebAssembly. |
| `mise run test` | Run Rust and JavaScript tests. |
| `mise run fmt` / `mise run fmt-check` | Format Rust / check formatting. |
| `mise run clippy` | Run Rust lint checks. |

```text
src/          Rust application, canvas layout, state, and tools
extension/    Extension manifest, HTML, JavaScript bridge, and service worker
assets/       Bundled resources
mise.toml     Development and build tasks
DESIGN.md     UI behavior and implementation guidelines
```

`dist/` and `target/` are generated directories. Make changes in the source files and rebuild.

## Tagged releases

Pushes to `master` run checks, tests, and an optimized build to populate the mise tool cache and Cargo dependency/build caches. Tag builds can restore the default-branch caches; only tags package and publish release assets.

Pushing any tag (for example, `git push origin v0.1.0`) triggers GitHub Actions to check, test, and build the optimized extension using mise. It creates a GitHub Release with `real-useful-newtabs-<tag>.crx` and `.tgz` assets; rerunning the workflow replaces those assets in an existing release. The tarball contains the unpacked extension; extract it and load the directory through `chrome://extensions`.

To preserve the CRX extension ID between releases, set the repository Actions secret `CRX_PRIVATE_KEY` to a PEM-encoded RSA private key. Without it, each run uses a temporary key and produces a new extension ID. Signing keys are never included in release assets. The extension version comes from `extension/manifest.json`; update it before tagging a new version.

## Contributing

Bug reports, fixes, and documentation translations are welcome. Include reproduction steps and your browser version when [reporting an issue](https://github.com/turinmouse/real-useful-newtabs/issues).

Read [DESIGN.md](DESIGN.md) before changing the UI. Run `mise run check` and `mise run test` before submitting code changes, and verify UI changes in Chrome, including narrow layouts.

To add a README translation, create `README.<language-tag>.md`, preserve the same section structure, and add a language link to every version. Update both existing versions when features or setup instructions change. These language links refer to documentation; they do not indicate app UI localization.

## License

This project is licensed under the **GNU General Public License v3.0 only** (`GPL-3.0-only`). See [LICENSE](LICENSE) for the full terms.

## Acknowledgments

Built on [egui/eframe](https://github.com/emilk/egui) and [wasm-bindgen](https://github.com/wasm-bindgen/wasm-bindgen). The README's concise introduction and navigation structure are inspired by [Zen Browser](https://github.com/zen-browser/desktop).
