use eframe::egui::{self, Color32, Key, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

use crate::patterns::{self, Category, Pattern};
use crate::rules;
use crate::simulation::{Cell, SimState, CHUNK_SIZE, WORLD_MAX, WORLD_MIN};
use crate::starts;
use crate::view::{self, View, MAX_CELL_SIZE};

/// Zoom factor applied per keyboard/button zoom-shortcut press ('+'/'-').
const KEY_ZOOM_STEP: f32 = 1.2;
/// Screen-pixel-equivalent pan distance per keyboard arrow / pan-button press.
const PAN_STEP: f32 = 60.0;
/// Options for the "generations to skip" selector next to Step.
const SKIP_OPTIONS: &[u32] = &[0, 5, 10, 50, 100, 500, 1000];
/// On-screen size of the minimap box, anchored to the canvas's bottom-right
/// corner with `MINIMAP_MARGIN` of breathing room.
/// The world is a 16:9 rectangle (960x540), not a square, so the minimap
/// must be too — a square box would non-uniformly stretch the map (and
/// everything drawn on it: the viewport outline, occupied-chunk markers)
/// to fill it, distorting proportions instead of just showing the whole
/// plane shrunk down evenly.
const MINIMAP_SIZE: Vec2 = Vec2::new(160.0, 90.0);
const MINIMAP_MARGIN: f32 = 12.0;

/// Which action a left-click/drag on the canvas performs. Mutually
/// exclusive, like a toolbox — picking one always turns off the others.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tool {
    /// Click toggles a cell, drag paints (or erases, if the stroke starts on
    /// a live cell) a trail.
    Draw,
    /// Click-and-drag pans the view, like grabbing and dragging a map — the
    /// same gesture Google Maps uses on desktop. A separate tool from Draw
    /// because the primary mouse button is already spoken for by drawing.
    Pan,
    /// Click/drag always removes cells, regardless of their state.
    Eraser,
}

/// Everything in this file is the *2D renderer*: it turns `SimState`'s cells
/// and `View`'s camera into `egui::Painter` calls, and turns pointer/keyboard
/// input into `View`/`SimState` mutations. It deliberately never reaches
/// into simulation internals beyond the public `Cell`/`SimState` API, so a
/// future 3D build could swap this whole module for a `wgpu`-based
/// voxel/instanced-cube renderer and an orbiting 3D camera (replacing
/// `View`) without `simulation.rs`/`rules.rs`/`patterns.rs` needing to
/// change beyond widening `Cell` to `(i64, i64, i64)` (see the neighbor-
/// offset note in `simulation.rs`).
pub struct App {
    sim: SimState,
    view: View,
    library: Vec<Pattern>,
    selected_pattern: Option<usize>,
    preset_name: &'static str,
    preset_class: &'static str,
    random_density: f32,
    /// Whether the current drag-paint stroke is drawing (true) or erasing (false).
    paint_value: Option<bool>,
    last_paint_cell: Option<Cell>,
    show_grid: bool,
    /// How many generations a single "Step" advances at once (0 behaves as 1).
    skip_generations: u32,
    /// Which action left-click/drag performs on the canvas (draw/pan/erase).
    /// Selecting a pattern from the library always resets this to `Draw`.
    tool: Tool,
    /// Index into `starts::START_CONFIGS`, the "Start" dropdown's selection.
    selected_start: usize,
    /// Canvas size from the last frame, used to anchor button/slider zoom on
    /// the canvas center (the mouse-based zoom anchors on the cursor instead).
    canvas_size: Vec2,
    /// Whether an in-progress drag started inside the minimap (so it keeps
    /// steering the camera even if the pointer strays outside the box).
    dragging_minimap: bool,
    /// Whether a middle-mouse-button drag-to-pan is in progress (tracked
    /// explicitly, like `dragging_minimap`, so panning keeps working even if
    /// the cursor slips off the canvas mid-drag).
    middle_pan_active: bool,
    /// Whether the pattern-library overlay is shown — the "☰" button toggles
    /// this. It floats on top of the canvas (an `egui::Area`, not a side
    /// `Panel`) so showing/hiding it never changes the canvas's own size —
    /// the map's aspect ratio stays correct and full-size either way.
    show_side_panel: bool,
    /// Whether the Simulation/custom-rule control rows are shown below the
    /// always-visible essentials — the "⚙" button toggles this, collapsing
    /// them to reclaim canvas height.
    show_extra_controls: bool,
}

impl App {
    pub fn new() -> Self {
        let rule = rules::preset_rule(&rules::PRESETS[0]);
        let mut sim = SimState::new(rule);
        // Seed with a glider so the canvas isn't empty on first launch.
        sim.stamp(&crate::rle::parse("bo$2bo$3o!"), (2, 2));

        App {
            sim,
            view: View::default(),
            library: patterns::library(),
            selected_pattern: None,
            preset_name: rules::PRESETS[0].name,
            preset_class: rules::PRESETS[0].class,
            random_density: 0.35,
            paint_value: None,
            last_paint_cell: None,
            show_grid: true,
            skip_generations: 0,
            tool: Tool::Draw,
            selected_start: 0,
            canvas_size: Vec2::new(800.0, 600.0),
            dragging_minimap: false,
            middle_pan_active: false,
            show_side_panel: true,
            show_extra_controls: true,
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let dt = ctx.input(|i| i.stable_dt);
        self.sim.tick(dt);
        if self.sim.running {
            ctx.request_repaint();
        }

        self.top_panel(ui);
        self.central_canvas(ui);
    }
}

impl App {
    fn top_panel(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("controls").show(ui, |ui| {
            // Always-visible essentials, regardless of `show_extra_controls`
            // — includes the two collapse toggles themselves, so the bars
            // can always be reclaimed for canvas space and always reopened.
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(self.show_side_panel, "☰")
                    .on_hover_text("Show/hide the pattern library panel")
                    .clicked()
                {
                    self.show_side_panel = !self.show_side_panel;
                }
                if ui
                    .selectable_label(self.show_extra_controls, "⚙")
                    .on_hover_text("Show/hide rule/board/custom-rule settings — collapse both bars to give the map more room")
                    .clicked()
                {
                    self.show_extra_controls = !self.show_extra_controls;
                }

                ui.separator();
                if ui
                    .button(if self.sim.running { "⏸" } else { "▶" })
                    .on_hover_text(if self.sim.running { "Pause" } else { "Play" })
                    .clicked()
                {
                    self.sim.running = !self.sim.running;
                }
                if ui
                    .button("⏭")
                    .on_hover_text("Step: advance by the Skip amount (1 generation if Skip is 0)")
                    .clicked()
                {
                    self.sim.step_n(self.skip_generations);
                }

                // The tool toggle lives here, always visible (not behind the
                // "⚙" collapse), since it's a core interaction mode rather
                // than a setting — it needs to always be reachable.
                ui.separator();
                if ui.selectable_label(self.tool == Tool::Draw, "Draw").on_hover_text("Click/drag to toggle or paint cells").clicked() {
                    self.tool = Tool::Draw;
                }
                if ui
                    .selectable_label(self.tool == Tool::Pan, "✋ Pan")
                    .on_hover_text("Click/drag to pan the view, like dragging a map (Google Maps-style)")
                    .clicked()
                {
                    self.tool = Tool::Pan;
                    self.selected_pattern = None;
                }
                if ui
                    .selectable_label(self.tool == Tool::Eraser, "Eraser")
                    .on_hover_text("Click/drag to remove cells (goma de borrar)")
                    .clicked()
                {
                    self.tool = Tool::Eraser;
                    self.selected_pattern = None;
                }

                ui.separator();
                ui.label(format!("Gen: {}", self.sim.generation));
                ui.label(format!("Live: {}", self.sim.live.len()));
            });

            if !self.show_extra_controls {
                return;
            }

            ui.separator();
            ui.label(egui::RichText::new("Simulation").small().strong());
            // One wrapped row instead of separate "Simulation"/"Board"
            // rows — wraps to more lines on a narrow window rather than
            // overflowing, since there's now a fair amount packed in here.
            ui.horizontal_wrapped(|ui| {
                ui.label("Rule:");
                egui::ComboBox::from_id_salt("rule_preset")
                    .selected_text(format!("{} ({})", self.preset_name, self.preset_class))
                    .show_ui(ui, |ui| {
                        // Grouped by long-term random-soup behavior instead
                        // of one flat 22-entry list, so a preset's class is
                        // a section header you land in rather than a suffix
                        // you have to read on every single row.
                        for &class in &["chaotic", "explosive", "stable"] {
                            ui.label(egui::RichText::new(class).small().strong());
                            for preset in rules::PRESETS.iter().filter(|p| p.class == class) {
                                ui.horizontal(|ui| {
                                    ui.add_space(12.0);
                                    if ui.selectable_label(self.preset_name == preset.name, preset.name).clicked() {
                                        self.preset_name = preset.name;
                                        self.preset_class = preset.class;
                                        self.sim.rule = rules::preset_rule(preset);
                                    }
                                });
                            }
                        }
                    });
                ui.label(self.sim.rule.to_bs_string());

                ui.separator();
                ui.label("Skip");
                egui::ComboBox::from_id_salt("skip_generations")
                    .selected_text(self.skip_generations.to_string())
                    .show_ui(ui, |ui| {
                        for &n in SKIP_OPTIONS {
                            ui.selectable_value(&mut self.skip_generations, n, n.to_string());
                        }
                    });

                ui.separator();
                ui.label("Speed");
                ui.add(egui::Slider::new(&mut self.sim.speed, 0.5..=60.0).suffix(" gen/s"));

                ui.separator();
                ui.label(format!("Births: {}", self.sim.last_births))
                    .on_hover_text("Cells born on the most recent step (or summed over a Skip batch)");
                ui.label(format!("Deaths: {}", self.sim.last_deaths))
                    .on_hover_text("Cells that died on the most recent step (or summed over a Skip batch)");

                ui.separator();
                ui.label("Start:");
                egui::ComboBox::from_id_salt("start_config")
                    .selected_text(starts::START_CONFIGS[self.selected_start])
                    .show_ui(ui, |ui| {
                        for (idx, name) in starts::START_CONFIGS.iter().enumerate() {
                            ui.selectable_value(&mut self.selected_start, idx, *name);
                        }
                    });
                if ui
                    .button("Load")
                    .on_hover_text("Clear the board and lay out the selected starting configuration")
                    .clicked()
                {
                    starts::apply(&mut self.sim, starts::START_CONFIGS[self.selected_start], &self.library, self.random_density);
                    self.selected_pattern = None;
                    let center = ((WORLD_MIN.0 + WORLD_MAX.0) / 2, (WORLD_MIN.1 + WORLD_MAX.1) / 2);
                    self.view.center_on(center, self.canvas_size);
                }

                ui.separator();
                if ui.button("🗑").on_hover_text("Clear: erase every live cell").clicked() {
                    self.sim.clear();
                }
                if ui
                    .button("🎲")
                    .on_hover_text(format!("Random: fill the visible area at {:.0}% density", self.random_density * 100.0))
                    .clicked()
                {
                    // `canvas_size` (one frame stale, like the zoom controls
                    // above used to be) is the aspect-locked map size, not
                    // this row's own width — using `ui.available_size()`
                    // here would fill by the top bar's width, not the map's.
                    let (min, max) = self.view.visible_bounds(self.canvas_size.max(Vec2::new(400.0, 400.0)));
                    self.sim.randomize(min, max, self.random_density);
                }
                ui.add(egui::Slider::new(&mut self.random_density, 0.05..=0.9).text("density"));

                ui.separator();
                ui.checkbox(&mut self.show_grid, "Show grid");
            });

            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Custom rule — Birth:");
                let mut changed = false;
                for n in 0..=8u8 {
                    let mut on = self.sim.rule.birth[n as usize];
                    if ui.checkbox(&mut on, n.to_string()).changed() {
                        self.sim.rule.birth[n as usize] = on;
                        changed = true;
                    }
                }
                ui.label("Survive:");
                for n in 0..=8u8 {
                    let mut on = self.sim.rule.survive[n as usize];
                    if ui.checkbox(&mut on, n.to_string()).changed() {
                        self.sim.rule.survive[n as usize] = on;
                        changed = true;
                    }
                }
                if changed {
                    self.preset_name = "Custom";
                    self.preset_class = "custom";
                }
            });
        });
    }

    /// The pattern library as a floating overlay on top of the canvas
    /// (rather than a side `Panel` that shrinks it) — toggled by the "☰"
    /// button, positioned in the canvas's top-left corner. Since it's just
    /// painted over the canvas in its own layer, showing/hiding it never
    /// changes the canvas's actual size, so the map's aspect ratio (and how
    /// much of it is visible) stays exactly the same either way — including
    /// in a maximized/full-screen window, where this is most visible.
    fn pattern_library_overlay(&mut self, ctx: &egui::Context, canvas_rect: Rect) {
        if !self.show_side_panel {
            return;
        }
        egui::Area::new(egui::Id::new("pattern_library_overlay"))
            .fixed_pos(canvas_rect.min + Vec2::splat(MINIMAP_MARGIN))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(240.0);
                    ui.horizontal(|ui| {
                        ui.heading("Pattern Library");
                        // Right-to-left sub-layout pins the close button to
                        // this row's far right edge (the panel's top-right
                        // corner), rather than immediately after the heading
                        // text.
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("✖").on_hover_text("Hide the pattern library").clicked() {
                                self.show_side_panel = false;
                            }
                        });
                    });
                    if self.selected_pattern.is_some() {
                        ui.horizontal(|ui| {
                            ui.label("Click canvas to place. ");
                            if ui.button("✖").on_hover_text("Cancel pattern placement").clicked() {
                                self.selected_pattern = None;
                            }
                        });
                    }
                    ui.separator();
                    // `auto_shrink([false, true])`: always claim the full
                    // panel width (so the scrollbar sits flush at the
                    // panel's right edge, not hugging the widest row's
                    // content) while still only growing as tall as the
                    // (now default-open) categories' content needs, up to
                    // `max_height` — most of the canvas's height, so the
                    // library can actually grow to show everything at once
                    // instead of being capped short.
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, true])
                        .max_height((canvas_rect.height() - 3.0 * MINIMAP_MARGIN - 70.0).max(240.0))
                        .show(ui, |ui| {
                            for category in Category::ALL {
                                egui::CollapsingHeader::new(category.label()).default_open(true).show(ui, |ui| {
                                    for (idx, pattern) in self.library.iter().enumerate() {
                                        if pattern.category != category {
                                            continue;
                                        }
                                        ui.horizontal(|ui| {
                                            let (rect, response) =
                                                ui.allocate_exact_size(Vec2::new(36.0, 36.0), Sense::click());
                                            paint_pattern_preview(ui.painter(), rect, &pattern.cells);
                                            let label = ui.selectable_label(
                                                self.selected_pattern == Some(idx),
                                                pattern.name,
                                            );
                                            if response.clicked() || label.clicked() {
                                                self.selected_pattern = Some(idx);
                                                self.tool = Tool::Draw;
                                            }
                                        });
                                    }
                                });
                            }
                        });
                });
            });
    }

    fn central_canvas(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        egui::CentralPanel::default().show(ui, |ui| {
            let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
            // The world is a fixed 16:9 rectangle, but the canvas rarely is
            // (the window's own shape, plus whatever the top/side bars still
            // take up, decide that) — so `map_rect` is the largest exact-16:9
            // rectangle that fits centered in `rect`, and everything below
            // maps screen<->world through it instead of the raw canvas rect.
            // This is what actually fixes "the map doesn't look 16:9": the
            // area really is a true, undistorted 16:9 rectangle now, letter/
            // pillar-boxed within whatever space the canvas has, rather than
            // stretched or cropped to fit an arbitrary window shape.
            let world_w = (WORLD_MAX.0 - WORLD_MIN.0 + 1) as f32;
            let world_h = (WORLD_MAX.1 - WORLD_MIN.1 + 1) as f32;
            let map_rect = fit_aspect_rect(rect, world_w / world_h);
            self.canvas_size = map_rect.size();
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 0.0, Color32::from_gray(10));
            painter.rect_filled(map_rect, 0.0, Color32::from_gray(18));

            let pointer_local = response.hover_pos().map(|p| p - map_rect.min);
            let min_cell_size = view::min_cell_size_to_fit_world(self.canvas_size);
            let zoom_anchor = ctx
                .input(|i| i.multi_touch())
                .map(|t| t.center_pos - map_rect.min)
                .or(pointer_local)
                .unwrap_or(map_rect.size() / 2.0);

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
                    let &egui::Event::MouseWheel { unit, delta, modifiers, .. } = event else {
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
                self.view.zoom(KEY_ZOOM_STEP.powf(line_wheel_notches), zoom_anchor, min_cell_size);
            }
            if trackpad_pan_delta != Vec2::ZERO {
                self.view.pan(trackpad_pan_delta);
            }

            // Middle-mouse-button drag always pans, regardless of the active
            // tool or whether a pattern is selected — the "hold the wheel
            // button and drag" convention from Blender/Photoshop/Figma, kept
            // on a separate button so it never competes with the primary
            // button's drawing/pattern-placement/Pan-tool duties. Tracked
            // explicitly (like `dragging_minimap`) so it keeps panning even
            // if the cursor slips off the canvas mid-drag.
            if response.hover_pos().is_some() && ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Middle)) {
                self.middle_pan_active = true;
            }
            if self.middle_pan_active {
                let (delta, still_down) =
                    ctx.input(|i| (i.pointer.delta(), i.pointer.button_down(egui::PointerButton::Middle)));
                if delta != Vec2::ZERO {
                    self.view.pan(delta);
                }
                if !still_down {
                    self.middle_pan_active = false;
                }
            }

            // Keyboard shortcuts (ignored while a widget like a text field wants
            // keyboard input, though none currently exist in this app).
            // One-shot actions (Escape/Space/S/C/R) ignore key-repeat events so
            // holding the key down doesn't spam them; zoom repeats on purpose.
            if !ctx.egui_wants_keyboard_input() {
                ctx.input(|i| {
                    for event in &i.events {
                        let &egui::Event::Key { key, pressed: true, repeat, .. } = event else {
                            continue;
                        };
                        match key {
                            Key::Escape if !repeat => self.selected_pattern = None,
                            Key::Space if !repeat => self.sim.running = !self.sim.running,
                            Key::S if !repeat => self.sim.step_n(self.skip_generations),
                            Key::C if !repeat => self.sim.clear(),
                            Key::R if !repeat => {
                                let (min, max) = self.view.visible_bounds(map_rect.size());
                                self.sim.randomize(min, max, self.random_density);
                            }
                            Key::Plus | Key::Equals => self.view.zoom(KEY_ZOOM_STEP, map_rect.size() / 2.0, min_cell_size),
                            Key::Minus => self.view.zoom(1.0 / KEY_ZOOM_STEP, map_rect.size() / 2.0, min_cell_size),
                            Key::ArrowUp => self.view.pan(Vec2::new(0.0, PAN_STEP)),
                            Key::ArrowDown => self.view.pan(Vec2::new(0.0, -PAN_STEP)),
                            Key::ArrowLeft => self.view.pan(Vec2::new(PAN_STEP, 0.0)),
                            Key::ArrowRight => self.view.pan(Vec2::new(-PAN_STEP, 0.0)),
                            _ => {}
                        }
                    }
                });
            }

            // The minimap lives in the bottom-right corner and intercepts
            // clicks/drags there for navigation instead of painting/stamping.
            let minimap_rect = Rect::from_min_size(rect.max - MINIMAP_SIZE - Vec2::splat(MINIMAP_MARGIN), MINIMAP_SIZE);
            if response.drag_started()
                && let Some(p) = response.interact_pointer_pos()
                && minimap_rect.contains(p)
            {
                self.dragging_minimap = true;
            }
            let minimap_handled = if self.dragging_minimap {
                if let Some(p) = response.interact_pointer_pos().or_else(|| response.hover_pos()) {
                    self.view.center_on(minimap_to_world(minimap_rect, p), self.canvas_size);
                }
                if response.drag_stopped() {
                    self.dragging_minimap = false;
                }
                true
            } else if response.clicked()
                && let Some(p) = response.interact_pointer_pos()
                && minimap_rect.contains(p)
            {
                self.view.center_on(minimap_to_world(minimap_rect, p), self.canvas_size);
                true
            } else {
                false
            };

            if !minimap_handled {
                match self.selected_pattern {
                    Some(idx) => {
                        // Placing a pattern: a single click stamps it once.
                        // Ignored outside `map_rect` (the letter/pillarbox
                        // margin isn't part of the map).
                        if response.clicked()
                            && let Some(pointer) = response.interact_pointer_pos()
                            && map_rect.contains(pointer)
                        {
                            let cell = self.view.screen_to_cell(map_rect.min, pointer);
                            let cells = self.library[idx].cells.clone();
                            self.sim.stamp(&cells, cell);
                        }
                    }
                    None if self.tool == Tool::Pan => {
                        // Google Maps-style click-and-drag panning: the
                        // primary button, which Draw/Eraser use for
                        // painting, instead moves the map directly under
                        // the cursor. Not gated to `map_rect` — panning from
                        // the letterbox margin is harmless and still useful.
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
                        // of the Draw tool's toggle/paint-a-trail behavior. Ignored outside
                        // `map_rect` for the same reason as pattern placement above.
                        let erase = self.tool == Tool::Eraser;
                        if response.drag_started() {
                            if let Some(pointer) = response.interact_pointer_pos()
                                && map_rect.contains(pointer)
                            {
                                let cell = self.view.screen_to_cell(map_rect.min, pointer);
                                let value = if erase { false } else { !self.sim.live.contains(&cell) };
                                self.sim.set_cell(cell, value);
                                self.paint_value = Some(value);
                                self.last_paint_cell = Some(cell);
                            }
                        } else if response.dragged() {
                            if let (Some(pointer), Some(value)) = (response.interact_pointer_pos(), self.paint_value)
                                && map_rect.contains(pointer)
                            {
                                let cell = self.view.screen_to_cell(map_rect.min, pointer);
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
                            && map_rect.contains(pointer)
                        {
                            let cell = self.view.screen_to_cell(map_rect.min, pointer);
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
            if response.secondary_clicked() {
                self.selected_pattern = None;
            }

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
            self.view.cell_size = self.view.cell_size.clamp(min_cell_size.min(MAX_CELL_SIZE), MAX_CELL_SIZE);
            self.view.clamp_to_world(self.canvas_size);

            let (min, max) = self.view.visible_bounds(map_rect.size());
            let cs = self.view.cell_size;

            if self.show_grid && cs > 4.0 {
                let stroke = Stroke::new(1.0, Color32::from_gray(35));
                let mut x = min.0;
                while x <= max.0 {
                    let p = self.view.cell_to_screen(map_rect.min, (x, 0));
                    painter.line_segment([Pos2::new(p.x, map_rect.min.y), Pos2::new(p.x, map_rect.max.y)], stroke);
                    x += 1;
                }
                let mut y = min.1;
                while y <= max.1 {
                    let p = self.view.cell_to_screen(map_rect.min, (0, y));
                    painter.line_segment([Pos2::new(map_rect.min.x, p.y), Pos2::new(map_rect.max.x, p.y)], stroke);
                    y += 1;
                }
            }

            for (x, y) in self.sim.cells_in_bounds(min, max) {
                let p = self.view.cell_to_screen(map_rect.min, (x, y));
                painter.rect_filled(
                    Rect::from_min_size(p, Vec2::splat(cs)),
                    0.0,
                    Color32::from_rgb(120, 220, 130),
                );
            }

            // Ghost preview of the pattern in hand, following the cursor.
            if let Some(idx) = self.selected_pattern
                && let Some(pointer) = response.hover_pos()
            {
                let base = self.view.screen_to_cell(map_rect.min, pointer);
                for &(dx, dy) in &self.library[idx].cells {
                    let p = self.view.cell_to_screen(map_rect.min, (base.0 + dx as i64, base.1 + dy as i64));
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
                let cell = self.view.screen_to_cell(map_rect.min, pointer);
                let p = self.view.cell_to_screen(map_rect.min, cell);
                painter.rect_stroke(
                    Rect::from_min_size(p, Vec2::splat(cs)),
                    0.0,
                    Stroke::new(2.0, Color32::from_rgb(220, 90, 90)),
                    StrokeKind::Outside,
                );
            }

            // The plane is finite: draw its edge wherever it's on-screen, so
            // it's clear painting/patterns stop working past this line. At
            // minimum zoom this now coincides exactly with `map_rect`'s
            // border, since both represent the same 16:9 world rectangle.
            let world_screen_min = self.view.cell_to_screen(map_rect.min, WORLD_MIN);
            let world_screen_max = self.view.cell_to_screen(map_rect.min, (WORLD_MAX.0 + 1, WORLD_MAX.1 + 1));
            painter.rect_stroke(
                Rect::from_min_max(world_screen_min, world_screen_max),
                0.0,
                Stroke::new(2.0, Color32::from_rgb(190, 90, 90)),
                StrokeKind::Outside,
            );

            draw_minimap(&self.sim, &self.view, self.canvas_size, &painter, minimap_rect);

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
                .anchor(egui::Align2::LEFT_BOTTOM, Vec2::new(MINIMAP_MARGIN, -MINIMAP_MARGIN))
                .order(egui::Order::Foreground)
                .show(&ctx, |ui| {
                    egui::Frame::popup(ui.style()).inner_margin(6.0).show(ui, |ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(0.0, 4.0);
                        ui.vertical(|ui| {
                            let plus = egui::RichText::new("+").size(18.0).strong();
                            if ui.add_sized(zoom_button_size, egui::Button::new(plus)).on_hover_text("Zoom in").clicked() {
                                self.view.zoom(KEY_ZOOM_STEP, map_rect.size() / 2.0, min_cell_size);
                            }
                            let minus = egui::RichText::new("−").size(18.0).strong();
                            if ui.add_sized(zoom_button_size, egui::Button::new(minus)).on_hover_text("Zoom out").clicked() {
                                self.view.zoom(1.0 / KEY_ZOOM_STEP, map_rect.size() / 2.0, min_cell_size);
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

            self.pattern_library_overlay(&ctx, rect);
        });
    }
}

/// The largest rectangle with the given `aspect` (width / height) that fits
/// centered inside `outer`, letter- or pillar-boxing whichever axis doesn't
/// match. Used to keep the map itself always a true, undistorted rectangle
/// matching the world's own aspect ratio, regardless of the canvas's actual
/// (arbitrary, window-and-panel-dependent) shape.
fn fit_aspect_rect(outer: Rect, aspect: f32) -> Rect {
    let size = if outer.width() / outer.height() > aspect {
        Vec2::new(outer.height() * aspect, outer.height()) // outer is relatively wider: pillarbox
    } else {
        Vec2::new(outer.width(), outer.width() / aspect) // outer is relatively taller: letterbox
    };
    Rect::from_center_size(outer.center(), size)
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
        minimap_rect.min.x + ((vmax.0 + 1).clamp(WORLD_MIN.0, WORLD_MAX.0 + 1) as f32 - WORLD_MIN.0 as f32) * sx,
        minimap_rect.min.y + ((vmax.1 + 1).clamp(WORLD_MIN.1, WORLD_MAX.1 + 1) as f32 - WORLD_MIN.1 as f32) * sy,
    );
    let viewport_rect = Rect::from_min_max(vp_min, vp_max).intersect(minimap_rect);
    painter.rect_stroke(viewport_rect, 0.0, Stroke::new(1.5, Color32::from_rgb(255, 210, 90)), StrokeKind::Outside);

    painter.rect_stroke(minimap_rect, 4.0, Stroke::new(1.0, Color32::from_gray(110)), StrokeKind::Outside);
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

fn paint_pattern_preview(painter: &egui::Painter, rect: Rect, cells: &[(i32, i32)]) {
    painter.rect_filled(rect, 2.0, Color32::from_gray(28));
    if cells.is_empty() {
        return;
    }
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for &(x, y) in cells {
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }
    let w = (max_x - min_x + 1) as f32;
    let h = (max_y - min_y + 1) as f32;
    let pad = 4.0;
    // Fit the pattern's bounding box inside the preview box, shrinking large
    // patterns (e.g. the 36-wide Gosper Glider Gun) instead of forcing at
    // least 1px/cell, which used to make them overflow the tiny icon and
    // look like an unrecognizable blob.
    let scale = ((rect.width() - pad * 2.0) / w).min((rect.height() - pad * 2.0) / h).clamp(0.3, 6.0);
    let origin = rect.min + Vec2::new(pad, pad);
    for &(x, y) in cells {
        let p = origin + Vec2::new((x - min_x) as f32 * scale, (y - min_y) as f32 * scale);
        painter.rect_filled(Rect::from_min_size(p, Vec2::splat(scale.max(1.0))), 0.0, Color32::from_rgb(120, 220, 130));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_aspect_rect_pillarboxes_a_relatively_wide_outer_rect() {
        let outer = Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(1000.0, 500.0)); // 2:1, wider than 16:9
        let fitted = fit_aspect_rect(outer, 16.0 / 9.0);
        assert!((fitted.height() - 500.0).abs() < 1e-3, "height should match the constraining axis");
        assert!((fitted.width() - 500.0 * 16.0 / 9.0).abs() < 1e-3);
        assert!((fitted.center() - outer.center()).length() < 1e-3, "should stay centered in outer");
    }

    #[test]
    fn fit_aspect_rect_letterboxes_a_relatively_tall_outer_rect() {
        let outer = Rect::from_min_size(Pos2::new(5.0, 0.0), Vec2::new(500.0, 1000.0)); // 1:2, taller than 16:9
        let fitted = fit_aspect_rect(outer, 16.0 / 9.0);
        assert!((fitted.width() - 500.0).abs() < 1e-3, "width should match the constraining axis");
        assert!((fitted.height() - 500.0 * 9.0 / 16.0).abs() < 1e-3);
        assert!((fitted.center() - outer.center()).length() < 1e-3, "should stay centered in outer");
    }

    #[test]
    fn fit_aspect_rect_leaves_an_already_matching_rect_untouched() {
        let outer = Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 540.0)); // already exactly 16:9
        let fitted = fit_aspect_rect(outer, 16.0 / 9.0);
        assert!((fitted.width() - outer.width()).abs() < 1e-3);
        assert!((fitted.height() - outer.height()).abs() < 1e-3);
    }
}
