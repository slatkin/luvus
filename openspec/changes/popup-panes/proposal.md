# Proposal

## Why

Module panes and CLI users cannot open a floating, modal panel in luvus. `placement = "overlay"` is a split plus `zoomed = true` (`src/app/modules.rs`). `zoomed` is one global flag, and closing the pane does not clear it. A short modal panel, such as an input prompt, ends up as a plain split. tmux has `display-popup`, and herdr has `type = "popup"` key commands. See epic #1.

## What Changes

- Add a **popup**: one pane that floats over the tiled layout, does not change the layout, and takes all input until its command exits.
- The popup is session-global. At most one popup is open at a time. Per-client popups are out of scope (epic tier 3).
- A popup has a width and a height (cells), a title, and a close mode (close when the command exits).
- Add an API method and a CLI command to open a popup and to close it.
- Add `placement = "popup"` for module `[[panes]]` entries, with optional width and height.
- Closing the popup returns focus to the pane that had it before.
- A popup is not saved in the session and does not return after a server restart.
- `placement = "overlay"` keeps its current behavior. This change does not alter it.

Out of scope for this change (epic tiers 2 and 3): percent sizes, position codes, working directory and environment options, border styles, key-binding entries, mouse move and resize, per-client popups.

## Capabilities

### New Capabilities

- `popup-panes`: opening, drawing, input handling, closing and lifecycle of a floating popup pane.

### Modified Capabilities

None. The repository has no existing OpenSpec specs. Module pane placement gains one value. The `popup-panes` spec will cover it.

## Impact

- `src/app/` (new popup state, input and mouse routing, close and focus return, session persist exclusion).
- `src/ui/` (draw the popup over the panes, resize its PTY).
- `src/app/modules.rs` and the module manifest (new `popup` placement).
- `src/api/` and `src/cli.rs` (new open and close entries).
- `MODULE-GUIDE.md`, `website/src/content/docs/`, `skills/luvus-module/`, and the i18n strings that name placements.
- Main risk: `src/app/input.rs` (about 11k lines). Many paths assume that the focused pane is in the layout.
- Open questions for the design: how input and mouse events reach a pane that is outside the layout, and whether the server-owned popup rectangle in `src/ipc/protocol.rs` can carry the popup.
