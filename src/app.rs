use eframe::egui::{self, Color32, Key, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

use crate::patterns::{self, Category, Pattern};
use crate::rle;
use crate::rules::{self, Class, RuleSet};
use crate::simulation::{CHUNK_SIZE, Cell, SimState, WORLD_MAX, WORLD_MIN};
use crate::starts;
use crate::view::{self, MAX_CELL_SIZE, View};

/// Zoom factor applied per keyboard/button zoom-shortcut press ('+'/'-').
const KEY_ZOOM_STEP: f32 = 1.2;
/// Screen-pixel-equivalent pan distance per keyboard arrow / pan-button press.
const PAN_STEP: f32 = 60.0;
/// Options for the "generations to skip" selector next to Step.
const SKIP_OPTIONS: &[u32] = &[0, 5, 10, 50, 100, 500, 1000];
/// On-screen size of the minimap box, anchored to the canvas's bottom-right
/// corner with `MINIMAP_MARGIN` of breathing room.
/// Must match the world's aspect ratio (a square) or the map, viewport
/// outline and occupied-chunk markers would be stretched. 128px over 4096
/// cells makes one spatial-index chunk (32 cells) exactly one pixel.
const MINIMAP_SIZE: Vec2 = Vec2::new(128.0, 128.0);
/// Largest span (cells per axis) the Random button fills, so zooming all the
/// way out and pressing it cannot ask for millions of live cells.
const RANDOM_MAX_SPAN: i64 = 1024;
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
    preset_class: Option<Class>,
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
    /// `None` by default — nothing is pre-selected, so "Load" is disabled
    /// until the user deliberately picks one.
    selected_start: Option<usize>,
    /// Rotation count for the currently selected library pattern preview.
    selected_pattern_rotation: u8,
    /// Horizontal mirror toggle for the selected library pattern preview.
    selected_pattern_flip: bool,
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
    /// Whether the pattern-library overlay is shown — starts collapsed, the
    /// "☰" button toggles it. It floats on top of the canvas (an
    /// `egui::Area`, not a side `Panel`) so showing/hiding it never changes
    /// the canvas's own size — the map's aspect ratio stays correct and
    /// full-size either way.
    show_side_panel: bool,
    /// Whether the Simulation/custom-rule control rows are shown below the
    /// always-visible essentials — the "⚙" button toggles this, collapsing
    /// them to reclaim canvas height.
    show_extra_controls: bool,
    /// Short feedback line for clipboard import/export, shown in the top bar.
    status: Option<String>,
    /// Set by the library's Open all / Close all buttons; applied to every
    /// category header on the next frame, then cleared.
    library_force_open: Option<bool>,
}

impl App {
    pub fn new() -> Self {
        let rule = rules::preset_rule(&rules::PRESETS[0]);
        let mut sim = SimState::new(rule);
        // Seed with a glider so the canvas isn't empty on first launch.
        sim.stamp(&rle::cells("bo$2bo$3o!"), (2, 2));

        App {
            sim,
            view: View::default(),
            library: patterns::library_for(&rule),
            selected_pattern: None,
            preset_name: rules::PRESETS[0].name,
            preset_class: Some(rules::PRESETS[0].class),
            random_density: 0.35,
            paint_value: None,
            last_paint_cell: None,
            show_grid: true,
            skip_generations: 0,
            tool: Tool::Draw,
            selected_start: None,
            selected_pattern_rotation: 0,
            selected_pattern_flip: false,
            canvas_size: Vec2::new(800.0, 600.0),
            dragging_minimap: false,
            middle_pan_active: false,
            show_side_panel: false,
            show_extra_controls: true,
            status: None,
            library_force_open: None,
        }
    }

    /// Switches the active rule and everything derived from it: the preset
    /// label, the pattern collection for that rule, and any selection that
    /// only made sense under the old one.
    fn set_rule(&mut self, rule: RuleSet) {
        self.sim.rule = rule;
        let preset = rules::PRESETS.iter().find(|p| rules::preset_rule(p) == rule);
        (self.preset_name, self.preset_class) = preset.map_or(("Custom", None), |p| (p.name, Some(p.class)));
        self.library = patterns::library_for(&rule);
        self.selected_pattern = None;
        self.selected_start = None;
    }

    /// Fills the visible area randomly, capped at `RANDOM_MAX_SPAN` cells per
    /// axis around its center.
    fn randomize_visible(&mut self, canvas: Vec2) {
        let (min, max) = self.view.visible_bounds(canvas);
        let cap = |lo: i64, hi: i64| {
            let mid = (lo + hi) / 2;
            (
                lo.max(mid - RANDOM_MAX_SPAN / 2),
                hi.min(mid + RANDOM_MAX_SPAN / 2),
            )
        };
        let ((min_x, max_x), (min_y, max_y)) = (cap(min.0, max.0), cap(min.1, max.1));
        self.sim
            .randomize((min_x, min_y), (max_x, max_y), self.random_density);
    }

    /// Saves the board as a Golly-format `.rle` file through the native
    /// save dialog.
    fn save_rle_file(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Save board as RLE")
            .add_filter("RLE pattern", &["rle"])
            .set_file_name("pattern.rle")
            .save_file()
        else {
            return;
        };
        let text = rle::to_rle(self.sim.live.iter().copied(), self.sim.rule);
        self.status = Some(match std::fs::write(&path, text) {
            Ok(()) => format!("Saved {} cells to {}", self.sim.live.len(), path.display()),
            Err(e) => format!("Save failed: {e}"),
        });
    }

    /// Opens a Golly-format `.rle` file through the native file picker.
    fn import_rle_file(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Import RLE pattern")
            .add_filter("RLE pattern", &["rle", "txt"])
            .pick_file()
        else {
            return;
        };
        let name = path
            .file_stem()
            .map_or_else(|| "Imported pattern".into(), |s| s.to_string_lossy().into_owned());
        match std::fs::read_to_string(&path) {
            Ok(text) => self.paste(&text, &name),
            Err(e) => self.status = Some(format!("Import failed: {e}")),
        }
    }

    /// Golly interchange: RLE text pasted from the clipboard becomes the
    /// pattern in hand. If its header names a different rule, that rule is
    /// activated first (as Golly does when opening a file).
    fn paste(&mut self, text: &str, name: &str) {
        let rle = match rle::parse(text) {
            Ok(rle) if rle.cells.is_empty() => {
                return self.status = Some("Import failed: no live cells".into());
            }
            Ok(rle) => rle,
            Err(e) => return self.status = Some(format!("Import failed: {e}")),
        };
        if let Some(rule) = rle.rule
            && rule != self.sim.rule
        {
            self.set_rule(rule);
        }
        self.status = Some(format!(
            "Loaded {} cells under {} - click canvas to place",
            rle.cells.len(),
            self.sim.rule.to_bs_string()
        ));
        self.library.retain(|p| p.category != Category::Imported);
        self.library.push(Pattern {
            name: name.into(),
            category: Category::Imported,
            cells: rle.cells,
        });
        self.selected_pattern = Some(self.library.len() - 1);
        self.selected_pattern_rotation = 0;
        self.selected_pattern_flip = false;
        self.tool = Tool::Draw;
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let dt = ctx.input(|i| i.stable_dt);
        let was_running = self.sim.running;
        self.sim.tick(dt);
        if was_running && !self.sim.running {
            self.status = Some(format!(
                "Paused: over {} live cells (Step still works)",
                self.sim.population_limit
            ));
        }
        let pasted = ctx.input(|i| {
            i.events.iter().find_map(|e| {
                if let egui::Event::Paste(t) = e {
                    Some(t.clone())
                } else {
                    None
                }
            })
        });
        if let Some(text) = pasted
            && !ctx.egui_wants_keyboard_input()
        {
            self.paste(&text, "Pasted pattern");
        }
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
                    .selectable_label(self.show_side_panel, "☰ Library")
                    .on_hover_text("Show/hide the pattern library panel")
                    .clicked()
                {
                    self.show_side_panel = !self.show_side_panel;
                }
                if ui
                    .selectable_label(self.show_extra_controls, "⚙ Settings")
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
                if let Some(status) = &self.status {
                    ui.separator();
                    ui.label(egui::RichText::new(status).small().italics());
                }
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
                    .width(180.0)
                    .selected_text(match self.preset_class {
                        Some(class) => format!("{} ({})", self.preset_name, class.label()),
                        None => self.preset_name.to_string(),
                    })
                    .show_ui(ui, |ui| {
                        // Grouped by long-term random-soup behavior instead
                        // of one flat 22-entry list, so a preset's class is
                        // a section header you land in rather than a suffix
                        // you have to read on every single row. Each row
                        // uses `add_sized` at the indented width's full
                        // extent so the whole row is clickable/highlighted,
                        // not just the text itself.
                        for class in Class::ALL {
                            ui.label(egui::RichText::new(class.label()).small().strong());
                            ui.indent(("rule_class_indent", class.label()), |ui| {
                                for preset in rules::PRESETS.iter().filter(|p| p.class == class) {
                                    let width = ui.available_width();
                                    let selected = self.preset_name == preset.name;
                                    let row = egui::Button::selectable(selected, preset.name);
                                    if ui.add_sized([width, 0.0], row).clicked() {
                                        self.set_rule(rules::preset_rule(preset));
                                    }
                                }
                            });
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
                    .selected_text(self.selected_start.map_or("(choose one)", |idx| starts::START_CONFIGS[idx]))
                    .show_ui(ui, |ui| {
                        for (idx, name) in starts::START_CONFIGS.iter().enumerate() {
                            if starts::is_available(name, &self.library) {
                                ui.selectable_value(&mut self.selected_start, Some(idx), *name);
                            }
                        }
                    });
                if ui
                    .add_enabled(self.selected_start.is_some(), egui::Button::new("Load"))
                    .on_hover_text("Clear the board and lay out the selected starting configuration")
                    .clicked()
                    && let Some(idx) = self.selected_start
                {
                    starts::apply(&mut self.sim, starts::START_CONFIGS[idx], &self.library, self.random_density);
                    self.selected_pattern = None;
                    let center = ((WORLD_MIN.0 + WORLD_MAX.0) / 2, (WORLD_MIN.1 + WORLD_MAX.1) / 2);
                    self.view.center_on(center, self.canvas_size);
                }

                ui.separator();
                if ui.button("🗑").on_hover_text("Clear: erase every live cell").clicked() {
                    self.sim.clear();
                }
                if ui
                    .button("Copy RLE")
                    .on_hover_text("Copy the board as Golly-compatible RLE. To bring a pattern in from Golly, copy it there and press Ctrl+V here")
                    .clicked()
                {
                    ui.ctx().copy_text(rle::to_rle(self.sim.live.iter().copied(), self.sim.rule));
                    self.status = Some(format!("Copied {} cells as RLE", self.sim.live.len()));
                }
                if ui
                    .button("Save RLE")
                    .on_hover_text("Save the board as a Golly-format .rle file")
                    .clicked()
                {
                    self.save_rle_file();
                }
                if ui
                    .button("Import RLE")
                    .on_hover_text(
                        "Open a Golly-format .rle file and place it like a library pattern (switches to the rule in its header)",
                    )
                    .clicked()
                {
                    self.import_rle_file();
                }
                if ui
                    .button("🎲")
                    .on_hover_text(format!("Random: fill the visible area (at most {RANDOM_MAX_SPAN}x{RANDOM_MAX_SPAN} cells around its center) at {:.0}% density", self.random_density * 100.0))
                    .clicked()
                {
                    // `canvas_size` (one frame stale, like the zoom controls
                    // above used to be) is the aspect-locked map size, not
                    // this row's own width — using `ui.available_size()`
                    // here would fill by the top bar's width, not the map's.
                    self.randomize_visible(self.canvas_size.max(Vec2::new(400.0, 400.0)));
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
                    // B0 is unsupported (see `RuleSet::from_bs_string`).
                    if ui.add_enabled(n != 0, egui::Checkbox::new(&mut on, n.to_string())).changed() {
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
                    self.set_rule(self.sim.rule);
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
                    // Grow to nearly the full canvas height (the frame's own
                    // padding accounts for the rest).
                    ui.set_max_height(canvas_rect.height() - 2.0 * MINIMAP_MARGIN - 24.0);
                    ui.horizontal(|ui| {
                        ui.heading("Pattern Library");
                        // Right-to-left sub-layout pins the close button to
                        // this row's far right edge (the panel's top-right
                        // corner), rather than immediately after the heading
                        // text.
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .small_button("✖")
                                .on_hover_text("Hide the pattern library")
                                .clicked()
                            {
                                self.show_side_panel = false;
                            }
                        });
                    });
                    match patterns::collection_name(&self.sim.rule) {
                        Some(name) => ui.label(
                            egui::RichText::new(format!("{name} collection"))
                                .small()
                                .italics(),
                        ),
                        None => ui.label(
                            egui::RichText::new(format!(
                                "No built-in patterns for {}. Paste RLE from Golly with Ctrl+V.",
                                self.sim.rule.to_bs_string()
                            ))
                            .small()
                            .italics(),
                        ),
                    };
                    if self.selected_pattern.is_some() {
                        ui.horizontal(|ui| {
                            ui.label("Click canvas to place. ");
                            if ui.button("✖").on_hover_text("Cancel pattern placement").clicked() {
                                self.selected_pattern = None;
                            }
                        });
                    }
                    if !self.library.is_empty() {
                        ui.horizontal(|ui| {
                            if ui.small_button("Open all").clicked() {
                                self.library_force_open = Some(true);
                            }
                            if ui.small_button("Close all").clicked() {
                                self.library_force_open = Some(false);
                            }
                        });
                    }
                    ui.separator();
                    let force_open = self.library_force_open.take();
                    // `auto_shrink([false, true])`: claim the full panel width
                    // (scrollbar flush at the right edge) but only as much
                    // height as the open categories need, up to whatever is
                    // left of the panel's full-canvas-height budget.
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, true])
                        .max_height(ui.available_height())
                        .show(ui, |ui| {
                            for category in Category::ALL {
                                if !self.library.iter().any(|p| p.category == category) {
                                    continue;
                                }
                                egui::CollapsingHeader::new(category.label())
                                    .default_open(false)
                                    .open(force_open)
                                    .show(ui, |ui| {
                                        for (idx, pattern) in self.library.iter().enumerate() {
                                            if pattern.category != category {
                                                continue;
                                            }
                                            let selected = self.selected_pattern == Some(idx);
                                            if pattern_row(ui, selected, &pattern.cells, &pattern.name)
                                                .clicked()
                                            {
                                                self.selected_pattern = Some(idx);
                                                self.tool = Tool::Draw;
                                            }
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
            // The canvas grid adapts to whatever space is actually available
            // (window shape minus the top bar) rather than being locked to
            // the world's own aspect ratio — no letterbox/pillarbox margin
            // is drawn. Cells are still always rendered as perfect squares
            // (`View::cell_size` is one scalar shared by both axes), so
            // nothing gets visually stretched; the axis with more canvas
            // room than the world needs just shows more of it, clamped
            // exactly at the world's edge by `View::clamp_to_world` below
            // (drawn as a red boundary line), same as any other axis.
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

            // All pointer-driven zoom/pan below is gated on the canvas
            // actually being the topmost thing under the pointer right now
            // (egui resolves this per-layer, so it's automatically `false`
            // while the pointer is over the pattern-library overlay or any
            // other floating Area). Unlike click/drag — which egui already
            // routes to only the topmost widget — a pinch gesture or a raw
            // `Event::MouseWheel` isn't tied to a specific widget, so
            // without this check scrolling the pattern library's list would
            // also zoom/pan the map underneath it.
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
                            Key::Minus => {
                                self.view
                                    .zoom(1.0 / KEY_ZOOM_STEP, rect.size() / 2.0, min_cell_size)
                            }
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
            let minimap_rect = Rect::from_min_size(
                rect.max - MINIMAP_SIZE - Vec2::splat(MINIMAP_MARGIN),
                MINIMAP_SIZE,
            );
            if response.drag_started()
                && let Some(p) = response.interact_pointer_pos()
                && minimap_rect.contains(p)
            {
                self.dragging_minimap = true;
            }
            let minimap_handled = if self.dragging_minimap {
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
            };

            if !minimap_handled {
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
                            if let (Some(pointer), Some(value)) =
                                (response.interact_pointer_pos(), self.paint_value)
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
            self.view.cell_size = self
                .view
                .cell_size
                .clamp(min_cell_size.min(MAX_CELL_SIZE), MAX_CELL_SIZE);
            self.view.clamp_to_world(self.canvas_size);

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

            for (x, y) in self.sim.cells_in_bounds(min, max) {
                let p = self.view.cell_to_screen(rect.min, (x, y));
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

            // The plane is finite: draw its edge wherever it's on-screen, so
            // it's clear painting/patterns stop working past this line.
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
                .anchor(
                    egui::Align2::LEFT_BOTTOM,
                    Vec2::new(MINIMAP_MARGIN, -MINIMAP_MARGIN),
                )
                .order(egui::Order::Foreground)
                .show(&ctx, |ui| {
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

            self.pattern_library_overlay(&ctx, rect);
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

/// One row in the pattern library: a preview icon plus a name, where the
/// *entire row* — full available width, not just the small icon or exactly
/// the text — is one hoverable/clickable target. Like a radio button whose
/// reachable area extends across its whole row instead of stopping at the
/// tiny circle, so picking a pattern doesn't need aiming precisely at a
/// narrow icon or a short label.
fn pattern_row(ui: &mut egui::Ui, selected: bool, cells: &[(i32, i32)], name: &str) -> egui::Response {
    let icon_size = 36.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), icon_size), Sense::click());

    let visuals = ui.style().interact_selectable(&response, selected);
    ui.painter()
        .rect_filled(rect, visuals.corner_radius, visuals.weak_bg_fill);

    let icon_rect = Rect::from_min_size(rect.min, Vec2::splat(icon_size));
    paint_pattern_preview(ui.painter(), icon_rect, cells);

    ui.painter().text(
        Pos2::new(icon_rect.max.x + 8.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        name,
        egui::TextStyle::Body.resolve(ui.style()),
        visuals.text_color(),
    );

    response
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
    let scale = ((rect.width() - pad * 2.0) / w)
        .min((rect.height() - pad * 2.0) / h)
        .clamp(0.3, 6.0);
    let origin = rect.min + Vec2::new(pad, pad);
    for &(x, y) in cells {
        let p = origin + Vec2::new((x - min_x) as f32 * scale, (y - min_y) as f32 * scale);
        painter.rect_filled(
            Rect::from_min_size(p, Vec2::splat(scale.max(1.0))),
            0.0,
            Color32::from_rgb(120, 220, 130),
        );
    }
}
