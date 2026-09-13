use rand::RngExt;

use crate::patterns::Pattern;
use crate::simulation::{SimState, WORLD_MAX, WORLD_MIN};

/// Named starting layouts the user can load in one action instead of
/// stamping/painting a fresh board by hand every time. Every option here is
/// a density-based scatter across the whole world rather than a single
/// fixed figure or a hardcoded instance count, so the same density slider
/// that drives "Random soup" also controls how crowded a "Glider field" or
/// "Pulsar field" comes out. There's no "Empty board" entry — clearing the
/// board already has its own dedicated button, and nothing is selected by
/// default (see `App::selected_start`), so loading a start is always a
/// deliberate choice, not an accidental no-op. Reuses the pattern library's
/// cell data rather than duplicating RLE strings here, so a fix to a
/// pattern's shape automatically applies to any start built from it.
pub const START_CONFIGS: &[&str] = &["Random soup", "Glider field", "Gosper gun field", "Pulsar field"];

/// Clears the board, then lays out `name` across the whole world at
/// `density`.
pub fn apply(sim: &mut SimState, name: &str, library: &[Pattern], density: f32) {
    sim.clear();
    match name {
        "Random soup" => sim.randomize(WORLD_MIN, WORLD_MAX, density),
        "Glider field" => scatter(sim, library, "Glider", 24, density),
        "Gosper gun field" => scatter(sim, library, "Gosper Glider Gun", 60, density),
        "Pulsar field" => scatter(sim, library, "Pulsar", 24, density),
        // Any unrecognized name just leaves the cleared board.
        _ => {}
    }
}

/// Scatters copies of `pattern_name` across the whole world on a lattice of
/// candidate centers `spacing` cells apart, stamping one at each candidate
/// independently with probability `density`. `spacing` should be picked per
/// pattern to keep overlap between neighboring instances rare — roughly the
/// pattern's own footprint plus margin for spaceships/guns that grow as
/// they run.
fn scatter(sim: &mut SimState, library: &[Pattern], pattern_name: &str, spacing: i64, density: f32) {
    let Some(pattern) = library.iter().find(|p| p.name == pattern_name) else {
        return;
    };
    let (cx, cy) = bounding_center(&pattern.cells);
    let mut rng = rand::rng();
    let mut x = WORLD_MIN.0;
    while x <= WORLD_MAX.0 {
        let mut y = WORLD_MIN.1;
        while y <= WORLD_MAX.1 {
            if rng.random::<f32>() < density {
                sim.stamp(&pattern.cells, (x - cx, y - cy));
            }
            y += spacing;
        }
        x += spacing;
    }
}

/// Rounds-toward-zero midpoint of a pattern's bounding box, used to center
/// it on a target cell instead of stamping from its RLE-origin corner.
fn bounding_center(cells: &[(i32, i32)]) -> (i64, i64) {
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for &(x, y) in cells {
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }
    (((min_x + max_x) / 2) as i64, ((min_y + max_y) / 2) as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patterns;
    use crate::rules::RuleSet;

    fn empty_sim_and_library() -> (SimState, Vec<Pattern>) {
        (SimState::new(RuleSet::from_counts(&[3], &[2, 3])), patterns::library())
    }

    #[test]
    fn unrecognized_name_still_clears_and_places_nothing() {
        let (mut sim, library) = empty_sim_and_library();
        sim.stamp(&[(0, 0)], (0, 0));
        apply(&mut sim, "not a real start config", &library, 1.0);
        assert!(sim.live.is_empty());
    }

    #[test]
    fn zero_density_scatter_places_nothing() {
        let (mut sim, library) = empty_sim_and_library();
        for name in ["Random soup", "Glider field", "Gosper gun field", "Pulsar field"] {
            apply(&mut sim, name, &library, 0.0);
            assert!(sim.live.is_empty(), "{name} at density 0.0 should place nothing");
        }
    }

    #[test]
    fn full_density_scatter_fills_every_lattice_slot() {
        let (mut sim, library) = empty_sim_and_library();
        for name in ["Glider field", "Gosper gun field", "Pulsar field"] {
            apply(&mut sim, name, &library, 1.0);
            assert!(!sim.live.is_empty(), "{name} at density 1.0 should place something");
        }
    }
}
