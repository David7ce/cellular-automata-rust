use crate::rle;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Category {
    StillLife,
    Oscillator,
    Spaceship,
    Gun,
    Methuselah,
}

impl Category {
    pub const ALL: [Category; 5] = [
        Category::StillLife,
        Category::Oscillator,
        Category::Spaceship,
        Category::Gun,
        Category::Methuselah,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Category::StillLife => "Still Lifes",
            Category::Oscillator => "Oscillators",
            Category::Spaceship => "Spaceships",
            Category::Gun => "Guns",
            Category::Methuselah => "Methuselahs",
        }
    }
}

pub struct Pattern {
    pub name: &'static str,
    pub category: Category,
    pub cells: Vec<(i32, i32)>,
}

struct Def {
    name: &'static str,
    category: Category,
    rle: &'static str,
}

const DEFS: &[Def] = &[
    // Still lifes
    Def { name: "Block", category: Category::StillLife, rle: "2o$2o!" },
    Def { name: "Beehive", category: Category::StillLife, rle: "b2ob$o2bo$b2ob!" },
    Def { name: "Loaf", category: Category::StillLife, rle: "b2ob$o2bo$bobo$2bo!" },
    Def { name: "Boat", category: Category::StillLife, rle: "2ob$obo$bo!" },
    Def { name: "Tub", category: Category::StillLife, rle: "bob$obo$bo!" },
    Def { name: "Ship", category: Category::StillLife, rle: "2ob$obo$b2o!" },
    Def { name: "Pond", category: Category::StillLife, rle: "b2ob$o2bo$o2bo$b2o!" },
    Def { name: "Barge", category: Category::StillLife, rle: "bo2b$obob$bobo$2bo!" },
    Def { name: "Long Boat", category: Category::StillLife, rle: "bo2b$obob$bobo$2b2o!" },
    // Oscillators
    Def { name: "Blinker", category: Category::Oscillator, rle: "3o!" },
    Def { name: "Toad", category: Category::Oscillator, rle: "b3o$3o!" },
    Def { name: "Beacon", category: Category::Oscillator, rle: "2o$2o$2b2o$2b2o!" },
    Def { name: "Clock", category: Category::Oscillator, rle: "2bob$obob$bobo$bo!" },
    Def {
        name: "Pulsar",
        category: Category::Oscillator,
        rle: "2b3o3b3o2b2$o4bobo4bo$o4bobo4bo$o4bobo4bo$2b3o3b3o2b2$2b3o3b3o2b$o4bobo4bo$o4bobo4bo$o4bobo4bo2$2b3o3b3o!",
    },
    Def { name: "Pentadecathlon", category: Category::Oscillator, rle: "2bo4bo2b$2ob4ob2o$2bo4bo2b!" },
    Def {
        name: "Queen Bee Shuttle",
        category: Category::Oscillator,
        rle: "9bo12b$7bobo12b$6bobo13b$2o3bo2bo11b2o$2o4bobo11b2o$7bobo12b$9bo!",
    },
    Def { name: "Figure Eight", category: Category::Oscillator, rle: "2o4b$2obo2b$4bob$bo4b$2bob2o$4b2o!" },
    Def {
        name: "Kok's Galaxy",
        category: Category::Oscillator,
        rle: "2bo2bobob$2obob3ob$bo6bo$2o5bob2$bo5b2o$o6bob$b3obob2o$bobo2bo!",
    },
    // Spaceships
    Def { name: "Glider", category: Category::Spaceship, rle: "bo$2bo$3o!" },
    Def { name: "Lightweight Spaceship", category: Category::Spaceship, rle: "bo2bo$o$o3bo$4o!" },
    Def { name: "Middleweight Spaceship", category: Category::Spaceship, rle: "3bo2b$bo3bo$o5b$o4bo$5o!" },
    Def { name: "Heavyweight Spaceship", category: Category::Spaceship, rle: "3b2o2b$bo4bo$o6b$o5bo$6o!" },
    Def {
        name: "Loafer",
        category: Category::Spaceship,
        rle: "b2o2bob2o$o2bo2b2o$bobo$2bo$8bo$6b3o$5bo$6bo$7b2o!",
    },
    Def {
        name: "Copperhead",
        category: Category::Spaceship,
        rle: "b2o2b2o$3b2o$3b2o$obo2bobo$o6bo2$o6bo$b2o2b2o$2b4o2$3b2o$3b2o!",
    },
    // Guns
    Def {
        name: "Gosper Glider Gun",
        category: Category::Gun,
        rle: "24bo11b$22bobo11b$12b2o6b2o12b2o$11bo3bo4b2o12b2o$2o8bo5bo3b2o14b$2o8bo3bob2o4bobo11b$10bo5bo7bo11b$11bo3bo20b$12b2o!",
    },
    Def {
        name: "Simkin Glider Gun",
        category: Category::Gun,
        rle: "2o5b2o$2o5b2o2$4b2o$4b2o5$22b2ob2o$21bo5bo$21bo6bo2b2o$21b3o3bo3b2o$26bo4$20b2o$20bo$21b3o$23bo!",
    },
    // Methuselahs
    Def { name: "R-pentomino", category: Category::Methuselah, rle: "b2o$2o$bo!" },
    Def { name: "Diehard", category: Category::Methuselah, rle: "6bo$2o6b$bo3b3o!" },
    Def { name: "Acorn", category: Category::Methuselah, rle: "bo5b$3bo3b$2o2b3o!" },
    Def { name: "B-heptomino", category: Category::Methuselah, rle: "ob2o$3ob$bo!" },
    Def { name: "Pi-heptomino", category: Category::Methuselah, rle: "3o$obo$obo!" },
    Def { name: "Rabbits", category: Category::Methuselah, rle: "o3b3o$3o2bob$bo!" },
];

pub fn library() -> Vec<Pattern> {
    DEFS.iter()
        .map(|def| Pattern {
            name: def.name,
            category: def.category,
            cells: rle::parse(def.rle),
        })
        .collect()
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

        let lib = library();
        assert_eq!(lib.len(), expected.len(), "DEFS and the expected-population table drifted apart");
        for &(name, count) in expected {
            let pattern = lib.iter().find(|p| p.name == name).unwrap_or_else(|| panic!("missing pattern {name}"));
            assert_eq!(pattern.cells.len(), count, "{name} population mismatch");
        }
    }
}
