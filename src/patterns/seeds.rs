use super::{Category, Def};

// Seeds has no surviving cells, so nothing here is a still life: every
// pattern is rebuilt from births each generation.
pub(super) const DEFS: &[Def] = &[
    Def {
        name: "Diagonal Pair",
        category: Category::Oscillator,
        rle: "bo$o!",
    },
    Def {
        name: "Gap Trio",
        category: Category::Oscillator,
        rle: "2obo!",
    },
    Def {
        name: "Row of Four",
        category: Category::Oscillator,
        rle: "4o!",
    },
    Def {
        name: "Sliver",
        category: Category::Spaceship,
        rle: "b2o$3bo$o!",
    },
];
