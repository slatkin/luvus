//! Binary space-partition tiling tree. Tiled panes are leaves; floating panes
//! are IDs with rects above the tree. See docs/04-data-model.md §4.

use std::collections::{HashMap, HashSet};

use ratatui::layout::Rect;
use serde::{Deserialize, Serialize};

use crate::ids::PaneId;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Axis {
    /// Children side by side (vertical divider).
    Col,
    /// Children stacked (horizontal divider).
    Row,
}

/// Typical monospace cell height divided by width. Terminal glyphs are about
/// twice as tall as they are wide (for example 8×16 pixels), so comparing
/// columns to rows directly treats a physically tall pane as landscape.
pub const CELL_ASPECT_HEIGHT_OVER_WIDTH: f32 = 2.0;

/// Split the longer *physical* side of a pane so the two halves stay closer to
/// square. `cell_aspect` is cell height / cell width. `1.0` treats cells as
/// square and is the headless fallback: a 10_000×10_000 logical area still
/// splits left/right. Equal physical sides keep the historical left/right default.
pub fn auto_split_axis_with_cell_aspect(width: u16, height: u16, cell_aspect: f32) -> Axis {
    let aspect = if cell_aspect.is_finite() && cell_aspect > 0.0 {
        cell_aspect
    } else {
        1.0
    };
    if (height as f32) * aspect > f32::from(width) {
        Axis::Row
    } else {
        Axis::Col
    }
}

/// Split using square cells. Prefer [`auto_split_axis_with_cell_aspect`] for
/// painted terminal geometry.
pub fn auto_split_axis(width: u16, height: u16) -> Axis {
    auto_split_axis_with_cell_aspect(width, height, 1.0)
}

enum Node {
    Leaf(PaneId),
    Split {
        axis: Axis,
        ratio: f32,
        a: Box<Node>,
        b: Box<Node>,
    },
}

pub struct PaneInfo {
    pub id: PaneId,
    pub rect: Rect,
}

/// A resizable boundary between two panes: the split node it belongs to (as a
/// root→node path of A/B turns), its axis, the cell line it sits on, and the
/// perpendicular extent it spans. Produced by [`TileLayout::dividers`].
#[derive(Clone)]
pub struct Divider {
    /// Turns from the root to the owning `Split` (`false` → child A, `true` → B).
    pub path: Vec<bool>,
    pub axis: Axis,
    /// Col: the divider column x. Row: the divider row y.
    pub line: u16,
    /// Perpendicular extent `[lo, hi)` the divider spans.
    pub span: (u16, u16),
}

/// Minimum cells a pane keeps along the split axis, so a resize can't crush it.
pub const MIN_PANE: u16 = 4;

/// Minimum outer size of a floating pane: a border plus one content cell.
pub const MIN_FLOATING: u16 = 3;

/// First cascade position for a floating pane, in pane-area cells (tmux 3.8).
const FLOATING_CASCADE_START: (u16, u16) = (4, 2);

/// Cascade step between floating panes (tmux 3.8).
const FLOATING_CASCADE_STEP: (u16, u16) = (4, 2);

/// Default outer size for a floating pane created without a size: half the
/// pane area's width and a quarter of its height. Never smaller than
/// [`MIN_FLOATING`], never larger than the area.
pub fn default_floating_size(area: Rect) -> (u16, u16) {
    let width = (area.width / 2).clamp(MIN_FLOATING.min(area.width), area.width);
    let height = (area.height / 4).clamp(MIN_FLOATING.min(area.height), area.height);
    (width, height)
}

/// Next cascaded position for a floating pane of `width` by `height` cells.
///
/// `last` is the position of the previously created floating pane, or `None`
/// when the tab has no floating panes. The first pane goes to (4, 2); each next
/// one moves 4 right and 2 down. An axis whose next value would put the pane
/// past the pane area restarts at 4 or 2. Positions are relative to the pane
/// area. The returned position is also the cursor to pass as `last` next time.
pub fn cascade_floating_position(
    area: Rect,
    width: u16,
    height: u16,
    last: Option<(u16, u16)>,
) -> (u16, u16) {
    let (start_x, start_y) = FLOATING_CASCADE_START;
    let (step_x, step_y) = FLOATING_CASCADE_STEP;
    let (mut x, mut y) = match last {
        Some((last_x, last_y)) => (last_x.saturating_add(step_x), last_y.saturating_add(step_y)),
        None => (start_x, start_y),
    };
    if x.saturating_add(width) > area.width {
        x = start_x;
    }
    if y.saturating_add(height) > area.height {
        y = start_y;
    }
    (x, y)
}

/// Whether an outer floating-pane size is at least [`MIN_FLOATING`] on both
/// axes.
pub fn floating_size_valid(width: u16, height: u16) -> bool {
    width >= MIN_FLOATING && height >= MIN_FLOATING
}

/// A resolved floating-pane placement in pane-area cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FloatGeometry {
    /// The clamped outer rect to store.
    pub rect: Rect,
    /// The requested top-left, remembered as the tab's cascade cursor.
    pub cascade: (u16, u16),
}

/// Resolve a new floating pane's outer rect from requested size and position.
///
/// Missing `width`/`height` use [`default_floating_size`]; missing `x`/`y`
/// cascade from `last` like [`cascade_floating_position`]. Returns `None` when
/// the resulting size is below [`MIN_FLOATING`]. The rect is clamped to `area`.
pub fn resolve_floating_geometry(
    area: Rect,
    width: Option<u16>,
    height: Option<u16>,
    x: Option<u16>,
    y: Option<u16>,
    last: Option<(u16, u16)>,
) -> Option<FloatGeometry> {
    let (default_width, default_height) = default_floating_size(area);
    let width = width.unwrap_or(default_width);
    let height = height.unwrap_or(default_height);
    if !floating_size_valid(width, height) {
        return None;
    }
    let cascade =
        (x.is_none() || y.is_none()).then(|| cascade_floating_position(area, width, height, last));
    let x = x.or_else(|| cascade.map(|(x, _)| x)).unwrap_or(0);
    let y = y.or_else(|| cascade.map(|(_, y)| y)).unwrap_or(0);
    Some(FloatGeometry {
        rect: clamp_floating_rect(Rect::new(x, y, width, height), area),
        cascade: (x, y),
    })
}

/// Shrink and move a floating pane rect until it fits inside the pane area.
///
/// The minimum outer size is [`MIN_FLOATING`] by [`MIN_FLOATING`], unless the
/// area itself is smaller. Rects use pane-area coordinates, so only the area's
/// extent matters here.
pub fn clamp_floating_rect(rect: Rect, area: Rect) -> Rect {
    let width = rect.width.clamp(MIN_FLOATING.min(area.width), area.width);
    let height = rect
        .height
        .clamp(MIN_FLOATING.min(area.height), area.height);
    let x = rect.x.min(area.width.saturating_sub(width));
    let y = rect.y.min(area.height.saturating_sub(height));
    Rect::new(x, y, width, height)
}

pub struct TileLayout {
    root: Node,
    /// The focused pane of the tab: a tree leaf or a floating pane.
    pub focus: PaneId,
    /// Floating panes above the tree, bottom first and top last. Rects are
    /// outer rects in pane-area cells.
    floating: Vec<(PaneId, Rect)>,
    /// The last focused tiled pane; always a tree leaf.
    pub tiled_focus: PaneId,
}

/// Serializable mirror of the tree (leaves carry the runtime pane id at save
/// time). Used by persistence; see docs/09.
#[derive(Clone, Serialize, Deserialize)]
pub enum LayoutTree {
    Leaf(u32),
    Split {
        axis: u8, // 0 = Col, 1 = Row
        ratio: f32,
        a: Box<LayoutTree>,
        b: Box<LayoutTree>,
    },
}

impl TileLayout {
    pub fn new(root: PaneId) -> Self {
        TileLayout {
            root: Node::Leaf(root),
            focus: root,
            floating: Vec::new(),
            tiled_focus: root,
        }
    }

    pub fn to_tree(&self) -> LayoutTree {
        node_to_tree(&self.root)
    }

    /// Rebuild a layout from a saved tree, mapping old raw ids to freshly
    /// allocated panes. Returns `None` if no leaf survives.
    pub fn from_tree(
        tree: &LayoutTree,
        remap: &HashMap<u32, PaneId>,
        focus_raw: u32,
    ) -> Option<Self> {
        let root = build_node(tree, remap)?;
        let focus = remap
            .get(&focus_raw)
            .copied()
            .unwrap_or_else(|| first_leaf(&root));
        Some(TileLayout {
            root,
            focus,
            floating: Vec::new(),
            tiled_focus: focus,
        })
    }

    /// Number of panes in the tab, tiled and floating.
    pub fn len(&self) -> usize {
        count(&self.root) + self.floating.len()
    }

    /// Zero-based visual position of a pane without allocating the full leaf
    /// list. Mobile chrome uses this on every rendered frame.
    pub fn leaf_position(&self, id: PaneId) -> Option<usize> {
        let mut next = 0;
        find_leaf_position(&self.root, id, &mut next)
    }

    /// Tiled leaves in left-to-right / top-to-bottom order.
    ///
    /// Floating panes are deliberately excluded: tree operations, persistence
    /// and the damage fast path must never see them.
    pub fn leaves(&self) -> Vec<PaneId> {
        let mut v = Vec::new();
        collect_leaves(&self.root, &mut v);
        v
    }

    /// Every pane in the tab: tiled leaves, then floating panes top first.
    pub fn all_panes(&self) -> Vec<PaneId> {
        let mut v = self.leaves();
        v.extend(self.floating.iter().rev().map(|(id, _)| *id));
        v
    }

    /// Stored floating pane rects, top first. Rects are relative to the tab's
    /// pane area.
    pub fn floating_rects(&self) -> Vec<(PaneId, Rect)> {
        self.floating.iter().rev().copied().collect()
    }

    /// Stored rect of one floating pane, without allocating the full list.
    /// Tiled and unknown IDs return `None`.
    pub fn floating_rect(&self, id: PaneId) -> Option<Rect> {
        self.floating
            .iter()
            .find(|(pane, _)| *pane == id)
            .map(|(_, rect)| *rect)
    }

    /// Whether `id` is a floating pane.
    pub fn is_floating(&self, id: PaneId) -> bool {
        self.floating.iter().any(|(pane, _)| *pane == id)
    }

    /// Add a floating pane above the current top one.
    pub fn add_floating(&mut self, id: PaneId, rect: Rect) {
        self.floating.push((id, rect));
    }

    /// Replace the stored rect of a floating pane without changing its stacking
    /// order. Unknown or tiled IDs are ignored.
    pub fn set_floating_rect(&mut self, id: PaneId, rect: Rect) {
        if let Some(entry) = self.floating.iter_mut().find(|(pane, _)| *pane == id) {
            entry.1 = rect;
        }
    }

    /// Move a floating pane to the top of the stack. Returns `false` when the
    /// pane is not floating.
    pub fn raise(&mut self, id: PaneId) -> bool {
        let Some(index) = self.floating.iter().position(|(pane, _)| *pane == id) else {
            return false;
        };
        let pane = self.floating.remove(index);
        self.floating.push(pane);
        true
    }

    /// Whether a pane belongs to this layout, without allocating a leaf list.
    pub fn contains(&self, id: PaneId) -> bool {
        contains_leaf(&self.root, id) || self.is_floating(id)
    }

    /// The tree leaf that a tree-only operation acts on.
    fn tree_focus(&self) -> PaneId {
        if self.is_floating(self.focus) {
            self.tiled_focus
        } else {
            self.focus
        }
    }

    /// Tiled pane geometry within `area`. Floating panes have stored rects
    /// instead, so they are excluded here.
    pub fn panes(&self, area: Rect) -> Vec<PaneInfo> {
        let mut v = Vec::new();
        collect(&self.root, area, &mut v);
        v
    }

    /// Replace the focused leaf with a split and focus the new pane.
    ///
    /// A floating focus splits the last focused tiled pane instead; the new
    /// pane takes focus from the floating pane.
    pub fn split_focused(&mut self, axis: Axis, new_id: PaneId) {
        let target = self.tree_focus();
        debug_assert!(
            contains_leaf(&self.root, target),
            "split target must be a tiled pane"
        );
        split_at(&mut self.root, target, axis, new_id);
        self.focus = new_id;
        self.tiled_focus = new_id;
    }

    /// Remove a pane from the tree or the floating stack, collapsing a split
    /// parent when needed. Returns `true` if the tree is now empty.
    ///
    /// Removing the focused floating pane returns focus to the last focused
    /// tiled pane. A tree with no leaves counts as empty even when floating
    /// panes remain, so the tab closes.
    pub fn remove(&mut self, id: PaneId) -> bool {
        if let Some(index) = self.floating.iter().position(|(pane, _)| *pane == id) {
            self.floating.remove(index);
            if self.focus == id {
                self.focus = self.tiled_focus;
            }
            return false;
        }
        let root = std::mem::replace(&mut self.root, Node::Leaf(id));
        match remove_node(root, id) {
            Some(n) => {
                self.root = n;
                if self.focus == id {
                    self.focus = first_leaf(&self.root);
                }
                if self.tiled_focus == id {
                    self.tiled_focus = if self.is_floating(self.focus) {
                        first_leaf(&self.root)
                    } else {
                        self.focus
                    };
                }
                false
            }
            None => true,
        }
    }

    /// Move focus to the nearest tiled pane in `dir` (geometric).
    ///
    /// A floating focus starts from the last focused tiled pane; the move never
    /// ends on a floating pane.
    pub fn focus_dir(&mut self, area: Rect, dir: Dir) {
        let from = self.tree_focus();
        if let Some(id) = self.find_in_direction_from(area, from, dir) {
            self.focus = id;
            self.tiled_focus = id;
        }
    }

    /// Query directional navigation without changing focus. Socket inspection
    /// uses exactly the same geometric rule as the TUI. A floating pane answers
    /// from the tab's last focused tiled pane.
    pub fn neighbor(&self, area: Rect, pane: PaneId, dir: Dir) -> Option<PaneId> {
        let from = if self.is_floating(pane) {
            self.tiled_focus
        } else {
            pane
        };
        self.find_in_direction_from(area, from, dir)
    }

    /// Tree geometry for a tiled pane. Floating panes have stored rects instead.
    pub fn pane_rect(&self, area: Rect, pane: PaneId) -> Option<Rect> {
        debug_assert!(
            contains_leaf(&self.root, pane),
            "pane_rect is tree geometry and needs a tiled pane"
        );
        self.panes(area)
            .into_iter()
            .find(|candidate| candidate.id == pane)
            .map(|candidate| candidate.rect)
    }

    /// Geometry for any pane in the tab: the stored rect for a floating pane,
    /// tree geometry for a tiled one. Callers that accept both kinds of pane
    /// should use this instead of branching on [`Self::is_floating`].
    pub fn pane_rect_any(&self, area: Rect, pane: PaneId) -> Option<Rect> {
        self.floating_rect(pane)
            .or_else(|| self.pane_rect(area, pane))
    }

    /// Swap two leaf identities without moving or recreating either PTY.
    pub fn swap_panes(&mut self, first: PaneId, second: PaneId) -> bool {
        if first == second
            || !contains_leaf(&self.root, first)
            || !contains_leaf(&self.root, second)
        {
            return false;
        }
        swap_leaf_ids(&mut self.root, first, second);
        true
    }

    /// Transpose the split immediately containing `pane`.
    ///
    /// This changes only that split's orientation, preserving its ratio and
    /// both pane identities. Descendant splits and their ratios are untouched,
    /// so transposing a pane is a local layout operation rather than a global
    /// rearrangement.
    ///
    /// A floating pane maps to the last focused tiled pane; the dispatch layer
    /// rejects floating targets before this call.
    pub fn transpose_pane(&mut self, pane: PaneId) -> bool {
        let pane = if self.is_floating(pane) {
            self.tiled_focus
        } else {
            pane
        };
        let mut path = Vec::new();
        if !path_to_leaf(&self.root, pane, &mut path) || path.is_empty() {
            return false;
        }
        // The final turn selects the leaf; its prefix identifies the split
        // directly containing it.
        path.pop();
        let Some(Node::Split { axis, .. }) = self.node_at_mut(&path) else {
            return false;
        };
        *axis = match *axis {
            Axis::Col => Axis::Row,
            Axis::Row => Axis::Col,
        };
        true
    }

    /// Replace the tree only when it contains every current pane exactly once.
    /// This makes `layout.apply` atomic and preserves all pane-owned state.
    pub fn apply_tree(&mut self, tree: &LayoutTree, focus: PaneId) -> Result<(), &'static str> {
        if self.is_floating(focus) {
            return Err("layout focus must be a tiled pane");
        }
        let current: HashSet<u32> = self.leaves().into_iter().map(|id| id.0).collect();
        let mut requested = Vec::new();
        collect_raw_leaves(tree, &mut requested);
        if requested.iter().any(|raw| self.is_floating(PaneId(*raw))) {
            return Err("layout must not contain a floating pane");
        }
        let requested_set: HashSet<u32> = requested.iter().copied().collect();
        if requested_set.len() != requested.len() {
            return Err("layout contains a duplicate pane");
        }
        if requested_set != current {
            return Err("layout must contain every pane in the tab exactly once");
        }
        if !requested_set.contains(&focus.0) {
            return Err("layout focus is not part of the tree");
        }
        self.root = build_live_node(tree, Rect::new(0, 0, 10_000, 10_000));
        debug_assert!(contains_leaf(&self.root, focus));
        self.tiled_focus = focus;
        // Applying a tree never pulls focus off a floating pane.
        if !self.is_floating(self.focus) {
            self.focus = focus;
        }
        Ok(())
    }

    /// The pane to move focus to, or `None` when there is nothing that way.
    ///
    /// Movement is geometric, and the rule that makes it feel right is that a
    /// candidate must actually **face** the current pane: it has to begin beyond
    /// the edge we are leaving, *and* its span on the perpendicular axis has to
    /// overlap ours.
    ///
    /// Comparing centres alone (what this used to do) let a pane win merely for
    /// sitting further along the axis, however far off to the side it was. With
    /// one pane on the left and two stacked on the right, Down from the top-right
    /// pane jumped back to the **left** pane: its centre was lower, and the cost
    /// function weighted along-axis distance 1000x against sideways offset, so
    /// being in a completely different column cost almost nothing.
    fn find_in_direction_from(&self, area: Rect, from: PaneId, dir: Dir) -> Option<PaneId> {
        debug_assert!(
            contains_leaf(&self.root, from),
            "directional focus starts on a tiled pane"
        );
        let panes = self.panes(area);
        let cur = panes.iter().find(|p| p.id == from)?;
        let (c_lo, c_hi) = perp_span(cur.rect, dir);

        // Ordered by (gap along the axis, widest overlap, then topmost/leftmost)
        // — nearest first, then the pane most face-to-face with us, with a
        // positional tiebreak so the choice is always deterministic.
        let mut best: Option<((i64, i64, i64), PaneId)> = None;
        // Only used when nothing overlaps at all, which a ragged layout or a
        // configured gap can produce. Without it such a move would silently do
        // nothing, which reads as a dead keypress.
        let mut fallback: Option<((i64, i64), PaneId)> = None;

        for p in &panes {
            if p.id == from {
                continue;
            }
            let gap = gap_towards(cur.rect, p.rect, dir);
            if gap < 0 {
                continue; // behind us, or straddling the edge we are leaving
            }
            let (p_lo, p_hi) = perp_span(p.rect, dir);
            let overlap = c_hi.min(p_hi) - c_lo.max(p_lo);
            if overlap > 0 {
                let key = (gap, -overlap, p_lo);
                if best.as_ref().is_none_or(|(k, _)| key < *k) {
                    best = Some((key, p.id));
                }
            } else {
                let key = (c_lo.max(p_lo) - c_hi.min(p_hi), gap);
                if fallback.as_ref().is_none_or(|(k, _)| key < *k) {
                    fallback = Some((key, p.id));
                }
            }
        }
        best.map(|(_, id)| id).or(fallback.map(|(_, id)| id))
    }

    // ── resize (docs/27) ────────────────────────────────────────────────────

    /// Every inter-pane divider within `area` (the same recursion as `panes`).
    pub fn dividers(&self, area: Rect) -> Vec<Divider> {
        let mut out = Vec::new();
        let mut path = Vec::new();
        collect_dividers(&self.root, area, &mut path, &mut out);
        out
    }

    /// The divider within `tol` cells of `(c, r)` (nearest wins), or `None`.
    pub fn divider_at(&self, area: Rect, c: u16, r: u16, tol: u16) -> Option<Divider> {
        let mut best: Option<(u16, Divider)> = None;
        for d in self.dividers(area) {
            let (on_span, dist) = match d.axis {
                Axis::Col => (
                    r >= d.span.0 && r < d.span.1,
                    (c as i32 - d.line as i32).unsigned_abs() as u16,
                ),
                Axis::Row => (
                    c >= d.span.0 && c < d.span.1,
                    (r as i32 - d.line as i32).unsigned_abs() as u16,
                ),
            };
            if on_span && dist <= tol && best.as_ref().is_none_or(|(bd, _)| dist < *bd) {
                best = Some((dist, d));
            }
        }
        best.map(|(_, d)| d)
    }

    /// The divider nearest `(c, r)` regardless of distance (the Ctrl+drag path).
    pub fn nearest_divider(&self, area: Rect, c: u16, r: u16) -> Option<Divider> {
        self.dividers(area)
            .into_iter()
            .min_by_key(|d| match d.axis {
                Axis::Col => (c as i32 - d.line as i32).unsigned_abs(),
                Axis::Row => (r as i32 - d.line as i32).unsigned_abs(),
            })
    }

    /// The rect the split node at `path` currently occupies (root walk with live
    /// ratios) — converts a mouse position into a ratio during a drag.
    pub fn node_rect(&self, area: Rect, path: &[bool]) -> Option<Rect> {
        let mut node = &self.root;
        let mut rect = area;
        for &side in path {
            match node {
                Node::Split { axis, ratio, a, b } => {
                    let (ra, rb) = split_rect(rect, *axis, *ratio);
                    if side {
                        node = b;
                        rect = rb;
                    } else {
                        node = a;
                        rect = ra;
                    }
                }
                Node::Leaf(_) => return None,
            }
        }
        Some(rect)
    }

    /// Set the ratio of the split node at `path`, clamped so both children keep
    /// at least [`MIN_PANE`] cells.
    pub fn set_ratio(&mut self, area: Rect, path: &[bool], ratio: f32) {
        let Some(rect) = self.node_rect(area, path) else {
            return;
        };
        let mut node = &mut self.root;
        for &side in path {
            match node {
                Node::Split { a, b, .. } => node = if side { b } else { a },
                Node::Leaf(_) => return,
            }
        }
        if let Node::Split { axis, ratio: r, .. } = node {
            let len = match axis {
                Axis::Col => rect.width,
                Axis::Row => rect.height,
            };
            if len == 0 {
                return;
            }
            // Cap `min` at 0.5 so a node too small for two MIN_PANE children just
            // centers (a valid clamp range) instead of panicking.
            let min = (MIN_PANE as f32 / len as f32).min(0.5);
            *r = ratio.clamp(min, 1.0 - min);
        }
    }

    /// Validated ratio mutation for UHP. Returns false when `path`
    /// does not identify a split; a failed request never mutates the tree.
    pub fn try_set_ratio(&mut self, area: Rect, path: &[bool], ratio: f32) -> bool {
        if !ratio.is_finite()
            || !matches!(self.node_at(path), Some(Node::Split { .. }))
            || self.node_rect(area, path).is_none()
        {
            return false;
        }
        self.set_ratio(area, path, ratio);
        true
    }

    /// Keyboard resize: grow the focused pane toward `dir` by `delta` cells, by
    /// nudging the nearest ancestor split whose divider is on that side. Returns
    /// whether a split moved.
    pub fn resize_focused(&mut self, area: Rect, dir: Dir, delta: i16) -> bool {
        // (axis to move, which side of that split the focus must be on, sign).
        let (want_axis, want_side, sign): (Axis, bool, f32) = match dir {
            Dir::Right => (Axis::Col, false, 1.0), // focus in child A, grow A
            Dir::Left => (Axis::Col, true, -1.0),  // focus in child B, grow B
            Dir::Down => (Axis::Row, false, 1.0),
            Dir::Up => (Axis::Row, true, -1.0),
        };
        let target = self.tree_focus();
        debug_assert!(
            contains_leaf(&self.root, target),
            "resize target must be a tiled pane"
        );
        let mut leaf_path = Vec::new();
        if !path_to_leaf(&self.root, target, &mut leaf_path) {
            return false;
        }
        // Deepest ancestor split matching (axis, side) — closest to the focus.
        for k in (0..leaf_path.len()).rev() {
            if leaf_path[k] != want_side {
                continue;
            }
            let prefix = leaf_path[..k].to_vec();
            let (axis, cur) = match self.node_at(&prefix) {
                Some(Node::Split { axis, ratio, .. }) => (*axis, *ratio),
                _ => continue,
            };
            if axis != want_axis {
                continue;
            }
            let Some(rect) = self.node_rect(area, &prefix) else {
                continue;
            };
            let len = match want_axis {
                Axis::Col => rect.width,
                Axis::Row => rect.height,
            };
            if len == 0 {
                return false;
            }
            self.set_ratio(area, &prefix, cur + sign * (delta as f32 / len as f32));
            return true;
        }
        false
    }

    /// Reset every split to an even 50/50 (the `=` action in resize mode).
    pub fn equalize(&mut self) {
        equalize_node(&mut self.root);
    }

    fn node_at(&self, path: &[bool]) -> Option<&Node> {
        let mut node = &self.root;
        for &side in path {
            match node {
                Node::Split { a, b, .. } => node = if side { b } else { a },
                Node::Leaf(_) => return None,
            }
        }
        Some(node)
    }

    fn node_at_mut(&mut self, path: &[bool]) -> Option<&mut Node> {
        let mut node = &mut self.root;
        for &side in path {
            match node {
                Node::Split { a, b, .. } => node = if side { b } else { a },
                Node::Leaf(_) => return None,
            }
        }
        Some(node)
    }
}

fn count(node: &Node) -> usize {
    match node {
        Node::Leaf(_) => 1,
        Node::Split { a, b, .. } => count(a) + count(b),
    }
}

fn collect_leaves(node: &Node, out: &mut Vec<PaneId>) {
    match node {
        Node::Leaf(id) => out.push(*id),
        Node::Split { a, b, .. } => {
            collect_leaves(a, out);
            collect_leaves(b, out);
        }
    }
}

fn swap_leaf_ids(node: &mut Node, first: PaneId, second: PaneId) {
    match node {
        Node::Leaf(id) if *id == first => *id = second,
        Node::Leaf(id) if *id == second => *id = first,
        Node::Leaf(_) => {}
        Node::Split { a, b, .. } => {
            swap_leaf_ids(a, first, second);
            swap_leaf_ids(b, first, second);
        }
    }
}

fn collect_raw_leaves(tree: &LayoutTree, out: &mut Vec<u32>) {
    match tree {
        LayoutTree::Leaf(raw) => out.push(*raw),
        LayoutTree::Split { a, b, .. } => {
            collect_raw_leaves(a, out);
            collect_raw_leaves(b, out);
        }
    }
}

fn build_live_node(tree: &LayoutTree, area: Rect) -> Node {
    match tree {
        LayoutTree::Leaf(raw) => Node::Leaf(PaneId(*raw)),
        LayoutTree::Split { axis, ratio, a, b } => {
            let axis = if *axis == 0 { Axis::Col } else { Axis::Row };
            let extent = match axis {
                Axis::Col => area.width,
                Axis::Row => area.height,
            };
            let gap = match axis {
                Axis::Col => GAP_COL.load(Ordering::Relaxed),
                Axis::Row => GAP_ROW.load(Ordering::Relaxed),
            };
            let available = extent.saturating_sub(gap);
            let min_ratio = if available >= MIN_PANE.saturating_mul(2) {
                MIN_PANE as f32 / available as f32
            } else {
                0.5
            };
            let ratio = ratio.clamp(min_ratio, 1.0 - min_ratio);
            let (a_area, b_area) = split_rect(area, axis, ratio);
            Node::Split {
                axis,
                ratio,
                a: Box::new(build_live_node(a, a_area)),
                b: Box::new(build_live_node(b, b_area)),
            }
        }
    }
}

fn contains_leaf(node: &Node, id: PaneId) -> bool {
    match node {
        Node::Leaf(pane) => *pane == id,
        Node::Split { a, b, .. } => contains_leaf(a, id) || contains_leaf(b, id),
    }
}

fn find_leaf_position(node: &Node, id: PaneId, next: &mut usize) -> Option<usize> {
    match node {
        Node::Leaf(candidate) => {
            let position = *next;
            *next += 1;
            (*candidate == id).then_some(position)
        }
        Node::Split { a, b, .. } => {
            find_leaf_position(a, id, next).or_else(|| find_leaf_position(b, id, next))
        }
    }
}

fn collect(node: &Node, area: Rect, out: &mut Vec<PaneInfo>) {
    match node {
        Node::Leaf(id) => out.push(PaneInfo {
            id: *id,
            rect: area,
        }),
        Node::Split { axis, ratio, a, b } => {
            let (ra, rb) = split_rect(area, *axis, *ratio);
            collect(a, ra, out);
            collect(b, rb, out);
        }
    }
}

/// Gap (in cells) between split children — configurable via Settings (Pane
/// Layout). Defaults: a one-column gap between left/right panes, none between
/// top/bottom. Stored as atomics so the recursive split math can read them
/// without threading config through every call.
use std::sync::atomic::{AtomicU16, Ordering};
static GAP_COL: AtomicU16 = AtomicU16::new(1);
static GAP_ROW: AtomicU16 = AtomicU16::new(0);

/// Set the inter-pane gaps (from `config.layout`). Clamped to 0..=1.
pub fn set_gaps(col: u16, row: u16) {
    GAP_COL.store(col.min(1), Ordering::Relaxed);
    GAP_ROW.store(row.min(1), Ordering::Relaxed);
}

fn split_rect(area: Rect, axis: Axis, ratio: f32) -> (Rect, Rect) {
    let gap_col = GAP_COL.load(Ordering::Relaxed);
    let gap_row = GAP_ROW.load(Ordering::Relaxed);
    match axis {
        Axis::Col => {
            let avail = area.width.saturating_sub(gap_col);
            let w1 = ((avail as f32) * ratio)
                .round()
                .clamp(1.0, (avail.saturating_sub(1)).max(1) as f32) as u16;
            let w2 = avail.saturating_sub(w1);
            (
                Rect::new(area.x, area.y, w1, area.height),
                Rect::new(area.x + w1 + gap_col, area.y, w2, area.height),
            )
        }
        Axis::Row => {
            let avail = area.height.saturating_sub(gap_row);
            let h1 = ((avail as f32) * ratio)
                .round()
                .clamp(1.0, (avail.saturating_sub(1)).max(1) as f32) as u16;
            let h2 = avail.saturating_sub(h1);
            (
                Rect::new(area.x, area.y, area.width, h1),
                Rect::new(area.x, area.y + h1 + gap_row, area.width, h2),
            )
        }
    }
}

fn split_at(node: &mut Node, target: PaneId, axis: Axis, new_id: PaneId) -> bool {
    match node {
        Node::Leaf(id) if *id == target => {
            let old = *id;
            *node = Node::Split {
                axis,
                ratio: 0.5,
                a: Box::new(Node::Leaf(old)),
                b: Box::new(Node::Leaf(new_id)),
            };
            true
        }
        Node::Leaf(_) => false,
        Node::Split { a, b, .. } => {
            split_at(a, target, axis, new_id) || split_at(b, target, axis, new_id)
        }
    }
}

fn remove_node(node: Node, id: PaneId) -> Option<Node> {
    match node {
        Node::Leaf(x) => {
            if x == id {
                None
            } else {
                Some(Node::Leaf(x))
            }
        }
        Node::Split { axis, ratio, a, b } => {
            let na = remove_node(*a, id);
            let nb = remove_node(*b, id);
            match (na, nb) {
                (Some(a), Some(b)) => Some(Node::Split {
                    axis,
                    ratio,
                    a: Box::new(a),
                    b: Box::new(b),
                }),
                // One child removed → collapse to the survivor.
                (Some(x), None) | (None, Some(x)) => Some(x),
                (None, None) => None,
            }
        }
    }
}

fn first_leaf(node: &Node) -> PaneId {
    match node {
        Node::Leaf(id) => *id,
        Node::Split { a, .. } => first_leaf(a),
    }
}

fn node_to_tree(node: &Node) -> LayoutTree {
    match node {
        Node::Leaf(id) => LayoutTree::Leaf(id.0),
        Node::Split { axis, ratio, a, b } => LayoutTree::Split {
            axis: match axis {
                Axis::Col => 0,
                Axis::Row => 1,
            },
            ratio: *ratio,
            a: Box::new(node_to_tree(a)),
            b: Box::new(node_to_tree(b)),
        },
    }
}

fn build_node(tree: &LayoutTree, remap: &HashMap<u32, PaneId>) -> Option<Node> {
    match tree {
        LayoutTree::Leaf(raw) => remap.get(raw).map(|id| Node::Leaf(*id)),
        LayoutTree::Split { axis, ratio, a, b } => {
            let na = build_node(a, remap);
            let nb = build_node(b, remap);
            match (na, nb) {
                (Some(a), Some(b)) => Some(Node::Split {
                    axis: if *axis == 0 { Axis::Col } else { Axis::Row },
                    ratio: *ratio,
                    a: Box::new(a),
                    b: Box::new(b),
                }),
                (Some(x), None) | (None, Some(x)) => Some(x),
                (None, None) => None,
            }
        }
    }
}

/// How far `other` begins beyond the edge of `cur` that we are moving off.
/// Negative means it is not in that direction at all, so it is not a candidate.
fn gap_towards(cur: Rect, other: Rect, dir: Dir) -> i64 {
    let (c, o) = (cur, other);
    match dir {
        Dir::Right => o.x as i64 - (c.x as i64 + c.width as i64),
        Dir::Left => c.x as i64 - (o.x as i64 + o.width as i64),
        Dir::Down => o.y as i64 - (c.y as i64 + c.height as i64),
        Dir::Up => c.y as i64 - (o.y as i64 + o.height as i64),
    }
}

/// A rect's extent on the axis **perpendicular** to `dir`, as `(start, end)`.
/// Moving left/right, that is its vertical span; moving up/down, horizontal.
fn perp_span(r: Rect, dir: Dir) -> (i64, i64) {
    match dir {
        Dir::Left | Dir::Right => (r.y as i64, r.y as i64 + r.height as i64),
        Dir::Up | Dir::Down => (r.x as i64, r.x as i64 + r.width as i64),
    }
}

/// Emit every split's divider (line + perpendicular span), tagging each with the
/// path taken to reach it, then recurse into both children.
fn collect_dividers(node: &Node, area: Rect, path: &mut Vec<bool>, out: &mut Vec<Divider>) {
    if let Node::Split { axis, ratio, a, b } = node {
        let (ra, rb) = split_rect(area, *axis, *ratio);
        let (line, span) = match axis {
            // The boundary sits just past child A (gap-aware, since `split_rect`
            // already subtracted the gap when sizing A).
            Axis::Col => (ra.right(), (area.y, area.bottom())),
            Axis::Row => (ra.bottom(), (area.x, area.right())),
        };
        out.push(Divider {
            path: path.clone(),
            axis: *axis,
            line,
            span,
        });
        path.push(false);
        collect_dividers(a, ra, path, out);
        path.pop();
        path.push(true);
        collect_dividers(b, rb, path, out);
        path.pop();
    }
}

/// Fill `out` with the A/B turns from the root to the leaf `target`. Returns
/// `false` (leaving `out` restored) if the leaf isn't in this subtree.
fn path_to_leaf(node: &Node, target: PaneId, out: &mut Vec<bool>) -> bool {
    match node {
        Node::Leaf(id) => *id == target,
        Node::Split { a, b, .. } => {
            out.push(false);
            if path_to_leaf(a, target, out) {
                return true;
            }
            out.pop();
            out.push(true);
            if path_to_leaf(b, target, out) {
                return true;
            }
            out.pop();
            false
        }
    }
}

fn equalize_node(node: &mut Node) {
    if let Node::Split { ratio, a, b, .. } = node {
        *ratio = 0.5;
        equalize_node(a);
        equalize_node(b);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transpose_preserves_ratio_focus_and_nested_splits() {
        let a = PaneId::alloc();
        let b = PaneId::alloc();
        let c = PaneId::alloc();
        let mut layout = TileLayout::new(a);
        assert!(!layout.transpose_pane(a));
        assert!(!layout.transpose_pane(b));
        layout.split_focused(Axis::Col, b);
        layout.split_focused(Axis::Row, c);
        layout.set_ratio(Rect::new(0, 0, 120, 60), &[], 0.6);
        layout.set_ratio(Rect::new(0, 0, 120, 60), &[true], 0.3);
        let before = serde_json::to_value(layout.to_tree()).unwrap();
        let focus = layout.focus;
        assert!(layout.transpose_pane(b));
        let mut expected = before.clone();
        expected["Split"]["b"]["Split"]["axis"] = serde_json::json!(0);
        assert_eq!(serde_json::to_value(layout.to_tree()).unwrap(), expected);
        assert_eq!(layout.focus, focus);
        assert_eq!(layout.leaves(), vec![a, b, c]);
        assert!(layout.transpose_pane(b));
        assert_eq!(serde_json::to_value(layout.to_tree()).unwrap(), before);
        assert!(layout.transpose_pane(a));
        let mut expected = before;
        expected["Split"]["axis"] = serde_json::json!(1);
        assert_eq!(serde_json::to_value(layout.to_tree()).unwrap(), expected);
        let remap = [(a.0, a), (b.0, b), (c.0, c)].into_iter().collect();
        let restored = TileLayout::from_tree(&layout.to_tree(), &remap, focus.0).unwrap();
        assert_eq!(serde_json::to_value(restored.to_tree()).unwrap(), expected);
        for area in [Rect::new(0, 0, 120, 60), Rect::new(0, 0, 1, 1)] {
            assert_eq!(restored.panes(area).len(), 3);
        }
    }

    #[test]
    fn split_and_navigate() {
        let a = PaneId::alloc();
        let mut l = TileLayout::new(a);
        assert_eq!(l.len(), 1);

        let b = PaneId::alloc();
        l.split_focused(Axis::Col, b); // a | b
        assert_eq!(l.len(), 2);
        assert!(l.contains(a));
        assert!(l.contains(b));
        assert_eq!(l.leaf_position(a), Some(0));
        assert_eq!(l.leaf_position(b), Some(1));
        assert_eq!(l.leaf_position(PaneId::alloc()), None);
        assert_eq!(l.focus, b);

        let area = Rect::new(0, 0, 80, 24);
        l.focus_dir(area, Dir::Left);
        assert_eq!(l.focus, a);
        l.focus_dir(area, Dir::Right);
        assert_eq!(l.focus, b);

        assert!(!l.remove(b)); // back to just `a`
        assert_eq!(l.len(), 1);
        assert_eq!(l.focus, a);
        assert!(!l.contains(b));
        assert!(l.remove(a)); // empty
    }

    #[test]
    fn socket_queries_and_tree_apply_preserve_live_leaf_identity() {
        let first = PaneId::alloc();
        let second = PaneId::alloc();
        let mut layout = TileLayout::new(first);
        layout.split_focused(Axis::Col, second);
        let area = Rect::new(0, 0, 100, 40);

        let focus = layout.focus;
        assert_eq!(layout.neighbor(area, second, Dir::Left), Some(first));
        assert_eq!(layout.focus, focus, "inspection never changes focus");

        assert!(layout.swap_panes(first, second));
        assert_eq!(
            layout.focus, focus,
            "swapping positions preserves focus identity"
        );

        let before = serde_json::to_value(layout.to_tree()).unwrap();
        let duplicate = LayoutTree::Split {
            axis: 0,
            ratio: 0.5,
            a: Box::new(LayoutTree::Leaf(first.0)),
            b: Box::new(LayoutTree::Leaf(first.0)),
        };
        assert!(layout.apply_tree(&duplicate, first).is_err());
        assert_eq!(
            serde_json::to_value(layout.to_tree()).unwrap(),
            before,
            "invalid replacement is atomic"
        );

        let extreme = LayoutTree::Split {
            axis: 0,
            ratio: 0.0,
            a: Box::new(LayoutTree::Leaf(first.0)),
            b: Box::new(LayoutTree::Leaf(second.0)),
        };
        layout.apply_tree(&extreme, first).unwrap();
        let LayoutTree::Split { ratio, .. } = layout.to_tree() else {
            panic!("applied split must remain a split");
        };
        assert!(ratio > 0.0 && ratio < 1.0);
        assert!(layout
            .panes(Rect::new(0, 0, 10_000, 10_000))
            .iter()
            .all(|pane| pane.rect.width >= MIN_PANE));
    }

    /// The reported bug: one pane on the left, two stacked on the right. From
    /// the top-right pane, Down jumped back to the *left* pane instead of the
    /// one directly underneath.
    #[test]
    fn navigates_to_the_pane_actually_in_that_direction() {
        let left = PaneId::alloc();
        let mut l = TileLayout::new(left);
        let top_right = PaneId::alloc();
        l.split_focused(Axis::Col, top_right); // left | top_right
        let bottom_right = PaneId::alloc();
        l.split_focused(Axis::Row, bottom_right); // right side stacked
        let area = Rect::new(0, 0, 80, 24);

        l.focus = top_right;
        l.focus_dir(area, Dir::Down);
        assert_eq!(l.focus, bottom_right, "Down from top-right goes underneath");

        l.focus_dir(area, Dir::Up);
        assert_eq!(l.focus, top_right, "and Up comes back");

        l.focus = bottom_right;
        l.focus_dir(area, Dir::Left);
        assert_eq!(
            l.focus, left,
            "Left from bottom-right reaches the left pane"
        );

        // Moving into a stacked column lands on the pane it faces, and moving
        // off the outer edge does nothing rather than jumping somewhere odd.
        l.focus = left;
        l.focus_dir(area, Dir::Right);
        assert_eq!(
            l.focus, top_right,
            "Right from the left pane enters the column"
        );
        l.focus_dir(area, Dir::Right);
        assert_eq!(l.focus, top_right, "nothing further right, so no movement");
        l.focus = left;
        l.focus_dir(area, Dir::Up);
        assert_eq!(l.focus, left, "nothing above the full-height left pane");
    }

    /// A four-pane grid: two columns, each split in two. Every move must land on
    /// the neighbour that shares an edge, never diagonally across the layout.
    #[test]
    fn grid_navigation_never_moves_diagonally() {
        let tl = PaneId::alloc();
        let mut l = TileLayout::new(tl);
        let tr = PaneId::alloc();
        l.split_focused(Axis::Col, tr); // tl | tr
        let br = PaneId::alloc();
        l.split_focused(Axis::Row, br); // right column: tr / br
        l.focus = tl;
        let bl = PaneId::alloc();
        l.split_focused(Axis::Row, bl); // left column: tl / bl
        let area = Rect::new(0, 0, 80, 24);

        for (from, dir, want, what) in [
            (tl, Dir::Right, tr, "top-left -> top-right"),
            (tl, Dir::Down, bl, "top-left -> bottom-left"),
            (tr, Dir::Left, tl, "top-right -> top-left"),
            (tr, Dir::Down, br, "top-right -> bottom-right"),
            (bl, Dir::Right, br, "bottom-left -> bottom-right"),
            (bl, Dir::Up, tl, "bottom-left -> top-left"),
            (br, Dir::Left, bl, "bottom-right -> bottom-left"),
            (br, Dir::Up, tr, "bottom-right -> top-right"),
        ] {
            l.focus = from;
            l.focus_dir(area, dir);
            assert_eq!(l.focus, want, "{what}");
        }
    }

    /// Three panes stacked in one column beside a full-height pane: stepping
    /// down must visit each in turn instead of skipping or bouncing sideways.
    #[test]
    fn stepping_through_a_tall_column_visits_every_pane() {
        let left = PaneId::alloc();
        let mut l = TileLayout::new(left);
        let r1 = PaneId::alloc();
        l.split_focused(Axis::Col, r1);
        let r2 = PaneId::alloc();
        l.split_focused(Axis::Row, r2);
        let r3 = PaneId::alloc();
        l.split_focused(Axis::Row, r3);
        let area = Rect::new(0, 0, 80, 30);

        // Top to bottom of the right column, one step at a time.
        l.focus = r1;
        let mut seen = vec![l.focus];
        for _ in 0..2 {
            l.focus_dir(area, Dir::Down);
            seen.push(l.focus);
        }
        assert_eq!(seen, vec![r1, r2, r3], "each Down goes one pane down");

        // And back up again.
        for _ in 0..2 {
            l.focus_dir(area, Dir::Up);
        }
        assert_eq!(l.focus, r1, "Up retraces the same path");
    }

    #[test]
    fn dividers_and_set_ratio() {
        let a = PaneId::alloc();
        let mut l = TileLayout::new(a);
        let b = PaneId::alloc();
        l.split_focused(Axis::Col, b); // a | b, 50/50
        let area = Rect::new(0, 0, 80, 24);

        let divs = l.dividers(area);
        assert_eq!(divs.len(), 1);
        assert_eq!(divs[0].axis, Axis::Col);
        // The divider sits at child A's right edge.
        let a_rect = l.panes(area).into_iter().find(|p| p.id == a).unwrap().rect;
        assert_eq!(divs[0].line, a_rect.right());

        // Widen A and confirm the rects follow.
        let path = divs[0].path.clone();
        let width = |l: &TileLayout, id| {
            l.panes(area)
                .into_iter()
                .find(|p| p.id == id)
                .unwrap()
                .rect
                .width
        };
        l.set_ratio(area, &path, 0.75);
        assert!(width(&l, a) > width(&l, b), "A wider after ratio 0.75");

        // Absurd ratios clamp so neither child drops below MIN_PANE.
        l.set_ratio(area, &path, 9.0);
        assert!(
            width(&l, b) >= MIN_PANE,
            "B keeps >= MIN_PANE, got {}",
            width(&l, b)
        );
        l.set_ratio(area, &path, -9.0);
        assert!(
            width(&l, a) >= MIN_PANE,
            "A keeps >= MIN_PANE, got {}",
            width(&l, a)
        );
    }

    #[test]
    fn keyboard_resize_grows_focus_then_equalizes() {
        let a = PaneId::alloc();
        let mut l = TileLayout::new(a);
        let b = PaneId::alloc();
        l.split_focused(Axis::Col, b); // focus = b (right child)
        let area = Rect::new(0, 0, 80, 24);
        let width = |l: &TileLayout, id| {
            l.panes(area)
                .into_iter()
                .find(|p| p.id == id)
                .unwrap()
                .rect
                .width
        };

        // Focus b (child B): Left grows it (divider moves left).
        let before = width(&l, b);
        assert!(l.resize_focused(area, Dir::Left, 6));
        assert!(width(&l, b) > before, "focused B grew leftward");

        // Equalize restores a near-even split.
        l.equalize();
        let (wa, wb) = (width(&l, a) as i32, width(&l, b) as i32);
        assert!((wa - wb).abs() <= 1, "equalized: {wa} vs {wb}");
    }

    #[test]
    fn resize_ratio_survives_serialization() {
        let a = PaneId::alloc();
        let mut l = TileLayout::new(a);
        let b = PaneId::alloc();
        l.split_focused(Axis::Row, b);
        let area = Rect::new(0, 0, 40, 40);
        let path = l.dividers(area)[0].path.clone();
        l.set_ratio(area, &path, 0.3);
        let want = l.panes(area).into_iter().find(|p| p.id == a).unwrap().rect;

        // Round-trip through the serializable mirror.
        let tree = l.to_tree();
        let remap: HashMap<u32, PaneId> = [(a.0, a), (b.0, b)].into_iter().collect();
        let l2 = TileLayout::from_tree(&tree, &remap, a.0).unwrap();
        let got = l2.panes(area).into_iter().find(|p| p.id == a).unwrap().rect;
        assert_eq!(want, got, "resized ratio persisted");
    }

    #[test]
    fn auto_split_axis_cuts_the_longer_side() {
        assert_eq!(auto_split_axis(120, 30), Axis::Col);
        assert_eq!(auto_split_axis(40, 80), Axis::Row);
        assert_eq!(auto_split_axis(50, 50), Axis::Col);
        assert_eq!(auto_split_axis(0, 0), Axis::Col);
        assert_eq!(auto_split_axis(1, 2), Axis::Row);
    }

    #[test]
    fn auto_split_axis_accounts_for_tall_terminal_cells() {
        let aspect = CELL_ASPECT_HEIGHT_OVER_WIDTH;
        // 60×40 cells at 8×16 px is 480×640 px, physically taller.
        assert_eq!(auto_split_axis_with_cell_aspect(60, 40, aspect), Axis::Row);
        assert_eq!(auto_split_axis(60, 40), Axis::Col);
        assert_eq!(auto_split_axis_with_cell_aspect(120, 30, aspect), Axis::Col);
        assert_eq!(auto_split_axis_with_cell_aspect(40, 80, aspect), Axis::Row);
        // Equal physical sides keep the left/right default.
        assert_eq!(auto_split_axis_with_cell_aspect(80, 40, aspect), Axis::Col);
        assert_eq!(auto_split_axis_with_cell_aspect(60, 40, 0.0), Axis::Col);
        assert_eq!(
            auto_split_axis_with_cell_aspect(60, 40, f32::NAN),
            Axis::Col
        );
    }

    fn floating_ids(layout: &TileLayout) -> Vec<PaneId> {
        layout
            .floating_rects()
            .into_iter()
            .map(|(id, _)| id)
            .collect()
    }

    #[test]
    fn new_layout_has_no_floating_panes_and_tracks_tiled_focus() {
        let a = PaneId::alloc();
        let mut layout = TileLayout::new(a);
        assert!(layout.floating_rects().is_empty());
        assert_eq!(layout.focus, a);
        assert_eq!(layout.tiled_focus, a);

        let b = PaneId::alloc();
        layout.split_focused(Axis::Col, b);
        assert_eq!(layout.tiled_focus, b, "a split focuses the new tiled pane");

        let remap = [(a.0, a), (b.0, b)].into_iter().collect();
        let restored = TileLayout::from_tree(&layout.to_tree(), &remap, a.0).unwrap();
        assert_eq!(restored.focus, a);
        assert_eq!(restored.tiled_focus, a);
        assert!(restored.floating_rects().is_empty());

        let tree = layout.to_tree();
        layout.apply_tree(&tree, a).unwrap();
        assert_eq!(layout.focus, a);
        assert_eq!(layout.tiled_focus, a);
    }

    #[test]
    fn floating_panes_stack_and_enumeration_includes_them() {
        let a = PaneId::alloc();
        let mut layout = TileLayout::new(a);
        let b = PaneId::alloc();
        layout.split_focused(Axis::Col, b);
        let (first, second, third) = (PaneId::alloc(), PaneId::alloc(), PaneId::alloc());
        for (id, x) in [(first, 0), (second, 4), (third, 8)] {
            layout.add_floating(id, Rect::new(x, 2, 20, 8));
        }

        assert_eq!(
            floating_ids(&layout),
            vec![third, second, first],
            "top first"
        );
        assert_eq!(layout.all_panes(), vec![a, b, third, second, first]);
        assert_eq!(layout.len(), 5, "len counts tiled and floating panes");
        assert!(layout.is_floating(third));
        assert!(!layout.is_floating(a));
        assert!(!layout.leaves().contains(&first), "leaves stays tree-only");

        assert!(layout.raise(first), "raising a float succeeds");
        assert_eq!(floating_ids(&layout), vec![first, third, second]);
        assert!(!layout.raise(a), "a tiled pane cannot be raised");
        assert!(!layout.remove(first), "the tree still has leaves");
        assert_eq!(floating_ids(&layout), vec![third, second]);
        assert!(!layout.contains(first));
        assert!(layout.contains(second));
        assert_eq!(layout.all_panes(), vec![a, b, third, second]);
        assert_eq!(layout.leaves(), vec![a, b]);
    }

    #[test]
    fn set_floating_rect_keeps_the_stacking_order() {
        let a = PaneId::alloc();
        let mut layout = TileLayout::new(a);
        let (bottom, top) = (PaneId::alloc(), PaneId::alloc());
        layout.add_floating(bottom, Rect::new(0, 0, 20, 8));
        layout.add_floating(top, Rect::new(4, 2, 20, 8));

        let moved = Rect::new(1, 1, 12, 6);
        layout.set_floating_rect(bottom, moved);
        assert_eq!(
            layout.floating_rects(),
            vec![(top, Rect::new(4, 2, 20, 8)), (bottom, moved)],
            "the updated pane keeps its place at the bottom of the stack"
        );

        let before = layout.floating_rects();
        layout.set_floating_rect(a, moved);
        layout.set_floating_rect(PaneId(999), moved);
        assert_eq!(
            layout.floating_rects(),
            before,
            "tiled and unknown IDs are ignored"
        );
    }

    #[test]
    fn pane_rect_any_resolves_tiled_and_floating_geometry() {
        let a = PaneId::alloc();
        let mut layout = TileLayout::new(a);
        let b = PaneId::alloc();
        layout.split_focused(Axis::Col, b);
        let floating = PaneId::alloc();
        let floating_rect = Rect::new(4, 2, 20, 8);
        layout.add_floating(floating, floating_rect);
        let area = Rect::new(0, 0, 80, 24);

        assert_eq!(layout.floating_rect(floating), Some(floating_rect));
        assert_eq!(layout.floating_rect(a), None, "tiled panes are not floats");
        assert_eq!(layout.floating_rect(PaneId(999)), None);

        let tiled = layout
            .panes(area)
            .into_iter()
            .find(|pane| pane.id == a)
            .expect("the tiled leaf stays in the tree")
            .rect;
        assert_eq!(
            layout.pane_rect_any(area, a),
            Some(tiled),
            "tiled panes keep tree geometry"
        );
        assert_eq!(
            layout.pane_rect_any(area, floating),
            Some(floating_rect),
            "floating panes keep their stored rect"
        );
        assert_ne!(
            tiled, floating_rect,
            "a floating pane never reports tree geometry"
        );
    }

    #[test]
    fn removing_the_focused_floating_pane_returns_to_tiled_focus() {
        let a = PaneId::alloc();
        let mut layout = TileLayout::new(a);
        let b = PaneId::alloc();
        layout.split_focused(Axis::Col, b);
        let area = Rect::new(0, 0, 80, 24);
        layout.focus_dir(area, Dir::Left);
        assert_eq!(layout.tiled_focus, a);

        let floating = PaneId::alloc();
        layout.add_floating(floating, Rect::new(4, 2, 20, 8));
        layout.focus = floating;

        assert!(!layout.remove(floating));
        assert_eq!(layout.focus, layout.tiled_focus);
        assert_eq!(layout.focus, a);
        assert!(!layout.contains(floating));
    }

    #[test]
    fn removing_the_last_tiled_pane_reports_the_tab_empty() {
        let a = PaneId::alloc();
        let mut layout = TileLayout::new(a);
        let floating = PaneId::alloc();
        layout.add_floating(floating, Rect::new(4, 2, 20, 8));

        assert!(
            layout.remove(a),
            "a tab with no tiled leaf is empty even with floating panes"
        );
        assert!(
            layout.is_floating(floating),
            "floating bookkeeping is left to the close path"
        );
    }

    #[test]
    fn floating_default_size_is_half_by_a_quarter() {
        assert_eq!(default_floating_size(Rect::new(0, 0, 120, 40)), (60, 10));
        assert_eq!(
            default_floating_size(Rect::new(0, 0, 4, 4)),
            (3, 3),
            "small areas still get the 3×3 minimum"
        );
    }

    #[test]
    fn floating_cascade_starts_at_four_two_and_steps() {
        let area = Rect::new(0, 0, 120, 40);
        let first = cascade_floating_position(area, 60, 10, None);
        let second = cascade_floating_position(area, 60, 10, Some(first));
        let third = cascade_floating_position(area, 60, 10, Some(second));
        assert_eq!((first, second, third), ((4, 2), (8, 4), (12, 6)));

        // With every floating pane closed the caller passes `None` again.
        assert_eq!(cascade_floating_position(area, 60, 10, None), (4, 2));

        // An axis that would pass the edge restarts, the other keeps stepping.
        let narrow = Rect::new(0, 0, 70, 20);
        let second = cascade_floating_position(narrow, 60, 10, Some((4, 2)));
        let third = cascade_floating_position(narrow, 60, 10, Some(second));
        assert_eq!(second, (8, 4));
        assert_eq!(third, (4, 6), "x wraps while y keeps cascading");
    }

    #[test]
    fn floating_rects_clamp_inside_the_area() {
        let area = Rect::new(0, 0, 120, 40);
        assert_eq!(
            clamp_floating_rect(Rect::new(4, 2, 200, 10), area),
            Rect::new(0, 2, 120, 10),
            "an oversized pane shrinks to the area and moves inside"
        );
        assert_eq!(
            clamp_floating_rect(Rect::new(80, 2, 40, 10), Rect::new(0, 0, 100, 30)),
            Rect::new(60, 2, 40, 10),
            "a pane past the right edge moves left"
        );
        assert_eq!(
            clamp_floating_rect(Rect::new(0, 0, 1, 1), area),
            Rect::new(0, 0, 3, 3),
            "the outer size has a 3×3 minimum"
        );
    }

    #[test]
    fn resolved_floating_geometry_defaults_cascades_and_clamps() {
        let area = Rect::new(0, 0, 120, 40);
        assert_eq!(
            resolve_floating_geometry(area, None, None, None, None, None),
            Some(FloatGeometry {
                rect: Rect::new(4, 2, 60, 10),
                cascade: (4, 2),
            })
        );
        assert_eq!(
            resolve_floating_geometry(area, Some(20), Some(8), Some(30), None, Some((4, 2))),
            Some(FloatGeometry {
                rect: Rect::new(30, 4, 20, 8),
                cascade: (30, 4),
            }),
            "a given x keeps its value while y cascades"
        );
        assert_eq!(
            resolve_floating_geometry(area, Some(200), Some(10), Some(80), Some(2), None),
            Some(FloatGeometry {
                rect: Rect::new(0, 2, 120, 10),
                cascade: (80, 2),
            }),
            "the rect clamps but the cascade cursor keeps the request"
        );
        assert_eq!(
            resolve_floating_geometry(area, Some(2), None, None, None, None),
            None,
            "a requested size below the minimum is rejected"
        );
    }

    #[test]
    fn directional_focus_from_a_floating_pane_starts_at_tiled_focus() {
        let left = PaneId::alloc();
        let mut layout = TileLayout::new(left);
        let right = PaneId::alloc();
        layout.split_focused(Axis::Col, right);
        let area = Rect::new(0, 0, 80, 24);
        layout.focus_dir(area, Dir::Left);
        assert_eq!(layout.tiled_focus, left);

        let floating = PaneId::alloc();
        layout.add_floating(floating, Rect::new(4, 2, 20, 8));
        layout.focus = floating;
        layout.focus_dir(area, Dir::Right);
        assert_eq!(layout.focus, right, "the move lands on a tiled pane");
        assert_eq!(layout.tiled_focus, right);
        assert_eq!(layout.neighbor(area, floating, Dir::Left), Some(left));
    }

    #[test]
    fn apply_tree_keeps_floating_panes_and_rejects_floating_ids() {
        let a = PaneId::alloc();
        let mut layout = TileLayout::new(a);
        let b = PaneId::alloc();
        layout.split_focused(Axis::Col, b);
        let area = Rect::new(0, 0, 80, 24);
        layout.focus_dir(area, Dir::Left);
        assert_eq!(layout.tiled_focus, a);
        let floating = PaneId::alloc();
        layout.add_floating(floating, Rect::new(4, 2, 20, 8));
        let before = layout.floating_rects();
        layout.focus = floating;

        let tiled_tree = LayoutTree::Split {
            axis: 1,
            ratio: 0.4,
            a: Box::new(LayoutTree::Leaf(a.0)),
            b: Box::new(LayoutTree::Leaf(b.0)),
        };
        layout.apply_tree(&tiled_tree, a).unwrap();
        assert_eq!(layout.tiled_focus, a);
        assert_eq!(layout.focus, floating, "a floating focus survives apply");
        assert_eq!(layout.floating_rects(), before, "floating geometry is kept");

        let tree_with_floating = LayoutTree::Split {
            axis: 0,
            ratio: 0.5,
            a: Box::new(LayoutTree::Leaf(a.0)),
            b: Box::new(LayoutTree::Leaf(floating.0)),
        };
        let before_tree = serde_json::to_value(layout.to_tree()).unwrap();
        assert!(layout.apply_tree(&tree_with_floating, a).is_err());
        assert!(
            layout.apply_tree(&tiled_tree, floating).is_err(),
            "a floating focus is not a valid tree focus"
        );
        assert_eq!(serde_json::to_value(layout.to_tree()).unwrap(), before_tree);
        assert_eq!(layout.focus, floating);
    }
}
