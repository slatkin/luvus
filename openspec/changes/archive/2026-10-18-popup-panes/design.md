# Design

## Context

See `proposal.md` (Why) for the motivation and `specs/floating-panes/spec.md` for the required behavior. tmux 3.8 is the model. The tmux facts this design relies on come from the tmux 3.8 man page and source (`window.c`, `layout.c`, `cmd-split-window.c`):

- `new-pane` defaults: width `w->sx / 2`, height `w->sy / 4`; cascade from x 4, y 2 in steps of 4 and 2, wrapping when the pane would pass the edge, and resetting when the window has no floating panes (`layout_floating_args_parse`).
- An explicit `-x`/`-y` includes the border (`sx -= 2`).
- Making a floating pane active moves it to the front of `w->z_index` (`window_set_active_pane`).
- A floating pane can be zoomed. Floating panes are hidden under zoom unless `-A`. Creating a floating pane unzooms the window first (`window_push_zoom`).
- When the active pane goes away, the previous entry on the window's `w->last_panes` becomes active (`window_lost_pane`).
- On a window resize, floating panes shrink and move to stay inside (`layout_clamp_floating_panes`).
- Floating cells cannot be split in the tiled sense (`layout_split_pane` rejects them).

Current luvus state that shapes the approach:

- Each `Tab` owns one `TileLayout { root: Node, focus: PaneId }` (`src/layout.rs`, module doc "Panes are leaves"). `focus` is read on about 630 lines (`rg 'layout\(\)\.focus|layout\.focus' src/`) as "the focused pane" for key forwarding, cursor, status and API defaults.
- `TileLayout::leaves()` walks the tree. Its callers split into two groups:
  - **Tree callers**, which must stay tree-only: `apply_tree` (`src/layout.rs:240`), persistence (`src/persist.rs:922`) and the terminal-damage fast path (`src/ui/panes.rs:369`).
  - **Enumeration callers**, which must also see floating panes: `api_pane_list` (`src/app/dispatch/topology.rs:94`), `focus_pane_global` (`src/app/mod.rs:9063`), the socket tab pane list (`src/app/dispatch/topology.rs:1737`), the sidebar (`src/ui/sidebar.rs:25`), the switcher (`src/app/switcher.rs:99`) and search (`src/app/search.rs:280`).
- `App::pane_location` finds a pane's tab with `tab.layout.contains(id)` (`src/app/backend.rs`). `close_pane` removes focus-history entries (`src/app/mod.rs:9638`) and calls `layout.remove(id)`. `remove` moves focus to `first_leaf` (`src/layout.rs:180`). No history walk restores focus today.
- Tabs close through `close_empty_tab`/`close_active_tab` (`src/app/mod.rs:9709`) and the dashboard paths `close_git_tab`, `close_orch_board` and `close_mission_tab`.
- `layout.apply` (`api_layout_apply`, `src/app/dispatch/topology.rs:1263`) defaults focus to `layout.focus` and calls `apply_tree`, which requires the tree to name every current leaf and the focus.
- Rendering builds `pane_rects: Vec<(PaneId, Rect)>` in `render_into_mode` (`src/ui/mod.rs`). `bordered = rects.len() > 1` (`src/ui/mod.rs:776`). Mouse hit tests use the first hit in `pane_rects` (`src/app/input.rs`). Only an interactive render (`resize_panes == true`) resizes PTYs. `render_into_mode` resets `last_pane_area` to `Rect::ZERO` (`src/ui/mod.rs:638`). Headless callers use `split_area()`, which falls back to `api::topology::logical_area()` (`src/app/mod.rs:5568`).
- `zoomed` is one `App` flag, and a focus change does not clear it today. Zoom draws `layout().focus` over the whole pane area.
- Shell panes spawn through `spawn_into_deferred(cwd, fallbacks)` (`src/app/mod.rs:5446`), which takes no argv. Command panes use `Pane::spawn_command` (`src/terminal/pty.rs:272`), and `module_open_pane` spawns before its placement match (`src/app/modules.rs:583-614`).
- `socket_pane_layout` returns `pane_rect` and `to_tree()` (`src/app/dispatch/topology.rs:1768`).
- `MODULE-GUIDE.md` does not document placements. Placement docs live in `website/src/content/docs/docs/extend/writing-modules.mdx` and `skills/luvus-module/SKILL.md`.

## Goals / Non-Goals

**Goals:**
- Keep `layout().focus` meaning "the focused pane of this tab", so most focus readers need no change.
- Keep `TileLayout` pure geometry: a BSP tree plus floating IDs and rects, with no UI text and no creation policy.
- One z-ordered geometry list per tab that render, hit tests, clamp and listing all read.

**Non-Goals:**
- No change to tiled-to-tiled zoom behavior. Only focus changes that involve a floating pane unzoom (spec: Zoom with floating panes).
- No per-client floating geometry. Clients clamp for drawing only.
- No border style option, no floating-over-zoom, no move or resize commands (tier 2, see proposal).

## Decisions

### D1. Geometry in `TileLayout`, policy and text in `Tab`

`TileLayout` gains:
- `floating: Vec<(PaneId, Rect)>`: index 0 is the bottom and the last entry is the top. The rect is the stored outer rect, relative to the pane area.
- `tiled_focus: PaneId`: always a tree leaf, the last focused tiled pane.

`focus` may now be a tree leaf **or** a floating ID. The module doc changes to "Tiled panes are leaves; floating panes are IDs with rects above the tree".

`Tab` gains:
- `floating_titles: HashMap<PaneId, String>`: the title overrides.
- `last_cascade: Option<(u16, u16)>`: the cascade cursor.

These are UI and creation policy, not geometry, so they stay out of `layout.rs`.

**Enumeration contract:**
- `leaves()` stays tree-only, so the tree callers keep working unchanged.
- A new `all_panes()` returns the tree leaves in their current order, then the floating panes from top to bottom.
- `contains()` and `len()` cover both parts.
- `remove()` removes from either part.

Every enumeration caller listed in Context switches to `all_panes()`. Task 2.4 lists those callers by name.

Alternatives:
- *`leaves()` includes floating panes.* This breaks `apply_tree`, persistence and the damage fast path, and each of them would need a tree-only escape.
- *A separate `Tab.floating` geometry with its own focus.* Every focus reader (about 630 lines) would need to ask "is a floating pane focused?".
- *Floating leaves inside the BSP tree (tmux's `LAYOUT_CELL_FLOATING`).* This breaks the "leaf owns a tile" invariant in every geometry walker.

### D2. Tree operations use `tiled_focus`

`split_focused`, `focus_dir`, `neighbor`, `resize_focused`, `swap_panes`, `transpose_pane` and `apply_tree` act on the tree. When `focus` is floating:
- Directional focus starts from `tiled_focus` (spec: Directional focus).
- Split, transpose, swap, move to another tab, and resize (API `pane.resize` and resize mode) return `invalid_request` ("pane is floating") from the API, or show a notice for a prefix binding (spec: Tiled-only pane commands).
- `pane.neighbor` and `pane.edges` answer for `tiled_focus`.
- `pane.layout` for a floating pane returns `floating: true`, the stored floating rect as `rect`, and `to_tree()` (spec: Pane layout for a floating pane).
- `layout.apply` defaults its focus to `tiled_focus`. A tree or focus that names a floating ID fails with `invalid_request`. On success, `apply_tree` sets `tiled_focus`, and `focus` stays on the floating pane when the request named no focus. Floating panes are not touched (spec: Apply a layout scenarios).

These rejections live in the dispatch layer and the key command layer, not in `layout.rs`.

### D3. Focus, raise and focus return

Setting focus to a floating pane moves its entry to the top of `floating`, as tmux does. Setting focus to a tiled pane updates both `focus` and `tiled_focus`. All changes go through the existing focus helpers, so focus history records floating panes like any other pane.

Focus return on close is **new** behavior. Today `close_pane` only removes history entries, and `remove` falls back to `first_leaf`. A new step in `close_pane` handles the case where the closed pane is floating and is the tab's focus: it walks `focus_history_back` from the most recent entry and takes the first live pane **in the same tab**, as tmux's per-window `last_panes` does. If there is none, it uses `tiled_focus`. Closing a tiled pane keeps the current `remove` behavior, so tiled focus does not change in this change.

When zoom is active and focus moves to or from a floating pane, the focus helper clears `zoomed` (spec: Zoom with floating panes). Tiled-to-tiled focus changes leave `zoomed` as it is today.

### D4. Geometry, defaults and clamp

The create path computes the stored rect from the request and `split_area()`. It does not use `last_pane_area`, which can be `Rect::ZERO` for a headless `pane.float`. The steps:
- Width and height default to `area.width / 2` and `area.height / 4`. The minimum is 3 by 3 outer cells, so that the border and one content cell fit.
- x and y follow the tmux cascade, using `Tab.last_cascade`. The cursor resets when `floating` becomes empty.
- The result is clamped into the area.

Clamp runs in two places:
- In the interactive render, the stored rect is clamped and stored back when the pane area changed. This is the tmux resize clamp, and only the interactive client may do it.
- In every render, a draw-only clamp is applied for that client's area.

The PTY is resized from the clamped content rect only in the interactive render (`resize_panes == true`), the same rule as tiled panes.

### D5. Render, borders, damage path and hit-test order

`render_into_mode` draws tiled panes first, then floating panes from bottom to top. Each floating pane draws a cleared rect, a border with its title, and its terminal content. The title is the `Tab.floating_titles` entry, else the normal pane title.

- **Tiled borders:** `bordered` counts tiled rects only, so a lone tiled pane under a floating pane keeps no border. Floating panes always have a border.
- **Damage fast path:** `patch_terminal_damage` compares `all_panes()` with `content_rects`. Floating content rects come after tiled ones in z order, so damage patching stays available when floating panes exist. If a floating pane overlaps a damaged tiled cell, the function returns `Err(())`, and the full renderer runs (the existing fallback).
- **Hit testing:** `pane_rects` lists floating panes first, from top to bottom, then tiled panes. The "first hit wins" lookups in `input.rs` then pick the topmost pane, and covered tiled cells get no event.

The server/client protocol does not change, because the server already draws full frames.

Under zoom, the render keeps its current path: it draws `layout().focus` over the whole area. If that pane is floating, it fills the area with its PTY resized, and its stored floating rect stays unchanged for unzoom. Other floating panes are not drawn while zoomed.

### D6. Create entry: `pane.float` and `luvus pane float`

The API gets a new method `pane.float`. The CLI gets `luvus pane float [<id>] [--width N] [--height N] [--x N] [--y N] [--title T] [--no-focus] [-- <cmd...>]`. Parameters mirror `pane.split` where they overlap (`pane`, `focus`). The new ones are `width`, `height`, `x`, `y`, `title` and `command` (an argv array). The response matches `pane.split`: `type`, `pane`, `workspace`, `tab`.

A separate method is used rather than an option on `pane.split`, because tmux keeps `new-pane` separate from `split-window`, and the split parameters (`direction`, auto axis) mean nothing for a floating pane.

Creation has two steps:
1. **Spawn.** Without a command, use `spawn_into_deferred` with the same cwd candidates as `split_pane_in`. With a command, use `Pane::spawn_command`.
2. **Attach.** `attach_floating(pane_id, workspace, tab, geometry, title, focus)` validates the target tab and geometry, clears zoom, inserts into `floating`, records the title, and sets focus.

Validation that needs no pane (target tab kind, sizes) runs before the spawn, so an invalid request spawns nothing.

### D7. Module placement `floating`

`PaneEntry` gains optional `width`, `height`, `x` and `y` (`Option<u16>`, serde default `None`). Other placements ignore them.

`module_open_pane` changes so that floating validation runs before its existing spawn. After the spawn, the `"floating"` arm calls `attach_floating` with the entry title. The `split`, `overlay` and `tab` arms do not change.

### D8. Persistence and tab close

`snapshot_layout` serializes `to_tree()` and `leaves()`. Both are tree-only (D1), so floating panes are never written. The saved focus becomes `tiled_focus`, never a floating ID, so restore always finds a leaf.

A tab that closes with floating panes must also close them, so that no PTY leaks:
- `close_empty_tab` and `close_active_tab` call `close_pane` for each floating pane first.
- `close_git_tab`, `close_orch_board` and `close_mission_tab` get a debug assertion that `floating` is empty, because those tabs reject floating panes (spec: Target is not a pane tab).

### D9. Public projection

`pane.list`, `pane.get`, `pane.current` and `pane.status` add `floating: bool` to every pane. Floating panes also get `rect: {x, y, width, height}` (the stored outer rect, relative to the pane area) and `z` (0 is the top). The UHP capability list and schema gain `pane.float`, and the pane objects gain the new fields as optional.

## Risks / Trade-offs

- [Some focus readers pass `focus` into tree geometry, such as `pane_rect(area, focus)` or `neighbor`] → Task 2.4 audits the readers that pass `focus` to a tree-geometry call or a `leaves()` consumer, and switches them to `tiled_focus` or a floating-aware helper. A debug assertion in `layout.rs` geometry calls checks that the ID is a tree leaf. These sites are a subset of the ~630 lines, and most readers only forward keys or report status.
- [`App.zoomed` is global and not cleared on close, as the proposal notes] → This change does not fix that bug. Focus return (D3) keeps the zoom render on a live pane.
- [The stored rect is clamped only by the interactive client, so a large passive client sees panes sized for the smaller interactive client] → This is the same trade-off as for tiled panes, and it matches the invariant that passive clients do not change shared geometry.
- [Some mouse paths in `input.rs` may use `layout().panes(area)` instead of `pane_rects`] → Task 3.4 lists them and routes them through `pane_rects`. Tests cover clicks on covered cells.
- [The tmux cascade step and wrap values come from reading `layout_floating_args_parse` on tmux master, not from a running tmux] → The spec pins the values, so a later difference from real tmux is a spec change, not a silent drift.

## Migration Plan

No data migration is needed. Snapshots do not change shape, and old snapshots have no floating panes. The new `PaneEntry` fields are optional with serde defaults, so existing manifests still load. Rollback is a revert. A snapshot written by the new build loads in an old build, because it contains only tiled data.
