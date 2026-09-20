use super::{Category, Def};

pub(super) const DEFS: &[Def] = &[
    Def { name: "Block", category: Category::StillLife, rle: "2o$2o!" },
    Def { name: "Beehive", category: Category::StillLife, rle: "b2ob$o2bo$b2ob!" },
    Def { name: "Blinker", category: Category::Oscillator, rle: "3o!" },
    Def { name: "Toad", category: Category::Oscillator, rle: "b3o$3o!" },
    Def { name: "Glider", category: Category::Spaceship, rle: "bo$2bo$3o!" },
    Def { name: "Lightweight Spaceship", category: Category::Spaceship, rle: "bo2bo$o$o3bo$4o!" },
    Def { name: "Replicator", category: Category::Replicator, rle: "2b3o$bo2bo$o3bo$o2bo$3o!" },
];
