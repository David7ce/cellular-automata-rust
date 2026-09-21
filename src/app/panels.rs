//! The top control bar and the pattern-library overlay.

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Vec2};

use super::{App, MINIMAP_MARGIN, RANDOM_MAX_SPAN, SKIP_OPTIONS, Tool};
use crate::patterns::{self, Category};
use crate::rle;
use crate::rules::{self, Class};
use crate::simulation::{WORLD_MAX, WORLD_MIN};
use crate::starts;

impl App {
    pub(super) fn top_panel(&mut self, ui: &mut egui::Ui) {
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
    pub(super) fn pattern_library_overlay(&mut self, ctx: &egui::Context, canvas_rect: Rect) {
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
