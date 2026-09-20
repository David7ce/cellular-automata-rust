use eframe::egui::{Pos2, Vec2};

use crate::simulation::{Cell, WORLD_MAX, WORLD_MIN};

/// Absolute safety floor for `cell_size`, guarding against degenerate math
/// (e.g. a zero-sized canvas during startup). Not a user-facing zoom limit —
/// see `min_cell_size_to_fit_world` for the real, dynamic "zoomed all the
/// way out" bound, which is normally far above this.
const ABSOLUTE_MIN_CELL_SIZE: f32 = 0.05;
pub const MAX_CELL_SIZE: f32 = 60.0;

/// Camera over the infinite grid: `offset` is the world-space point (in cell
/// units) shown at the top-left of the canvas; `cell_size` is the on-screen
/// pixel size of one cell (the zoom level).
pub struct View {
    pub offset: Vec2,
    pub cell_size: f32,
}

impl Default for View {
    fn default() -> Self {
        View {
            offset: Vec2::new(-20.0, -15.0),
            cell_size: 16.0,
        }
    }
}

impl View {
    pub fn screen_to_cell(&self, screen_origin: Pos2, p: Pos2) -> Cell {
        let local = p - screen_origin;
        let wx = self.offset.x + local.x / self.cell_size;
        let wy = self.offset.y + local.y / self.cell_size;
        (wx.floor() as i64, wy.floor() as i64)
    }

    pub fn cell_to_screen(&self, screen_origin: Pos2, cell: Cell) -> Pos2 {
        let wx = cell.0 as f32 - self.offset.x;
        let wy = cell.1 as f32 - self.offset.y;
        screen_origin + Vec2::new(wx * self.cell_size, wy * self.cell_size)
    }

    pub fn pan(&mut self, screen_delta: Vec2) {
        self.offset -= screen_delta / self.cell_size;
    }

    /// Zooms so that the world point currently under `anchor` (screen-space,
    /// relative to the canvas origin) stays fixed. `min_cell_size` is the
    /// current lower bound (see `min_cell_size_to_fit_world`) — passed in by
    /// the caller, which knows the current canvas size, rather than a fixed
    /// constant, so the user can never zoom out past "the whole map is
    /// visible" regardless of window size.
    pub fn zoom(&mut self, factor: f32, anchor_local: Vec2, min_cell_size: f32) {
        let world_x = self.offset.x + anchor_local.x / self.cell_size;
        let world_y = self.offset.y + anchor_local.y / self.cell_size;

        self.cell_size = (self.cell_size * factor).clamp(min_cell_size.min(MAX_CELL_SIZE), MAX_CELL_SIZE);

        self.offset.x = world_x - anchor_local.x / self.cell_size;
        self.offset.y = world_y - anchor_local.y / self.cell_size;
    }

    /// Visible cell bounds (inclusive) for a canvas of the given pixel size.
    pub fn visible_bounds(&self, canvas_size: Vec2) -> (Cell, Cell) {
        let min = (self.offset.x.floor() as i64, self.offset.y.floor() as i64);
        let max = (
            (self.offset.x + canvas_size.x / self.cell_size).ceil() as i64,
            (self.offset.y + canvas_size.y / self.cell_size).ceil() as i64,
        );
        (min, max)
    }

    /// Re-centers the view on `cell` (keeping the current zoom level), for
    /// minimap click/drag navigation.
    pub fn center_on(&mut self, cell: Cell, canvas_size: Vec2) {
        self.offset = Vec2::new(cell.0 as f32, cell.1 as f32) - (canvas_size / 2.0) / self.cell_size;
    }

    /// Keeps the visible viewport fully inside the world's borders — it can
    /// slide right up to an edge but never show empty space beyond it. If
    /// the viewport is bigger than the world on some axis (e.g. zoomed far
    /// out on a large window), that axis is centered on the world instead,
    /// since there's no in-bounds position that would fill the screen.
    pub fn clamp_to_world(&mut self, canvas_size: Vec2) {
        let visible_w = canvas_size.x / self.cell_size;
        let visible_h = canvas_size.y / self.cell_size;
        let world_w = (WORLD_MAX.0 - WORLD_MIN.0 + 1) as f32;
        let world_h = (WORLD_MAX.1 - WORLD_MIN.1 + 1) as f32;

        self.offset.x = clamp_axis(self.offset.x, visible_w, WORLD_MIN.0 as f32, world_w);
        self.offset.y = clamp_axis(self.offset.y, visible_h, WORLD_MIN.1 as f32, world_h);
    }
}

fn clamp_axis(offset: f32, visible: f32, world_min: f32, world_size: f32) -> f32 {
    if visible >= world_size {
        world_min - (visible - world_size) / 2.0
    } else {
        offset.clamp(world_min, world_min + world_size - visible)
    }
}

/// The zoom level (px/cell) at which the whole world exactly fits within a
/// canvas of `canvas_size` — the Google Maps-style "minimum zoom": zooming
/// out stops once the entire map is on screen, since there's nothing more
/// to see beyond its borders. Whichever axis is tighter (the canvas's
/// aspect ratio rarely matches the world's exactly) determines the value;
/// `View::clamp_to_world` then centers the other, looser axis so nothing
/// outside the world is ever shown.
pub fn min_cell_size_to_fit_world(canvas_size: Vec2) -> f32 {
    let world_w = (WORLD_MAX.0 - WORLD_MIN.0 + 1) as f32;
    let world_h = (WORLD_MAX.1 - WORLD_MIN.1 + 1) as f32;
    (canvas_size.x / world_w)
        .min(canvas_size.y / world_h)
        .max(ABSOLUTE_MIN_CELL_SIZE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_pulls_a_far_away_offset_back_inside_the_world() {
        let canvas = Vec2::new(800.0, 600.0);
        let mut view = View {
            offset: Vec2::new(-1_000_000.0, 1_000_000.0),
            cell_size: 16.0,
        };
        view.clamp_to_world(canvas);

        let visible_w = canvas.x / view.cell_size;
        let visible_h = canvas.y / view.cell_size;
        let world_w = (WORLD_MAX.0 - WORLD_MIN.0 + 1) as f32;
        let world_h = (WORLD_MAX.1 - WORLD_MIN.1 + 1) as f32;

        assert!(view.offset.x >= WORLD_MIN.0 as f32 - 1e-3);
        assert!(view.offset.x + visible_w <= WORLD_MIN.0 as f32 + world_w + 1e-3);
        assert!(view.offset.y >= WORLD_MIN.1 as f32 - 1e-3);
        assert!(view.offset.y + visible_h <= WORLD_MIN.1 as f32 + world_h + 1e-3);
    }

    #[test]
    fn clamp_leaves_an_already_inside_offset_untouched() {
        let canvas = Vec2::new(800.0, 600.0);
        let mut view = View {
            offset: Vec2::new(-20.0, -15.0),
            cell_size: 16.0,
        };
        view.clamp_to_world(canvas);
        assert_eq!(view.offset, Vec2::new(-20.0, -15.0));
    }

    #[test]
    fn clamp_centers_an_axis_when_the_viewport_is_wider_than_the_world() {
        let cell_size = 0.1; // zoomed far out
        let canvas = Vec2::new(400.0, 300.0);
        let mut view = View {
            offset: Vec2::ZERO,
            cell_size,
        };
        view.clamp_to_world(canvas);

        let visible_w = canvas.x / cell_size;
        let world_w = (WORLD_MAX.0 - WORLD_MIN.0 + 1) as f32;
        assert!(
            visible_w > world_w,
            "test setup should make the viewport wider than the world"
        );

        let expected_x = WORLD_MIN.0 as f32 - (visible_w - world_w) / 2.0;
        assert!((view.offset.x - expected_x).abs() < 1e-3);
    }

    #[test]
    fn min_cell_size_to_fit_world_picks_the_tighter_axis() {
        let world_w = (WORLD_MAX.0 - WORLD_MIN.0 + 1) as f32;
        let world_h = (WORLD_MAX.1 - WORLD_MIN.1 + 1) as f32;

        // A canvas much wider than the world's aspect ratio: height is the
        // constraint, so the fit value should come from that axis.
        let canvas = Vec2::new(world_w * 10.0, world_h);
        let min_cs = min_cell_size_to_fit_world(canvas);
        assert!(
            (min_cs - 1.0).abs() < 1e-3,
            "expected height-bound fit of 1px/cell, got {min_cs}"
        );

        // And the resulting viewport at that zoom must be at least as big as
        // the whole world on both axes (so the whole map really is visible).
        assert!(canvas.x / min_cs >= world_w - 1e-3);
        assert!(canvas.y / min_cs >= world_h - 1e-3);
    }

    #[test]
    fn zoom_never_goes_below_the_passed_in_minimum() {
        let canvas = Vec2::new(200.0, 150.0);
        let min_cs = min_cell_size_to_fit_world(canvas);
        let mut view = View {
            offset: Vec2::ZERO,
            cell_size: min_cs,
        };
        // Try to zoom out far past the minimum in one step.
        view.zoom(0.001, canvas / 2.0, min_cs);
        assert!(
            (view.cell_size - min_cs).abs() < 1e-6,
            "zoom should clamp to min_cell_size, got {}",
            view.cell_size
        );
    }
}
