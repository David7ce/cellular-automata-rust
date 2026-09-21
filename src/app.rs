use eframe::egui::{self, Vec2};

use crate::patterns::{self, Category, Pattern};
use crate::rle;
use crate::rules::{self, Class, RuleSet};
use crate::simulation::{Cell, SimState};
use crate::view::View;

mod canvas;
mod panels;

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

/// Application state and the egui glue: `canvas` handles input and drawing on
/// the map, `panels` the top bar and pattern library.
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
