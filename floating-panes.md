# Floating panes

Ordinary luvus panes that float above a tab's tiled layout. Creating, moving,
or closing one never changes the tiled tree. A tab can hold many, in a
stacking order. They are not modal: keys go to the focused pane, prefix
bindings keep working. They are server state (all clients see them) and are
never written to the session file.

## Create

CLI (target defaults to the active tab; target a pane's tab with an ID):

```sh
luvus pane float [<id>] [--width N] [--height N] [--x N] [--y N] \
  [--title T] [--no-focus] [-- <cmd...>]
```

Everything after `--` is the command argv. No command runs the default shell.

API/UHP: `pane.float` with `pane`, `width`, `height`, `x`, `y`, `title`,
`focus`, `command` (argv array). Responds with `type`, `pane`, `workspace`,
`tab` (1-based). Validation runs before any spawn: bad geometry or a
non-pane tab fails with `invalid_request` and creates nothing. Git, ORCH, and
Mission tabs reject floating panes.

## Geometry

Outer rect (border included) in cells, relative to the tab's pane area.
Default: half the tab width by a quarter of the tab height. Missing x/y
cascades from (4, 2) in steps of (+4, +2), wrapping at the edge; the cascade
restarts at (4, 2) once the tab has no floating panes left. Panes clamp
inside the pane area (shrink, then move left/up). Minimum is 3x3. Width and
height below 3 fail. A pane narrower than 8 columns shows no title. Each
client clamps for its own drawing; a passive or smaller client never changes
the stored rect or PTY size.

## Title

`--title T` sets the top-border title (max 64 chars). Without it, the pane
shows the same title a tiled pane with that command would show.

## Focus, stacking, close

A new floating pane draws on top and takes focus unless `--no-focus` /
`focus: false`. Focusing one raises it to the top. Focusing one in another
tab switches to that tab. All pane commands (focus, run, send, read, status,
processes, name, close) accept floating IDs. When its command exits, the pane
closes; focus returns to the previously focused pane, else the most recent
live pane in history, else the tab's tiled focus. A floating pane never keeps
a tab alive: closing the last tiled pane closes the tab and its floating
panes.

Mouse: the topmost pane under the pointer gets clicks and scrolls; covered
tiled cells get nothing. A click on a floating pane focuses it. Directional
focus moves between tiled panes only; from a floating pane it starts at the
tab's last focused tiled pane.

Zoom shows only the zoomed pane and hides the rest. Creating a floating pane,
or a focus change to/from one, unzooms the tab. Closing a zoomed floating
pane keeps zoom.

## Tiled-only commands

Split, transpose, swap, move, and resize reject floating targets with
`invalid_request` ("pane is floating"), including when the focused pane is
the implicit target. Prefix bindings for these show a notice and do nothing.
`pane.neighbor` / `pane.edges` answer for the tab's tiled focus.
`pane.layout` for a floating pane returns `floating: true`, its `rect`, and
the tiled tree. `layout.apply` accepts tiled panes only and rejects floating
IDs. Note: `layout.export` can report a floating focus that `layout.apply`
rejects, so a naive export-then-apply round trip can fail.

## Listing and agent surfaces

`pane.list`, `pane.get`, `pane.current`, `pane.status`, `session.snapshot`,
agent list, agent kind lookup, terminal inventory, Mission Control,
orchestration scans, dock targets, and the commander picker all include
floating panes. Each carries `floating: true`, its `rect`, and `z` (0 is the
top). Orchestration never splits a new pane from a floating pane. Luvus Web
omits floating panes.

## Persistence

The saved session file never contains floating panes. After a restart the
tab returns with tiled panes only, focused on the last focused tiled pane.

## Modules

```toml
[[panes]]
placement = "floating"
width = 60
height = 8
# x = 4
# y = 2
```

Same defaults and validation as other floating panes; the entry title shows
in the border. Prompt pattern: open a floating pane that takes focus, closes
when its command exits, and returns focus. `placement = "overlay"` is
unchanged (split plus zoom). An explicit `--placement` flag always wins; a
manifest value other than `floating` without the flag is ignored as before.

## Out of scope

No percent sizes, named positions, move/resize/z-order commands, floating an
existing tiled pane, mouse move/resize, prefix binding to create one,
keep-open after exit, or modal popups.
