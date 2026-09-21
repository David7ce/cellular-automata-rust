//! Pattern collections, one per ruleset: a pattern only appears in the library
//! while a rule it is known to work under is active.

use crate::rle;
use crate::rules::RuleSet;

mod daynight;
mod highlife;
mod life;
mod seeds;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Category {
    StillLife,
    Oscillator,
    Spaceship,
    Gun,
    Methuselah,
    Replicator,
    /// Pasted from the clipboard (e.g. copied out of Golly); never in a built-in collection.
    Imported,
}

impl Category {
    pub const ALL: [Category; 7] = [
        Category::StillLife,
        Category::Oscillator,
        Category::Spaceship,
        Category::Gun,
        Category::Methuselah,
        Category::Replicator,
        Category::Imported,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Category::StillLife => "Still Lifes",
            Category::Oscillator => "Oscillators",
            Category::Spaceship => "Spaceships",
            Category::Gun => "Guns",
            Category::Methuselah => "Methuselahs",
            Category::Replicator => "Replicators",
            Category::Imported => "Imported (clipboard)",
        }
    }
}

pub struct Pattern {
    pub name: String,
    pub category: Category,
    pub cells: Vec<(i32, i32)>,
}

struct Def {
    name: &'static str,
    category: Category,
    rle: &'static str,
}

struct Collection {
    name: &'static str,
    /// Golly-style rule string; the collection is shown while this rule is active.
    rule: &'static str,
    defs: &'static [Def],
}

const COLLECTIONS: &[Collection] = &[
    Collection {
        name: "Conway's Life",
        rule: "B3/S23",
        defs: life::DEFS,
    },
    Collection {
        name: "HighLife",
        rule: "B36/S23",
        defs: highlife::DEFS,
    },
    Collection {
        name: "Seeds",
        rule: "B2/S",
        defs: seeds::DEFS,
    },
    Collection {
        name: "Day & Night",
        rule: "B3678/S34678",
        defs: daynight::DEFS,
    },
];

fn collection_for(rule: &RuleSet) -> Option<&'static Collection> {
    COLLECTIONS
        .iter()
        .find(|c| RuleSet::from_bs_string(c.rule).is_ok_and(|r| r == *rule))
}

/// The built-in patterns for `rule` — empty when the rule has no collection.
pub fn library_for(rule: &RuleSet) -> Vec<Pattern> {
    collection_for(rule)
        .map(|c| {
            c.defs
                .iter()
                .map(|d| Pattern {
                    name: d.name.to_string(),
                    category: d.category,
                    cells: rle::cells(d.rle),
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn collection_name(rule: &RuleSet) -> Option<&'static str> {
    collection_for(rule).map(|c| c.name)
}

pub fn transform_cells(cells: &[(i32, i32)], turns: u8, flip_x: bool) -> Vec<(i32, i32)> {
    let turns = turns % 4;
    cells
        .iter()
        .map(|&(x, y)| {
            let (x, y) = match turns {
                0 => (x, y),
                1 => (y, -x),
                2 => (-x, -y),
                3 => (-y, x),
                _ => unreachable!(),
            };
            let x = if flip_x { -x } else { x };
            (x, y)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::{Behavior, classify};
    use crate::simulation::{CellSet, next_generation};
    use std::collections::HashSet;

    #[test]
    fn selected_pattern_transform_flips_and_rotates_cells_consistently() {
        let cells = [(0, 0), (1, 0), (0, 1)];
        let transformed = transform_cells(&cells, 1, true);
        assert_eq!(transformed, vec![(0, 0), (0, -1), (-1, 0)]);
    }

    /// Every pattern's cell count checked against its well-known population
    /// (a LifeWiki-documented fact for each), independent of the exact
    /// layout of the RLE string. This caught a real bug: "Boat" was
    /// previously defined with the 6-cell "Ship" shape instead of its own
    /// 5-cell shape.
    #[test]
    fn known_population_counts() {
        let expected: &[(&str, usize)] = &[
            ("Block", 4),
            ("Beehive", 6),
            ("Loaf", 7),
            ("Boat", 5),
            ("Tub", 4),
            ("Ship", 6),
            ("Pond", 8),
            ("Barge", 6),
            ("Long Boat", 7),
            ("Blinker", 3),
            ("Toad", 6),
            ("Beacon", 8),
            ("Clock", 6),
            ("Pulsar", 48),
            ("Pentadecathlon", 12),
            ("Queen Bee Shuttle", 20),
            ("Figure Eight", 12),
            ("Kok's Galaxy", 28),
            ("Glider", 5),
            ("Lightweight Spaceship", 9),
            ("Middleweight Spaceship", 11),
            ("Heavyweight Spaceship", 13),
            ("Loafer", 20),
            ("Copperhead", 28),
            ("Gosper Glider Gun", 36),
            ("Simkin Glider Gun", 36),
            ("R-pentomino", 5),
            ("Diehard", 7),
            ("Acorn", 7),
            ("B-heptomino", 7),
            ("Pi-heptomino", 7),
            ("Rabbits", 9),
        ];

        let lib = library_for(&RuleSet::from_bs_string("B3/S23").unwrap());
        assert_eq!(
            lib.len(),
            expected.len(),
            "DEFS and the expected-population table drifted apart"
        );
        for &(name, count) in expected {
            let pattern = lib
                .iter()
                .find(|p| p.name == name)
                .unwrap_or_else(|| panic!("missing pattern {name}"));
            assert_eq!(pattern.cells.len(), count, "{name} population mismatch");
        }
    }

    /// The reason collections are per-rule: each entry is simulated under its
    /// own collection's rule and must really be what its category claims.
    #[test]
    fn every_collection_pattern_behaves_as_its_category_claims() {
        for collection in COLLECTIONS {
            let rule = RuleSet::from_bs_string(collection.rule).unwrap();
            for pattern in library_for(&rule) {
                let (bound, ok): (u32, fn(&Behavior) -> bool) = match pattern.category {
                    Category::StillLife => (200, |b| *b == Behavior::StillLife),
                    Category::Oscillator => (200, |b| matches!(b, Behavior::Oscillator { .. })),
                    Category::Spaceship => (200, |b| matches!(b, Behavior::Spaceship { .. })),
                    // Long-running: must still be changing after 100 generations.
                    Category::Gun | Category::Methuselah | Category::Replicator => {
                        (100, |b| *b == Behavior::Unsettled)
                    }
                    Category::Imported => panic!("built-in collections have no imported patterns"),
                };
                let behavior = classify(&pattern.cells, &rule, bound);
                assert!(
                    ok(&behavior),
                    "{} / {}: {behavior:?} is not {:?}",
                    collection.name,
                    pattern.name,
                    pattern.category
                );
            }
        }
    }

    #[test]
    fn guns_keep_growing() {
        for collection in COLLECTIONS {
            let rule = RuleSet::from_bs_string(collection.rule).unwrap();
            for pattern in library_for(&rule).iter().filter(|p| p.category == Category::Gun) {
                let mut live: CellSet = pattern.cells.iter().map(|&(x, y)| (x as i64, y as i64)).collect();
                let mut populations = Vec::new();
                for generation in 1..=240 {
                    live = next_generation(&live, &rule);
                    if generation == 120 || generation == 240 {
                        populations.push(live.len());
                    }
                }
                let (early, late) = (populations[0], populations[1]);
                assert!(
                    late > early,
                    "{} / {} stopped emitting",
                    collection.name,
                    pattern.name
                );
            }
        }
    }

    #[test]
    fn collections_are_keyed_by_a_valid_unique_rule_and_a_rule_without_one_is_empty() {
        let mut seen = HashSet::new();
        for c in COLLECTIONS {
            let rule = RuleSet::from_bs_string(c.rule).unwrap_or_else(|e| panic!("{}: {e}", c.name));
            assert!(seen.insert(rule.to_bs_string()), "duplicate rule {}", c.rule);
            assert!(!c.defs.is_empty(), "{} has no patterns", c.name);
            assert_eq!(collection_name(&rule), Some(c.name));
        }
        assert!(library_for(&RuleSet::from_bs_string("B345/S5").unwrap()).is_empty());
    }

    /// The one HighLife pattern with no Conway analogue: after 12 generations it
    /// is two whole copies of itself (each a rotation/reflection of the
    /// original), while the same cells under Conway never do that.
    #[test]
    fn highlife_replicator_becomes_two_copies_after_12_generations() {
        type Shape = Vec<(i64, i64)>;
        fn shape_up_to_symmetry(cells: &[(i64, i64)]) -> Shape {
            (0..8)
                .map(|t| {
                    let mut v: Shape = cells
                        .iter()
                        .map(|&(x, y)| {
                            let (x, y) = if t & 4 != 0 { (y, x) } else { (x, y) };
                            (if t & 1 != 0 { -x } else { x }, if t & 2 != 0 { -y } else { y })
                        })
                        .collect();
                    let (mx, my) = (
                        v.iter().map(|c| c.0).min().unwrap(),
                        v.iter().map(|c| c.1).min().unwrap(),
                    );
                    v.iter_mut().for_each(|c| *c = (c.0 - mx, c.1 - my));
                    v.sort();
                    v
                })
                .min()
                .unwrap()
        }
        /// 8-connected components of `live`.
        fn components(live: &CellSet) -> Vec<Shape> {
            let mut left = live.clone();
            let mut out = Vec::new();
            while let Some(&start) = left.iter().next() {
                let (mut stack, mut part) = (vec![start], Vec::new());
                left.remove(&start);
                while let Some((x, y)) = stack.pop() {
                    part.push((x, y));
                    for (dx, dy) in [
                        (-1, -1),
                        (0, -1),
                        (1, -1),
                        (-1, 0),
                        (1, 0),
                        (-1, 1),
                        (0, 1),
                        (1, 1),
                    ] {
                        if left.remove(&(x + dx, y + dy)) {
                            stack.push((x + dx, y + dy));
                        }
                    }
                }
                out.push(part);
            }
            out
        }
        let original: Shape = rle::cells("2b3o$bo2bo$o3bo$o2bo$3o!")
            .iter()
            .map(|&(x, y)| (x as i64, y as i64))
            .collect();
        let step = |rule: &str, generations: u32| {
            let rule = RuleSet::from_bs_string(rule).unwrap();
            let mut live: CellSet = original.iter().copied().collect();
            for _ in 0..generations {
                live = next_generation(&live, &rule);
            }
            live
        };

        let copies = components(&step("B36/S23", 12));
        assert_eq!(copies.len(), 2);
        assert!(
            copies
                .iter()
                .all(|c| shape_up_to_symmetry(c) == shape_up_to_symmetry(&original))
        );
        let conway = components(&step("B3/S23", 12));
        assert!(
            conway.len() != 2
                || conway
                    .iter()
                    .any(|c| shape_up_to_symmetry(c) != shape_up_to_symmetry(&original))
        );
    }
}
