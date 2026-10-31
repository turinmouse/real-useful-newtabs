# Design Guidelines

## Canvas Windows

All canvas tool windows use the tiling layout. Floating windows and window pinning are not
supported.

- Bookmarks, bookmark folders, Settings, and Tab Groups all participate in the
  same tiling tree.
- The Settings navigation entry and window remain available even when the window has no
  configuration items.
- Tool windows are opened from the right navigation and closed with their standard window close
  button. Opening and closing windows reflows the remaining layout.

## Tiling Layout

- The layout is a tree of horizontal and vertical split containers. Tabbed and stacked
  containers are not supported.
- Opening a window inserts it after the focused leaf in that leaf's parent split. A single root
  leaf is split horizontally, and the new window receives focus.
- Dragging a title bar to a target's left, right, top, or bottom edge moves the window into a
  corresponding split. Dropping in the center swaps the two leaves. Invalid drops are cancelled.
- Dragging a divider changes only the two adjacent child weights. Closing a window removes its
  leaf and recursively collapses empty or single-child containers.
- Tool-specific minimum sizes are honored when the canvas has enough space; Settings remains wide
  enough to show its link-opening preference on one row.
- Layout manipulation has no keyboard shortcuts. Pointer drag-and-drop and dividers are the only
  structural editing controls.
- Tiling always fills the visible canvas viewport. The canvas itself does not scroll.
- Tool windows clip oversized content to their assigned tile and scroll within it instead of
  expanding over adjacent windows or navigation.

## Persistence Boundaries

- `AppState` contains project-wide canvas metadata, browser behavior settings, and theme. Web
  builds synchronize it live between open new-tab pages.
- The selected canvas is local to each page and is never written to `AppState`; it is saved with the shell layout so newly opened pages restore the last selection.
- Window visibility, tiling structure and ratios, folder-window identity, and the selected canvas
  belong to the project layout. Layout changes are saved but are not applied live to other open
  pages.
- Panel sizes, text-edit state, and tree expansion are superficial UI state owned and persisted by
  egui `Memory`.
- Settings windows have independent visibility and tiling placement on every canvas.
- Link-opening behavior is global: ordinary navigation reuses the current browser tab, while
  Ctrl/Cmd-click opens a new tab whose activation follows the Settings preference.
- Saved tab groups and their page mappings use extension-local storage and remain available after
  Chrome removes an empty live group. The Chrome group name is the persistent unique identity;
  live numeric Chrome group IDs are only session bindings.

## New Tab Position

- Settings offers “Keep new tab pages on the far right”, disabled by default and shared
  live across pages. Enabling it immediately reconciles normal, non-incognito windows.
- Unpinned new-tab pages form a stable block at the right of each window. They are removed
  from tab groups when necessary. Browser-pinned pages remain untouched. Navigating away
  from the new-tab page releases its position; disabling the setting stops repositioning.
- Browser tab and group events restore the order asynchronously; Chrome does not offer a
  native right-side position lock.

## New Tab Reuse

- Settings offers “Reuse existing new tab page”, disabled by default and shared live.
- Newly created new-tab pages reuse the most recently accessed existing new-tab page in
  the same normal, non-incognito window. The existing page is activated before the new
  page is closed. Pinned existing new-tab pages can be reused.
- With no existing candidate the new page stays open. Existing duplicates are not closed
  when enabling the setting, and navigating or reloading an existing tab does not trigger reuse.
- This applies to Ctrl/Cmd+T and the + button alike. Chrome creates the new tab before
  the extension can react, so a brief flash is possible.

## Tab Group Enhancement

- The Tab Groups tool remains available in the right navigation. Its group tree starts collapsed
  and is read-only until Tab group enhancement is enabled in Settings.
- The Chrome tab context menu exposes `Tab group enhancement` with `Pin domain` and `Pin full path`
  children while the enhancement is enabled. Full path records a page mapping; Domain adds the
  custom rule `*://<host>/*`, preserving a non-default port while matching either HTTP or HTTPS.
  Pinning an existing mapping in the current group has no effect.
- Pin actions require the tab to already be inside a non-shared group. Only HTTP and HTTPS pages in
  normal, non-incognito windows are supported, and browser-pinned tabs are excluded.
- Moving a tab into a group, navigating within a grouped tab, creating a grouped tab,
  extension-driven restoration, enabling the enhancement, startup reconciliation, and opening the
  Tab Groups tool do not create mappings. Ineligible pin actions have no effect. Mappings can be
  edited or deleted only from the Tab Groups tool.
- The context-menu item is registered only while Tab group enhancement is enabled. Chrome versions
  without tab context-menu support continue without the item.
- Page ownership always uses full-path matching, including scheme, host, port, and pathname while
  excluding query parameters and fragments. There is no global URL matching setting.
- Saved groups may also contain custom full-URL glob rules. Rules support `*` for any number of
  characters, `?` for one character, and backslash escaping. Explicitly pinned page ownership
  always wins over custom rules; otherwise the most recently added or edited matching rule wins.
- Only non-empty Chrome group names are persistent identities. Unnamed live groups are ignored by
  saved page mappings and custom rules until they receive a name.
- Custom rules are listed before recorded pages inside a group. Each rule uses the same full-width
  tree-row presentation as a recorded page, prefixed with `RULE:`. Rules are added from the group
  context menu and edited or deleted from the rule row context menu.
- The Tab Groups window includes an address search field. While searching, groups without matching
  records are hidden and matching groups expand to show only matching custom rules and page URLs,
  with rules before pages. URL and rule text matching is case-insensitive substring matching; a
  complete address also matches custom rules through their wildcard semantics. Page titles and
  group names are not searched, and clearing the field restores the normal group tree state.
- Opening a recorded page restores it to its saved live group. If that group is in another window,
  the tab moves to that window; an active moved tab also focuses the destination window. If no
  live group remains, the extension recreates it with the saved title and color.
- Moving a tab out of a group removes its current URL mapping. Closing a tab inside a group keeps
  the cached mapping. Deleting a saved group ungroups its live tabs without closing them.
- Deleting a recorded page removes every full-path-equivalent cached URL and ungroups equivalent
  live tabs without closing them. Custom rules are not removed.

## Approved egui Exceptions

- Bookmark and saved tab-group page rows may use the shared custom tree-row adapter for
  full-row interaction, favicon composition, indentation guides, and bookmark drag-and-drop
  because egui has no equivalent tree widget.
- Tiling may use the shared canvas adapter for split-tree geometry, divider interaction,
  title-bar drag-and-drop, and drop previews because egui does not provide an i3-style tiling
  container. Tool contents and all controls remain egui widgets.

## Bookmark Reordering

- Bookmark rows remain directly clickable during normal browsing; there is no separate move mode.
- Bookmark rows use egui's standard drag gesture and pointer-following drag preview.
- Reordering is disabled while bookmark search results are shown because the filtered tree does not preserve sibling positions.
