use super::{Category, Def};

pub(super) const DEFS: &[Def] = &[
    Def { name: "Block", category: Category::StillLife, rle: "2o$2o!" },
    Def { name: "Hex Ring", category: Category::StillLife, rle: "bo$o2bo$o2bo$bo!" },
    Def { name: "S-tetromino", category: Category::Oscillator, rle: "b2o$2o!" },
    Def { name: "P-pentomino", category: Category::Oscillator, rle: "3o$2o!" },
];
