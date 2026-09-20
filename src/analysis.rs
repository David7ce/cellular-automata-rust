//! Behaviour classification by bounded simulation — used to verify that every
//! library pattern really is what its category claims under its own rule.

use std::collections::{HashMap, HashSet};

use crate::rules::RuleSet;
use crate::simulation::{Cell, next_generation};

#[derive(Debug, PartialEq, Eq)]
pub enum Behavior {
    Dies,
    StillLife,
    Oscillator { period: u32 },
    /// Returns to its own shape translated by `(dx, dy)` after `period` generations.
    Spaceship { period: u32, dx: i64, dy: i64 },
    /// No repeat within the generation bound (guns, methuselahs, replicators, chaos).
    Unsettled,
}

/// Shape of `cells` translated so its bounding box starts at the origin,
/// plus the translation that was removed.
fn normalize(cells: &HashSet<Cell>) -> (Vec<Cell>, Cell) {
    let min_x = cells.iter().map(|c| c.0).min().unwrap_or(0);
    let min_y = cells.iter().map(|c| c.1).min().unwrap_or(0);
    let mut shape: Vec<Cell> = cells.iter().map(|&(x, y)| (x - min_x, y - min_y)).collect();
    shape.sort_unstable();
    (shape, (min_x, min_y))
}

pub fn classify(start: &[(i32, i32)], rule: &RuleSet, max_generations: u32) -> Behavior {
    let mut live: HashSet<Cell> = start.iter().map(|&(x, y)| (x as i64, y as i64)).collect();
    let mut seen: HashMap<Vec<Cell>, (u32, Cell)> = HashMap::new();
    for generation in 0..=max_generations {
        if live.is_empty() {
            return Behavior::Dies;
        }
        let (shape, origin) = normalize(&live);
        if let Some(&(first, first_origin)) = seen.get(&shape) {
            let (dx, dy) = (origin.0 - first_origin.0, origin.1 - first_origin.1);
            let period = generation - first;
            return match (dx, dy) {
                (0, 0) if period == 1 => Behavior::StillLife,
                (0, 0) => Behavior::Oscillator { period },
                _ => Behavior::Spaceship { period, dx, dy },
            };
        }
        seen.insert(shape, (generation, origin));
        live = next_generation(&live, rule);
    }
    Behavior::Unsettled
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rle;

    fn life() -> RuleSet {
        RuleSet::from_counts(&[3], &[2, 3])
    }

    fn classify_rle(text: &str) -> Behavior {
        classify(&rle::cells(text), &life(), 100)
    }

    #[test]
    fn conway_basics() {
        assert_eq!(classify_rle("2o$2o!"), Behavior::StillLife);
        assert_eq!(classify_rle("3o!"), Behavior::Oscillator { period: 2 });
        assert_eq!(classify_rle("bo$2bo$3o!"), Behavior::Spaceship { period: 4, dx: 1, dy: 1 });
        assert_eq!(classify_rle("o!"), Behavior::Dies);
        assert_eq!(classify_rle("b2o$2o$bo!"), Behavior::Unsettled); // R-pentomino runs > 100 gens
    }
}
