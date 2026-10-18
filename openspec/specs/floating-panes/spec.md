# floating-panes Specification

## Purpose
Lets users, automation and modules open ordinary luvus panes that float over a tab's tiled layout, modeled on tmux 3.8 floating panes.

## Requirements

### Requirement: Create a floating pane
The system SHALL create a floating pane in the tab that holds the target pane, or in the active tab when no target is given. The pane runs the given command, or the default shell when no command is given. Creating it SHALL NOT change the tiled layout of the tab.

#### Scenario: Create with a command
- **WHEN** a client creates a floating pane with command `htop` in a tab that has two tiled panes
- **THEN** a new pane runs `htop` above the tab, and the two tiled panes keep their positions and sizes

#### Scenario: Create without a command
- **WHEN** a client creates a floating pane with no command
- **THEN** the new floating pane runs the default shell

#### Scenario: Create in the tab of a target pane
- **WHEN** a client creates a floating pane with target pane `7`, and pane `7` is in tab 2 of workspace 1
- **THEN** the new floating pane belongs to tab 2 of workspace 1, and the response names that workspace and tab

#### Scenario: No workspace exists
- **WHEN** a client creates a floating pane while the server has no workspace
- **THEN** the system creates a workspace the same way a split does, and the floating pane goes into its tab

#### Scenario: Target is not a pane tab
- **WHEN** the target tab is a Git, ORCH or Mission Control tab
- **THEN** the request fails with a structured error and no pane is created

### Requirement: Floating pane size and position
A floating pane SHALL have an outer width, outer height, x and y in cells, relative to the tab's pane area. The outer size includes the border. Width and height default to half the tab width and a quarter of the tab height.

#### Scenario: Default size
- **WHEN** a client creates a floating pane with no size in a pane area of 120 by 40 cells
- **THEN** the floating pane is 60 cells wide and 10 cells high

#### Scenario: Explicit size and position
- **WHEN** a client creates a floating pane with width 50, height 12, x 10 and y 3
- **THEN** the floating pane's outer rectangle starts at column 10, row 3 of the pane area and is 50 by 12 cells

#### Scenario: Invalid size
- **WHEN** a client gives a width or height that is not an integer, or is smaller than 3
- **THEN** the request fails with `invalid_request` before any pane is created

### Requirement: Default cascade position
When x or y is not given, the system SHALL cascade the position as tmux does. The first floating pane in a tab without floating panes goes to x 4, y 2. Each next one goes 4 cells right and 2 cells down from the last cascaded position, and a value that would put the pane past the edge restarts at 4 or 2.

#### Scenario: Cascade of three panes
- **WHEN** a client creates three floating panes with no position in a large tab that has no floating panes
- **THEN** their positions are (4, 2), (8, 4) and (12, 6)

#### Scenario: Cascade restarts after the last floating pane closes
- **WHEN** all floating panes in a tab close and a client creates a new one with no position
- **THEN** the new floating pane goes to (4, 2)

### Requirement: Floating panes stay inside the tab
The system SHALL keep each floating pane inside the tab's pane area. When the pane area becomes smaller than a floating pane, the system SHALL shrink the pane to fit, and SHALL move it left or up so that it does not go past the edge. The minimum outer size is 3 by 3 cells.

#### Scenario: Clamp on create
- **WHEN** a client creates a floating pane with width 200 in a pane area that is 120 cells wide
- **THEN** the floating pane is at most 120 cells wide and fully visible

#### Scenario: Clamp on terminal resize
- **WHEN** the client terminal shrinks so that a floating pane at x 80 with width 40 no longer fits in a 100-cell pane area
- **THEN** the floating pane moves left so its right edge is inside the pane area, and its PTY receives the new size if the size changed

### Requirement: Floating pane title
A floating pane SHALL show a title in its top border. When the create request gives a title, the system SHALL show that title. Otherwise it SHALL show the title a tiled pane with the same command would show.

#### Scenario: Explicit title
- **WHEN** a client creates a floating pane with title `Pick a branch`
- **THEN** the top border of the floating pane shows `Pick a branch`

#### Scenario: Default title
- **WHEN** a client creates a floating pane with command `htop` and no title
- **THEN** the top border shows the same title that a tiled `htop` pane shows

### Requirement: Stacking order
The floating panes of a tab SHALL have a stacking order. All floating panes draw above all tiled panes. A new floating pane goes on top. When a floating pane receives focus, it SHALL move to the top.

#### Scenario: New pane on top
- **WHEN** a tab has floating panes A and B that overlap, and a client creates floating pane C over both
- **THEN** C draws above B and A

#### Scenario: Focus raises
- **WHEN** floating pane A is below floating pane B and the user focuses A
- **THEN** A draws above B

### Requirement: Focus on create
A new floating pane SHALL take focus, unless the create request asks not to change focus.

#### Scenario: Default focus
- **WHEN** a client creates a floating pane with no focus option
- **THEN** the new floating pane is the focused pane and keyboard input goes to it

#### Scenario: No focus
- **WHEN** a client creates a floating pane with the no-focus option
- **THEN** focus stays on the pane that had it, and the new floating pane still draws on top

### Requirement: Floating panes are not modal
Keyboard input SHALL go to the focused pane, whether it is tiled or floating. The prefix key, prefix bindings and direct shortcuts SHALL work as usual while a floating pane is focused or visible.

#### Scenario: Prefix while floating pane focused
- **WHEN** a floating pane is focused and the user presses the prefix and then a binding that opens a new tab
- **THEN** luvus opens the new tab, and the floating pane stays in its own tab

#### Scenario: Keys go to the focused tiled pane
- **WHEN** a floating pane is visible, a tiled pane is focused, and the user types `ls`
- **THEN** `ls` goes to the tiled pane and not to the floating pane

### Requirement: Mouse routing with floating panes
A mouse event SHALL go to the topmost pane under the pointer. A click on a floating pane SHALL focus it. A click on a visible part of a tiled pane SHALL focus that tiled pane. Cells covered by a floating pane SHALL NOT pass mouse events to the panes below.

#### Scenario: Click on a floating pane
- **WHEN** the user clicks inside a floating pane that covers part of a tiled pane
- **THEN** the floating pane receives focus, and the tiled pane below gets no click

#### Scenario: Click on a visible tiled area
- **WHEN** the user clicks a part of a tiled pane that no floating pane covers
- **THEN** the tiled pane receives focus, and the floating panes keep their stacking order

#### Scenario: Scroll goes to the top pane
- **WHEN** the user scrolls the mouse wheel over two overlapping floating panes
- **THEN** only the topmost of the two receives the scroll

### Requirement: Directional focus with floating panes
Directional focus SHALL move between tiled panes only. When a floating pane is focused, a directional focus move SHALL start from the tab's last focused tiled pane.

#### Scenario: Move right from a floating pane
- **WHEN** a floating pane is focused, the last focused tiled pane is the left pane of two, and the user moves focus right
- **THEN** the right tiled pane receives focus

### Requirement: Close and exit
A floating pane SHALL close when its command exits, the same as a tiled pane. Closing a floating pane SHALL NOT change the tiled layout. When the focused floating pane closes, focus SHALL return to the pane that had focus before it.

#### Scenario: Command exits
- **WHEN** the command in a focused floating pane exits
- **THEN** the floating pane closes and focus returns to the pane that was focused before the floating pane took focus

#### Scenario: Previous pane is gone
- **WHEN** the focused floating pane closes and the pane focused before it has also closed
- **THEN** focus goes to the most recent live pane in focus history that is in the same tab, or to the tab's last focused tiled pane when the history has none

#### Scenario: Close command
- **WHEN** a client runs the pane close command on a floating pane
- **THEN** the floating pane closes and the tiled layout does not change

### Requirement: A floating pane does not keep a tab alive
A tab SHALL close when its last tiled pane closes, and its floating panes SHALL close with it. A floating pane SHALL NOT be the only pane in a tab.

#### Scenario: Last tiled pane closes
- **WHEN** a tab has one tiled pane and one floating pane, and the tiled pane closes
- **THEN** the tab closes and the floating pane's process ends

### Requirement: Zoom with floating panes
Zoom SHALL apply to the focused pane, tiled or floating. While a tab is zoomed, the system SHALL draw only the zoomed pane and SHALL hide the other floating panes. Creating a floating pane, or a focus change to or from a floating pane, in a zoomed tab SHALL unzoom the tab. A focus change between tiled panes keeps its current zoom behavior.

#### Scenario: Zoom a tiled pane
- **WHEN** a tiled pane is focused, a floating pane is visible, and the user toggles zoom
- **THEN** the tiled pane fills the pane area and the floating pane is hidden until unzoom

#### Scenario: Zoom a floating pane
- **WHEN** a floating pane is focused and the user toggles zoom, then toggles zoom again
- **THEN** the floating pane fills the pane area, and after the second toggle it returns to its floating size and position

#### Scenario: Create while zoomed
- **WHEN** a tab is zoomed and a client creates a floating pane
- **THEN** the tab unzooms and the new floating pane draws on top

#### Scenario: Focus a floating pane while zoomed
- **WHEN** a tiled pane is zoomed and a client focuses a floating pane in the same tab
- **THEN** the tab unzooms and the floating pane receives focus and moves to the top

#### Scenario: Focus leaves a zoomed floating pane
- **WHEN** a floating pane is zoomed and focus moves to a tiled pane by click, focus history or the focus command
- **THEN** the tab unzooms and the floating pane returns to its floating size and position

### Requirement: Tab switch with floating panes
Floating panes SHALL draw only while their tab is the visible tab. Switching away and back SHALL keep each floating pane's size, position, stacking order and process.

#### Scenario: Switch tabs and back
- **WHEN** tab 1 has a floating pane, the user switches to tab 2 and then back to tab 1
- **THEN** the floating pane is not drawn on tab 2, and on tab 1 it has the same size, position and process

### Requirement: Existing pane commands on floating panes
The pane commands focus, run, send, read, status, processes, name and close SHALL accept a floating pane ID and act on it as on a tiled pane.

#### Scenario: Send text to a floating pane
- **WHEN** a client sends text to a floating pane by its ID
- **THEN** the floating pane's PTY receives the text

#### Scenario: Focus a floating pane in another tab
- **WHEN** a client focuses a floating pane that is in another tab
- **THEN** the system switches to that tab and the floating pane receives focus and moves to the top

### Requirement: Tiled-only pane commands reject floating panes
Split, transpose, swap, move to another tab, and resize SHALL return a structured error when their target is a floating pane, and SHALL NOT change any state. When the implicit target is the focused pane and it is floating, the same error SHALL apply. Neighbor and edge queries from a floating pane SHALL answer for the tab's last focused tiled pane.

#### Scenario: Split a floating pane
- **WHEN** a client runs pane split with a floating pane as the target
- **THEN** the request fails with `invalid_request` that says the pane is floating, and no pane is created

#### Scenario: Prefix split while floating pane focused
- **WHEN** a floating pane is focused and the user presses the prefix split binding
- **THEN** no pane is created and luvus shows a notice that a floating pane cannot be split

### Requirement: Pane list shows floating panes
Pane list, get, current and status SHALL include floating panes and SHALL mark each pane as floating or tiled. For a floating pane, they SHALL include its outer rectangle and its stacking position, where 0 is the top. Layout tree results SHALL contain tiled panes only, and a layout apply SHALL accept only tiled panes.

#### Scenario: List a tab with a floating pane
- **WHEN** a client lists panes in a tab with two tiled panes and one floating pane
- **THEN** the result has three panes, the floating one has `floating: true` with its rectangle and `z: 0`, and the tiled ones have `floating: false`

#### Scenario: Pane layout for a floating pane
- **WHEN** a client asks for the pane layout of a floating pane
- **THEN** the result has `floating: true`, the floating pane's outer rectangle as `rect`, and the tab's tiled tree without the floating pane

#### Scenario: Apply a layout while a floating pane exists
- **WHEN** a tab has two tiled panes and one floating pane, and a client applies a layout tree that names only the two tiled panes and no focus
- **THEN** the apply succeeds, the floating pane keeps its geometry and stacking position, and focus does not change

#### Scenario: Apply a layout that names a floating pane
- **WHEN** a client applies a layout tree that contains a floating pane ID, or names a floating pane as its focus
- **THEN** the request fails with `invalid_request` and the layout does not change

### Requirement: Shared across clients
Floating panes SHALL be server state. Every attached client SHALL see the same floating panes in a tab, each clamped to that client's own pane area for drawing. A passive or differently sized client SHALL NOT change a floating pane's stored geometry or PTY size.

#### Scenario: Two clients attached
- **WHEN** two clients view the same tab and one client creates a floating pane
- **THEN** both clients draw the floating pane

#### Scenario: Smaller passive client
- **WHEN** a passive client with a smaller terminal views a tab with a floating pane
- **THEN** that client draws the pane clamped to its area, and the stored geometry and PTY size do not change

### Requirement: Floating panes are visible to agents
Every agent-facing surface SHALL treat a floating pane as a full pane and SHALL mark it as floating where it reports panes. A surface that picks a pane to split or to place a new pane next to SHALL use a tiled pane. Luvus Web MAY omit floating panes.

#### Scenario: Snapshot with a floating agent
- **WHEN** a tab has one tiled pane and one floating pane that runs an agent, and a client requests `session.snapshot`
- **THEN** the snapshot lists both panes, the floating one with `floating: true`, its agent kind and status, and `focused: true` when it holds focus

#### Scenario: Agent list with a floating agent
- **WHEN** a client lists agents and one agent runs in a floating pane
- **THEN** the list includes that agent with its workspace, tab and pane

#### Scenario: Agent kind lookup
- **WHEN** a client addresses an agent by kind and the only matching agent runs in a floating pane
- **THEN** the request reaches that agent, and two matches (one tiled, one floating) are reported as ambiguous

#### Scenario: Terminal backend inventory
- **WHEN** a tab has a floating pane
- **THEN** the terminal backend inventory lists its terminal, and a client can observe and control it

#### Scenario: Mission Control
- **WHEN** a floating pane runs an agent session with usage data
- **THEN** Mission Control shows a row for it and reads its usage

#### Scenario: Orchestration choices
- **WHEN** an active agent runs in a floating pane
- **THEN** it appears among the orchestration automation choices and can be reused for a task, and a new pane that orchestration places next to a tab's pane is never split from the floating pane

#### Scenario: Events for a floating pane
- **WHEN** a floating pane is created, focused or closed
- **THEN** the matching pane events fire with the same payload shape as for a tiled pane

### Requirement: Floating panes are not persisted
The persisted session snapshot (the saved file) SHALL NOT contain floating panes; the live runtime snapshot is separate and does. After a server restart, no floating pane SHALL return, and each tab SHALL restore its tiled layout and tiled focus.

#### Scenario: Restart with a floating pane open
- **WHEN** a tab has a focused floating pane and the server restarts
- **THEN** the tab returns with its tiled panes only, and focus is on the tab's last focused tiled pane

### Requirement: Module floating placement
A module `[[panes]]` entry SHALL accept `placement = "floating"` with optional integer `width`, `height`, `x` and `y`. Opening that entry SHALL create a floating pane with those values and the entry title, using the same defaults and validation as other floating panes.

#### Scenario: Open a floating module pane
- **WHEN** a module declares a pane entry with `placement = "floating"`, `width = 60`, `height = 8`, and the user opens it
- **THEN** a 60 by 8 floating pane runs the module command, shows the entry title, and takes focus

#### Scenario: Module prompt closes on exit
- **WHEN** the command of a floating module pane exits
- **THEN** the floating pane closes and focus returns to the pane that had it before

#### Scenario: Invalid manifest size
- **WHEN** a module manifest gives `width = 0` for a floating pane entry
- **THEN** opening that entry fails with a structured error and no pane is created

### Requirement: Overlay placement does not change
`placement = "overlay"` SHALL keep its current behavior.

#### Scenario: Overlay placement
- **WHEN** a module opens a pane with `placement = "overlay"`
- **THEN** the pane opens as a split with the tab zoomed, as before this change
