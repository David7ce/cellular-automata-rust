//! The central canvas: input handling (zoom, pan, tools, minimap) and drawing.

use eframe::egui::{self, Color32, Key, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

use super::{App, KEY_ZOOM_STEP, MINIMAP_MARGIN, MINIMAP_SIZE, PAN_STEP, Tool};
use crate::patterns;
use crate::simulation::{CHUNK_SIZE, Cell, SimState, WORLD_MAX, WORLD_MIN};
use crate::view::{self, MAX_CELL_SIZE, View};

impl App {
    pub(super) fn central_canvas(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        egui::CentralPanel::default().show(ui, |ui| {
            let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
            // The grid fills whatever space is available; cells stay square and
            // `View::clamp_to_world` keeps the viewport inside the world.
            self.canvas_size = rect.size();
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 0.0, Color32::from_gray(18));

            let pointer_local = response.hover_pos().map(|p| p - rect.min);
            let min_cell_size = view::min_cell_size_to_cover_world(self.canvas_size);
            let zoom_anchor = ctx
                .input(|i| i.multi_touch())
                .map(|t| t.center_pos - rect.min)
                .or(pointer_local)
                .unwrap_or(rect.size() / 2.0);

            self.handle_scroll_and_pinch(&ctx, &response, zoom_anchor, min_cell_size);
            self.handle_middle_pan(&ctx, &response);

            self.handle_keyboard(&ctx, rect, min_cell_size);

            // The minimap lives in the bottom-right corner and intercepts
            // clicks/drags there for navigation instead of painting/stamping.
            let minimap_rect = Rect::from_min_size(
                rect.max - MINIMAP_SIZE - Vec2::splat(MINIMAP_MARGIN),
                MINIMAP_SIZE,
            );
            let minimap_handled = self.handle_minimap_input(&response, minimap_rect);
            if !minimap_handled {
                self.handle_tool_input(&ctx, rect, &response);
            }
            if response.secondary_clicked() {
                self.selected_pattern = None;
            }

            self.clamp_view(min_cell_size);

            self.draw_grid(&painter, rect);
            self.draw_cells(&painter, rect);
            self.draw_cursor_overlay(&painter, rect, &response);

            self.draw_world_border(&painter, rect);

            draw_minimap(&self.sim, &self.view, self.canvas_size, &painter, minimap_rect);

            self.zoom_overlay(&ctx, rect, min_cell_size);

            self.pattern_library_overlay(&ctx, rect);
        });
    }

    fn handle_scroll_and_pinch(
        &mut self,
        ctx: &egui::Context,
        response: &egui::Response,
        zoom_anchor: Vec2,
        min_cell_size: f32,
    ) {
        // Gated on the canvas being the topmost thing under the pointer (egui
        // resolves this per layer, so it is false over the pattern library):
        // raw wheel/pinch events aren't routed to a widget, and without this
        // check scrolling the library's list would also zoom the map.
        if response.hover_pos().is_some() {
            // Pinch-to-zoom (touch pinch or ctrl+scroll), anchored on the gesture/pointer.
            let zoom_delta = ctx.input(|i| i.zoom_delta());
            if zoom_delta != 1.0 {
                self.view.zoom(zoom_delta, zoom_anchor, min_cell_size);
            }

            // Device-aware scroll, like a desktop map app: a physical mouse
            // wheel (discrete "line" steps) zooms anchored on the cursor —
            // the classic Google Maps behavior — while a trackpad's smooth,
            // continuous scrolling pans freely in both directions, like
            // panning a map on a phone or tablet. egui tags every scroll
            // event with which of these it came from (`MouseWheelUnit`), so
            // reading raw events instead of the pre-merged `scroll_delta`
            // lets the two devices drive genuinely different actions instead
            // of fighting over one. Ctrl/Cmd+scroll is left alone here since
            // `zoom_delta` above already handles it.
            let mut line_wheel_notches = 0.0f32;
            let mut trackpad_pan_delta = Vec2::ZERO;
            ctx.input(|i| {
                for event in &i.events {
                    let &egui::Event::MouseWheel {
                        unit,
                        delta,
                        modifiers,
                        ..
                    } = event
                    else {
                        continue;
                    };
                    if modifiers.ctrl || modifiers.command || modifiers.mac_cmd {
                        continue;
                    }
                    match unit {
                        egui::MouseWheelUnit::Line | egui::MouseWheelUnit::Page => {
                            line_wheel_notches += if delta.y != 0.0 { delta.y } else { delta.x };
                        }
                        egui::MouseWheelUnit::Point => trackpad_pan_delta += delta,
                    }
                }
            });
            if line_wheel_notches != 0.0 {
                self.view
                    .zoom(KEY_ZOOM_STEP.powf(line_wheel_notches), zoom_anchor, min_cell_size);
            }
            if trackpad_pan_delta != Vec2::ZERO {
                self.view.pan(trackpad_pan_delta);
            }
        }
    }

    fn handle_middle_pan(&mut self, ctx: &egui::Context, response: &egui::Response) {
        // Middle-mouse-button drag always pans, regardless of the active
        // tool or whether a pattern is selected — the "hold the wheel
        // button and drag" convention from Blender/Photoshop/Figma, kept
        // on a separate button so it never competes with the primary
        // button's drawing/pattern-placement/Pan-tool duties. Tracked
        // explicitly (like `dragging_minimap`) so it keeps panning even
        // if the cursor slips off the canvas mid-drag.
        if response.hover_pos().is_some()
            && ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Middle))
        {
            self.middle_pan_active = true;
        }
        if self.middle_pan_active {
            let (delta, still_down) = ctx.input(|i| {
                (
                    i.pointer.delta(),
                    i.pointer.button_down(egui::PointerButton::Middle),
                )
            });
            if delta != Vec2::ZERO {
                self.view.pan(delta);
            }
            if !still_down {
                self.middle_pan_active = false;
            }
        }
    }

    fn handle_keyboard(&mut self, ctx: &egui::Context, rect: Rect, min_cell_size: f32) {
        // Keyboard shortcuts (ignored while a widget like a text field wants
        // keyboard input, though none currently exist in this app).
        // One-shot actions (Escape/Space/S/C/R) ignore key-repeat events so
        // holding the key down doesn't spam them; zoom repeats on purpose.
        if !ctx.egui_wants_keyboard_input() {
            ctx.input(|i| {
                for event in &i.events {
                    let &egui::Event::Key {
                        key,
                        pressed: true,
                        repeat,
                        ..
                    } = event
                    else {
                        continue;
                    };
                    match key {
                        Key::Escape if !repeat => self.selected_pattern = None,
                        Key::Space if !repeat => self.sim.running = !self.sim.running,
                        Key::S if !repeat => self.sim.step_n(self.skip_generations),
                        Key::C if !repeat => self.sim.clear(),
                        Key::R if !repeat => {
                            if self.selected_pattern.is_some() {
                                self.selected_pattern_rotation =
                                    self.selected_pattern_rotation.wrapping_add(1) % 4;
                            } else {
                                self.randomize_visible(rect.size());
                            }
                        }
                        Key::F if !repeat => {
                            if self.selected_pattern.is_some() {
                                self.selected_pattern_flip = !self.selected_pattern_flip;
                            }
                        }
                        Key::Plus | Key::Equals => {
                            self.view.zoom(KEY_ZOOM_STEP, rect.size() / 2.0, min_cell_size)
                        }
                        Key::Minus => self
                            .view
                            .zoom(1.0 / KEY_ZOOM_STEP, rect.size() / 2.0, min_cell_size),
                        Key::ArrowUp => self.view.pan(Vec2::new(0.0, PAN_STEP)),
                        Key::ArrowDown => self.view.pan(Vec2::new(0.0, -PAN_STEP)),
                        Key::ArrowLeft => self.view.pan(Vec2::new(PAN_STEP, 0.0)),
                        Key::ArrowRight => self.view.pan(Vec2::new(-PAN_STEP, 0.0)),
                        _ => {}
                    }
                }
            });
        }
    }

    /// Returns whether the minimap consumed this frame's click/drag.
    fn handle_minimap_input(&mut self, response: &egui::Response, minimap_rect: Rect) -> bool {
        if response.drag_started()
            && let Some(p) = response.interact_pointer_pos()
            && minimap_rect.contains(p)
        {
            self.dragging_minimap = true;
        }
        if self.dragging_minimap {
            if let Some(p) = response.interact_pointer_pos().or_else(|| response.hover_pos()) {
                self.view
                    .center_on(minimap_to_world(minimap_rect, p), self.canvas_size);
            }
            if response.drag_stopped() {
                self.dragging_minimap = false;
            }
            true
        } else if response.clicked()
            && let Some(p) = response.interact_pointer_pos()
            && minimap_rect.contains(p)
        {
            self.view
                .center_on(minimap_to_world(minimap_rect, p), self.canvas_size);
            true
        } else {
            false
        }
    }

    /// Left-button behaviour of the active tool: stamp the selected pattern, pan, draw or erase.
    fn handle_tool_input(&mut self, ctx: &egui::Context, rect: Rect, response: &egui::Response) {
        match self.selected_pattern {
            Some(idx) => {
                // Placing a pattern: a single click stamps it once.
                if response.clicked()
                    && let Some(pointer) = response.interact_pointer_pos()
                {
                    let cell = self.view.screen_to_cell(rect.min, pointer);
                    let cells = patterns::transform_cells(
                        &self.library[idx].cells,
                        self.selected_pattern_rotation,
                        self.selected_pattern_flip,
                    );
                    self.sim.stamp(&cells, cell);
                }
            }
            None if self.tool == Tool::Pan => {
                // Google Maps-style click-and-drag panning: the
                // primary button, which Draw/Eraser use for
                // painting, instead moves the map directly under
                // the cursor.
                if response.dragged() {
                    self.view.pan(response.drag_delta());
                }
                ctx.set_cursor_icon(if response.dragged() {
                    egui::CursorIcon::Grabbing
                } else {
                    egui::CursorIcon::Grab
                });
            }
            None => {
                // Free drawing: press-and-drag paints (or erases) every cell the
                // cursor passes over, like a paintbrush. The Eraser tool forces
                // every stroke to remove cells regardless of their state, instead
                // of the Draw tool's toggle/paint-a-trail behavior.
                let erase = self.tool == Tool::Eraser;
                if response.drag_started() {
                    if let Some(pointer) = response.interact_pointer_pos() {
                        let cell = self.view.screen_to_cell(rect.min, pointer);
                        let value = if erase {
                            false
                        } else {
                            !self.sim.live.contains(&cell)
                        };
                        self.sim.set_cell(cell, value);
                        self.paint_value = Some(value);
                        self.last_paint_cell = Some(cell);
                    }
                } else if response.dragged() {
                    if let (Some(pointer), Some(value)) = (response.interact_pointer_pos(), self.paint_value)
                    {
                        let cell = self.view.screen_to_cell(rect.min, pointer);
                        if Some(cell) != self.last_paint_cell {
                            let from = self.last_paint_cell.unwrap_or(cell);
                            for c in line_cells(from, cell) {
                                self.sim.set_cell(c, value);
                            }
                            self.last_paint_cell = Some(cell);
                        }
                    }
                } else if response.clicked()
                    && let Some(pointer) = response.interact_pointer_pos()
                {
                    let cell = self.view.screen_to_cell(rect.min, pointer);
                    if erase {
                        self.sim.set_cell(cell, false);
                    } else {
                        self.sim.toggle_cell(cell);
                    }
                }
                if response.drag_stopped() {
                    self.paint_value = None;
                    self.last_paint_cell = None;
                }
            }
        }
    }

    fn clamp_view(&mut self, min_cell_size: f32) {
        // Applied once per frame, after every pan/zoom input this frame
        // (mouse, keyboard, on-screen buttons/slider, minimap) has had
        // its say. Re-clamping `cell_size` here (not just in `zoom()`)
        // covers window resizes too: if the canvas grows, `min_cell_size`
        // grows with it, and a `cell_size` that was previously exactly
        // at the "whole map visible" floor needs to grow to match —
        // otherwise resizing the window larger would let more than the
        // whole map show, breaking the "minimum zoom = whole map"
        // invariant. `clamp_to_world` then keeps the viewport fully
        // inside the world borders.
        self.view.cell_size = self
            .view
            .cell_size
            .clamp(min_cell_size.min(MAX_CELL_SIZE), MAX_CELL_SIZE);
        self.view.clamp_to_world(self.canvas_size);
    }

    fn draw_grid(&self, painter: &egui::Painter, rect: Rect) {
        let (min, max) = self.view.visible_bounds(rect.size());
        let cs = self.view.cell_size;

        if self.show_grid && cs > 4.0 {
            let stroke = Stroke::new(1.0, Color32::from_gray(35));
            let mut x = min.0;
            while x <= max.0 {
                let p = self.view.cell_to_screen(rect.min, (x, 0));
                painter.line_segment([Pos2::new(p.x, rect.min.y), Pos2::new(p.x, rect.max.y)], stroke);
                x += 1;
            }
            let mut y = min.1;
            while y <= max.1 {
                let p = self.view.cell_to_screen(rect.min, (0, y));
                painter.line_segment([Pos2::new(rect.min.x, p.y), Pos2::new(rect.max.x, p.y)], stroke);
                y += 1;
            }
        }
    }

    /// Live cells as one batched mesh (no per-rect shape tessellation).
    fn draw_cells(&self, painter: &egui::Painter, rect: Rect) {
        let (min, max) = self.view.visible_bounds(rect.size());
        let size = Vec2::splat(self.view.cell_size);
        let color = Color32::from_rgb(120, 220, 130);
        let mut mesh = egui::Mesh::default();
        for (x, y) in self.sim.cells_in_bounds(min, max) {
            let p = self.view.cell_to_screen(rect.min, (x, y));
            mesh.add_colored_rect(Rect::from_min_size(p, size), color);
            // Stay under the 65k-vertex limit of 16-bit-index backends.
            if mesh.vertices.len() >= 60_000 {
                painter.add(egui::Shape::mesh(std::mem::take(&mut mesh)));
            }
        }
        if !mesh.is_empty() {
            painter.add(egui::Shape::mesh(mesh));
        }
    }

    /// Ghost of the pattern in hand, or the eraser's cell outline, following the cursor.
    fn draw_cursor_overlay(&self, painter: &egui::Painter, rect: Rect, response: &egui::Response) {
        let cs = self.view.cell_size;
        // Ghost preview of the pattern in hand, following the cursor.
        if let Some(idx) = self.selected_pattern
            && let Some(pointer) = response.hover_pos()
        {
            let base = self.view.screen_to_cell(rect.min, pointer);
            let cells = patterns::transform_cells(
                &self.library[idx].cells,
                self.selected_pattern_rotation,
                self.selected_pattern_flip,
            );
            for (dx, dy) in cells {
                let p = self
                    .view
                    .cell_to_screen(rect.min, (base.0 + dx as i64, base.1 + dy as i64));
                painter.rect_filled(
                    Rect::from_min_size(p, Vec2::splat(cs)),
                    0.0,
                    Color32::from_rgba_unmultiplied(255, 255, 255, 100),
                );
            }
        } else if self.tool == Tool::Eraser
            && let Some(pointer) = response.hover_pos()
        {
            // Eraser cursor: a red outline over the cell it would remove.
            let cell = self.view.screen_to_cell(rect.min, pointer);
            let p = self.view.cell_to_screen(rect.min, cell);
            painter.rect_stroke(
                Rect::from_min_size(p, Vec2::splat(cs)),
                0.0,
                Stroke::new(2.0, Color32::from_rgb(220, 90, 90)),
                StrokeKind::Outside,
            );
        }
    }

    /// The plane is finite: draw its edge wherever it is on-screen.
    fn draw_world_border(&self, painter: &egui::Painter, rect: Rect) {
        let world_screen_min = self.view.cell_to_screen(rect.min, WORLD_MIN);
        let world_screen_max = self
            .view
            .cell_to_screen(rect.min, (WORLD_MAX.0 + 1, WORLD_MAX.1 + 1));
        painter.rect_stroke(
            Rect::from_min_max(world_screen_min, world_screen_max),
            0.0,
            Stroke::new(2.0, Color32::from_rgb(190, 90, 90)),
            StrokeKind::Outside,
        );
    }

    /// Floating +/-/reset zoom buttons over the canvas's bottom-left corner.
    fn zoom_overlay(&mut self, ctx: &egui::Context, rect: Rect, min_cell_size: f32) {
        // On-canvas zoom control (Google Maps-style +/-, plus a reset
        // button), floating over the canvas's bottom-left corner —
        // mirrors the minimap's placement in the opposite corner. A
        // floating `egui::Area` rather than a top-bar row, so it's
        // always available regardless of whether the top bar is
        // expanded or collapsed. Buttons are large, fixed-size squares
        // (rather than default-sized text buttons) so they read as a
        // deliberate map-style control cluster and stay comfortably
        // clickable at any zoom level.
        let zoom_button_size = Vec2::splat(34.0);
        // Anchored to the window's bottom-left corner with egui's own
        // (auto-measured) size, rather than a fixed position computed
        // from a guessed content height — that guess could drift out of
        // sync with the actual content and clip off the bottom of the
        // window on some platform/font combination; anchoring can't.
        egui::Area::new(egui::Id::new("zoom_overlay"))
            .anchor(
                egui::Align2::LEFT_BOTTOM,
                Vec2::new(MINIMAP_MARGIN, -MINIMAP_MARGIN),
            )
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).inner_margin(6.0).show(ui, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(0.0, 4.0);
                    ui.vertical(|ui| {
                        let plus = egui::RichText::new("+").size(18.0).strong();
                        if ui
                            .add_sized(zoom_button_size, egui::Button::new(plus))
                            .on_hover_text("Zoom in")
                            .clicked()
                        {
                            self.view.zoom(KEY_ZOOM_STEP, rect.size() / 2.0, min_cell_size);
                        }
                        let minus = egui::RichText::new("−").size(18.0).strong();
                        if ui
                            .add_sized(zoom_button_size, egui::Button::new(minus))
                            .on_hover_text("Zoom out")
                            .clicked()
                        {
                            self.view
                                .zoom(1.0 / KEY_ZOOM_STEP, rect.size() / 2.0, min_cell_size);
                        }
                        ui.separator();
                        if ui
                            .add_sized(zoom_button_size, egui::Button::new("⟲"))
                            .on_hover_text("Reset view: recenter on the world and reset zoom")
                            .clicked()
                        {
                            self.view = View::default();
                        }
                        ui.separator();
                        ui.label(egui::RichText::new(format!("{:.0}px", self.view.cell_size)).small());
                    });
                });
            });
    }
}

/// Maps a screen point inside the minimap box to the world cell it
/// represents, for click/drag-to-navigate.
fn minimap_to_world(minimap_rect: Rect, p: Pos2) -> Cell {
    let world_w = (WORLD_MAX.0 - WORLD_MIN.0 + 1) as f32;
    let world_h = (WORLD_MAX.1 - WORLD_MIN.1 + 1) as f32;
    let local = p - minimap_rect.min;
    let fx = (local.x / minimap_rect.width()).clamp(0.0, 1.0);
    let fy = (local.y / minimap_rect.height()).clamp(0.0, 1.0);
    (
        (WORLD_MIN.0 as f32 + fx * world_w).round() as i64,
        (WORLD_MIN.1 as f32 + fy * world_h).round() as i64,
    )
}

/// Draws the bottom-right minimap: the whole (finite) plane, a coarse marker
/// per occupied spatial-index chunk (cheap: `O(occupied chunks)`, not
/// `O(live cells)`), and a rectangle showing the current viewport.
fn draw_minimap(sim: &SimState, view: &View, canvas_size: Vec2, painter: &egui::Painter, minimap_rect: Rect) {
    painter.rect_filled(minimap_rect, 4.0, Color32::from_black_alpha(215));

    let world_w = (WORLD_MAX.0 - WORLD_MIN.0 + 1) as f32;
    let world_h = (WORLD_MAX.1 - WORLD_MIN.1 + 1) as f32;
    let sx = minimap_rect.width() / world_w;
    let sy = minimap_rect.height() / world_h;

    for (cx, cy) in sim.occupied_chunks() {
        let x0 = minimap_rect.min.x + ((cx * CHUNK_SIZE) as f32 - WORLD_MIN.0 as f32) * sx;
        let y0 = minimap_rect.min.y + ((cy * CHUNK_SIZE) as f32 - WORLD_MIN.1 as f32) * sy;
        let w = (CHUNK_SIZE as f32 * sx).max(1.0);
        let h = (CHUNK_SIZE as f32 * sy).max(1.0);
        let chunk_rect = Rect::from_min_size(Pos2::new(x0, y0), Vec2::new(w, h)).intersect(minimap_rect);
        painter.rect_filled(chunk_rect, 0.0, Color32::from_rgb(90, 170, 100));
    }

    let (vmin, vmax) = view.visible_bounds(canvas_size);
    let vp_min = Pos2::new(
        minimap_rect.min.x + (vmin.0.clamp(WORLD_MIN.0, WORLD_MAX.0) as f32 - WORLD_MIN.0 as f32) * sx,
        minimap_rect.min.y + (vmin.1.clamp(WORLD_MIN.1, WORLD_MAX.1) as f32 - WORLD_MIN.1 as f32) * sy,
    );
    let vp_max = Pos2::new(
        minimap_rect.min.x
            + ((vmax.0 + 1).clamp(WORLD_MIN.0, WORLD_MAX.0 + 1) as f32 - WORLD_MIN.0 as f32) * sx,
        minimap_rect.min.y
            + ((vmax.1 + 1).clamp(WORLD_MIN.1, WORLD_MAX.1 + 1) as f32 - WORLD_MIN.1 as f32) * sy,
    );
    let viewport_rect = Rect::from_min_max(vp_min, vp_max).intersect(minimap_rect);
    painter.rect_stroke(
        viewport_rect,
        0.0,
        Stroke::new(1.5, Color32::from_rgb(255, 210, 90)),
        StrokeKind::Outside,
    );

    painter.rect_stroke(
        minimap_rect,
        4.0,
        Stroke::new(1.0, Color32::from_gray(110)),
        StrokeKind::Outside,
    );
}

/// Bresenham line between two cells, so fast drags don't leave gaps.
fn line_cells(a: Cell, b: Cell) -> Vec<Cell> {
    let mut cells = Vec::new();
    let (mut x0, mut y0) = a;
    let (x1, y1) = b;
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx: i64 = if x0 < x1 { 1 } else { -1 };
    let sy: i64 = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        cells.push((x0, y0));
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
    cells
}
