# Tasks

## 1. Layout model (`src/layout.rs`)

- [x] 1.1 Add `floating: Vec<(PaneId, Rect)>` (bottom to top) and `tiled_focus` to `TileLayout`. `new`, `from_tree` and `apply_tree` set `tiled_focus` to the tree focus. Update the module doc to say that tiled panes are leaves and floating panes are IDs with rects above the tree. Verify with unit tests that a new layout has no floating panes and `focus == tiled_focus`.
- [x] 1.2 Add `add_floating`, `raise`, `floating_rects` (top first), `is_floating` and `all_panes` (tree leaves, then floating panes top first). Make `contains`, `len` and `remove` cover floating panes, and keep `leaves()` tree-only. Verify with unit tests: add three, raise the bottom one, remove the top one, check order and `contains`, and check that `leaves()` never returns a floating ID.
- [x] 1.3 Make `remove` of the focused floating pane leave `focus` on `tiled_focus`. Make removal of the last tree leaf report the tab as empty, even when floating panes remain. Verify with unit tests for both cases.
- [x] 1.4 Add the tmux default-size (half by a quarter), cascade (start 4/2, step 4/2, wrap, cursor passed in and returned) and clamp (min 3×3, shrink then move inside) helpers as pure functions. Verify with unit tests for the spec scenarios "Default size", "Cascade of three panes", "Cascade restarts", "Clamp on create" and "Clamp on terminal resize".
- [x] 1.5 Make `focus_dir`, `neighbor`, `resize_focused`, `split_focused` and `transpose_pane` use `tiled_focus` when `focus` is floating. Make `apply_tree` reject a floating ID in the tree or as the focus, and keep a floating `focus` when the caller passes `tiled_focus`. Add a debug assertion that tree-geometry calls get a tree leaf. Verify with unit tests for "Move right from a floating pane", "Apply a layout while a floating pane exists" and "Apply a layout that names a floating pane" at the layout level.

## 2. App state, focus and close (`src/app/`)

- [x] 2.1 Add `floating_titles` and `last_cascade` to `Tab`. Route focus changes through the existing focus helpers: focusing a floating pane raises it, focusing a tiled pane updates `tiled_focus`, history records floating panes, and a focus change to or from a floating pane clears `zoomed`. Verify with app tests for "Focus raises", "Focus a floating pane in another tab", "Focus a floating pane while zoomed" and "Focus leaves a zoomed floating pane".
- [x] 2.2 Add `attach_floating(pane_id, workspace, tab, geometry, title, focus)` and a `pane.float` create path. The create path validates the target tab kind (it rejects Git/ORCH/Mission tabs) and the geometry before any spawn. It spawns a shell with `spawn_into_deferred` (with the cwd candidates of `split_pane_in`), or a command with `Pane::spawn_command`. It creates a workspace when none exists, uses `split_area()` for defaults, and clears `zoomed` before it attaches. Verify with app tests for "Create with a command", "Create without a command", "Create in the tab of a target pane", "No workspace exists", "Target is not a pane tab" (no PTY spawned), "No focus" and "Create while zoomed".
- [x] 2.3 In `close_pane`, when the closed pane is floating and focused, walk `focus_history_back` for the most recent live pane in the same tab, else use `tiled_focus`. Keep tiled-pane close behavior unchanged. Make `close_empty_tab` and `close_active_tab` close every floating pane of the tab through `close_pane`. Add debug assertions in `close_git_tab`, `close_orch_board` and `close_mission_tab` that the tab has no floating panes. Verify with app tests for "Command exits", "Previous pane is gone", "Close command" and "Last tiled pane closes" (the floating PTY is gone from `App.panes`), and check that the existing tiled close tests still pass.
- [x] 2.4 Switch the enumeration callers to `all_panes()`: `api_pane_list` (`src/app/dispatch/topology.rs:94`), `focus_pane_global` (`src/app/mod.rs:9063`), the socket tab pane list (`src/app/dispatch/topology.rs:1737`), `src/ui/sidebar.rs:25`, `src/app/switcher.rs:99` and `src/app/search.rs:280`. Then audit the `layout().focus` / `layout.focus` readers that pass the ID to tree geometry (`pane_rect`, `neighbor`, `split_*`, resize mode, swap, transpose) or to a `leaves()` consumer, and switch them to `tiled_focus` or a floating-aware helper. Leave `persist.rs`, `apply_tree` and `patch_terminal_damage`'s tree checks on `leaves()`. Verify with `cargo test --locked` for `src/app`, a sidebar/switcher test that shows a floating pane, and a list of the changed sites in the PR description.
- [x] 2.5 Make the tiled-only key commands (split, transpose, swap, move to tab, resize mode) show a localized notice and do nothing when a floating pane is focused. Add the notice text for every registered language in `src/i18n/`. Verify with an app test for "Prefix split while floating pane focused" and the i18n completeness test.

## 3. Render and mouse (`src/ui/`, `src/app/input.rs`)

- [x] 3.1 Draw floating panes after tiled panes, from bottom to top, with a cleared rect, a border and a title (the `Tab.floating_titles` entry, else the normal pane title). Compute `bordered` from tiled rects only. Build `pane_rects` with floating panes first, from top to bottom. Verify with render tests for "New pane on top", "Explicit title", "Default title", and a lone tiled pane under a floating pane that stays borderless.
- [x] 3.2 In the interactive render, clamp and store floating rects when the pane area changed, and resize floating PTYs from the content rect. In passive renders, clamp for drawing only. Verify with render tests for "Clamp on terminal resize" and "Smaller passive client" (the stored rect and PTY size do not change).
- [x] 3.3 Under zoom, draw only `layout().focus` (tiled or floating) and hide the other floating panes. Unzoom restores the stored floating rect. Verify with render tests for "Zoom a tiled pane" and "Zoom a floating pane".
- [x] 3.4 Route every mouse path in `input.rs` through `pane_rects` or a floating-aware hit helper, so that the topmost pane gets clicks and scrolls and covered tiled cells get nothing. Verify with input tests for "Click on a floating pane", "Click on a visible tiled area" and "Scroll goes to the top pane".
- [x] 3.5 Make `patch_terminal_damage` (`src/ui/panes.rs:369`) compare `all_panes()` with `content_rects`, and return `Err(())` when a floating pane overlaps a damaged tiled cell. Verify with a damage-path test that patches a floating pane's output, and a test that falls back to the full render for an overlapped tiled pane.
- [x] 3.6 Verify "Tab switch with floating panes", "Keys go to the focused tiled pane" and "Prefix while floating pane focused" with app tests.

## 4. API, CLI and UHP

- [x] 4.1 Add `pane.float` to dispatch. It validates `pane`, `width`, `height`, `x`, `y` (integers, size ≥ 3), `title`, `focus` and `command` (an argv array) before any mutation or spawn, and responds with `type`, `pane`, `workspace` and `tab`. Verify with dispatch tests for "Explicit size and position" and "Invalid size", including the 1-based tab in the response.
- [x] 4.2 Make `pane.split`, `pane.transpose`, `pane.swap`, `pane.move` and `pane.resize` return `invalid_request` ("pane is floating") for a floating target or a floating implicit focus. Make `pane.neighbor` and `pane.edges` answer for `tiled_focus`. Make `pane.layout` for a floating pane return `floating: true`, its floating rect and the tiled tree. Make `layout.apply` default its focus to `tiled_focus` and reject floating IDs. Verify with dispatch tests for "Split a floating pane", each other rejected method, "Pane layout for a floating pane", "Apply a layout while a floating pane exists" and "Apply a layout that names a floating pane".
- [x] 4.3 Add `floating`, and for floating panes `rect` and `z`, to `pane.list`, `pane.get`, `pane.current` and `pane.status`. Verify with a dispatch test for "List a tab with a floating pane".
- [x] 4.4 Add `luvus pane float [<id>] [--width N] [--height N] [--x N] [--y N] [--title T] [--no-focus] [-- <cmd...>]` to `src/cli.rs`, with compact help, focused `pane float --help`, and localized text in `src/i18n/cli.rs`. Verify with parser tests for every flag, the positional ID, pass-through argv after `--`, and duplicate or invalid flags.
- [x] 4.5 Add `pane.float` to `src/api/capabilities.rs` and `protocol/uhp/v1/schema/`, add the optional pane fields to the schema, and update the UHP fixtures. Verify with the UHP fixture and conformance tests.
- [x] 4.6 Document `pane float`, the new pane fields, and the `layout.apply` and `pane.layout` behavior with floating panes in `website/src/content/docs/docs/reference/cli.mdx` and `website/src/content/docs/docs/uhp/methods.mdx`. Update `skills/luvus/`, `plugins/luvus/skills/luvus/` and `website/public/agent-readme.md` for the automation workflow. Verify that `cargo run --quiet -- help pane` matches the documented usage.

## 5. Modules

- [x] 5.1 Add optional `width`, `height`, `x` and `y` to `PaneEntry` with serde defaults. In `module_open_pane`, validate floating geometry before the existing spawn, and add a `"floating"` arm after the spawn that calls `attach_floating` with the entry title. Verify with module tests for "Open a floating module pane", "Module prompt closes on exit", "Invalid manifest size" (no PTY spawned), and an old manifest without the fields that still loads. Check that the existing `split`, `overlay` and `tab` module tests still pass.
- [x] 5.2 Document `placement = "floating"` and its fields in `website/src/content/docs/docs/extend/writing-modules.mdx` and `skills/luvus-module/SKILL.md`, including the "modal" prompt pattern. Verify that the documented manifest example loads in a module test.

## 6. Persistence

- [x] 6.1 Save `tiled_focus` as the tab focus in `snapshot_layout`, and keep the snapshot on `to_tree()` and `leaves()` so that floating panes are never written. Verify with a persist test for "Restart with a floating pane open", and a test that an old snapshot loads unchanged.

## 7. Integration checks

- [x] 7.1 Run `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test --locked`, and verify that all pass.
- [x] 7.2 In an isolated dev home (`env -u LUVUS_SOCKET_PATH -u LUVUS_SESSION LUVUS_HOME="$HOME/.luvus-dev" ./target/debug/luvus --session agent-test ...`), check the behavior that only a live run shows:
  - a real floating module pane whose command exits and returns focus;
  - a server restart with a floating pane open;
  - a real second, smaller client attached.

  Record the results in the PR.
- [x] 7.3 Run `cargo build --release --locked` and verify that it succeeds.

## 8. Agent visibility

- [x] 8.1 Make the live `session.snapshot` (`runtime_snapshot`, `src/app/dispatch/core.rs`) list floating panes with `floating`, `rect` and `z`, their agent kind, status and session, and `focused` when a floating pane holds focus. Make the `agent list` and the agent kind lookup (`src/app/dispatch/agents.rs`) and the terminal backend inventory (`src/app/backend.rs`) include floating panes. Verify with tests for "Snapshot with a floating agent", "Agent list with a floating agent", "Agent kind lookup" (floating only, and one tiled plus one floating = ambiguous) and "Terminal backend inventory" (observe and control a floating terminal).
- [x] 8.2 Make Mission Control usage targets and rows (`src/app/mission.rs`) and the orchestration scans (`src/app/board.rs`: task start reuse, agent choices, workspace tab listing) include floating panes, while every pane that orchestration splits from or places a new pane next to stays a tiled pane. Verify with tests for "Mission Control" and "Orchestration choices" (a floating agent is a choice and can be reused; a new pane is never split from the floating pane).
- [ ] 8.3 Make the agents dock targets, `workspace_of_pane`, empty-session teardown and the worktree path scan (`src/app/mod.rs`) include floating panes, and make the commander picker (`src/commander/app.rs`, scoped and unscoped) list them. Verify with tests: dock targets list a floating agent, `workspace_of_pane` returns the owner of a floating pane, an emptied session drops a floating runtime, the worktree path scan includes a floating pane's directory, and the picker lists a floating pane.
- [ ] 8.4 Verify "Events for a floating pane" (created, focused, closed payloads match tiled panes), and that the persisted session file still excludes floating panes while the live snapshot, agent list and inventory include them. Document in `website/src/content/docs/docs/uhp/methods.mdx`, `skills/luvus/SKILL.md`, `plugins/luvus/skills/luvus/SKILL.md` and `website/public/agent-readme.md` that floating panes appear in `session.snapshot`, agent list and the other agent surfaces (and that Luvus Web may omit them). Keep both `SKILL.md` copies identical.

## Workflow follow-up

- Commit the revised proposal and the new artifacts, then update the proposal link in `slatkin/luvus#2`.
- Archive the change after review.
