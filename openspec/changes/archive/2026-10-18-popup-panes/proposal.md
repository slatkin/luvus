# Proposal

## Why

Module panes and CLI users cannot open a floating pane in luvus. `placement = "overlay"` is a split plus `zoomed = true` (`src/app/modules.rs`). `zoomed` is one global flag, and closing the pane does not clear it. A module that needs a short panel, such as an input prompt, gets a plain split. tmux 3.8 has floating panes (`new-pane`). See epic #1.

tmux 3.8 is the design model for every decision in this change. When this proposal or a later artifact does not state a behavior, use the tmux 3.8 behavior.

## What Changes

- Add **floating panes**. A floating pane is an ordinary luvus pane that belongs to a tab. It draws over the tab's tiled layout and does not change that layout.
- A tab can have more than one floating pane. Floating panes have a stacking order. A new floating pane goes on top.
- A floating pane has a width and a height in cells, a position in cells, and a title. The default size is half the tab width and a quarter of the tab height. The default position cascades from the top-left of the tab, as in tmux.
- A floating pane is not modal. Keyboard input goes to the focused pane, and the prefix and its bindings work as usual. A mouse click on a floating pane focuses it. A click on a visible part of a tiled pane goes to that tiled pane.
- A new floating pane takes focus, unless the request asks not to change focus (tmux `-d`).
- A floating pane closes when its command exits, the same as a tiled pane. Closing it returns focus to the pane that had focus before.
- The existing pane commands (focus, run, send, read, status, name, close) work on floating panes.
- Add an API method and a CLI command to create a floating pane with a command, a size, a position and a title.
- Add `placement = "floating"` for module `[[panes]]` entries, with an optional width, height and position. A module gets a "modal" panel from this: it opens a floating pane that takes focus, and the pane closes when its command exits.
- Floating panes are server state, so every attached client sees the same floating panes in a tab.
- A floating pane is not saved in the session and does not return after a server restart.
- `placement = "overlay"` keeps its current behavior. This change does not alter it.

Out of scope for this change (epic tier 2): percent sizes, named positions, move and resize commands, z-order commands, float an existing tiled pane and return it to the layout (tmux `break-pane -W` and `join-pane`), mouse move and resize, a prefix binding to create a floating pane (tmux `*`), floating panes that stay visible above a zoomed pane (tmux `-A`), border styles, working directory and environment options, keep-open after exit (tmux `-k`). Modal panes (tmux `-O`) and client popups (tmux `display-popup`) are not planned.

## Capabilities

### New Capabilities

- `floating-panes`: create, draw, stacking order, focus, input and mouse routing, close and lifecycle of floating panes in a tab.

### Modified Capabilities

None. The repository has no existing OpenSpec specs. Module pane placement gains one value. The `floating-panes` spec will cover it.

## Impact

- `src/app/` (floating pane state per tab, focus and focus return, input and mouse routing, close path, session persist exclusion).
- `src/ui/` (draw floating panes over the tiled layout in stacking order, hit-test geometry, resize their PTYs).
- `src/app/modules.rs` and the module manifest (new `floating` placement).
- `src/api/`, `src/app/dispatch.rs` and `src/cli.rs` (new create entry; existing pane entries accept floating pane IDs).
- `website/src/content/docs/`, `skills/luvus-module/`, and the i18n strings that name placements.
- Main risk: `src/app/input.rs` (about 11k lines) and `src/layout.rs`. Many paths assume that every pane is a leaf in the tiled layout.
- Open questions for the design: where floating panes live (beside the tab's `TileLayout` or inside it), how a tiled-layout zoom interacts with floating panes, whether focus raises a floating pane to the top (verify against tmux 3.8), and how the API and CLI name the create entry.
