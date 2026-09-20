/// A totalistic Life-like rule: how many live neighbors (0-8) are required
/// for a dead cell to be born, and for a live cell to survive.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct RuleSet {
    pub birth: [bool; 9],
    pub survive: [bool; 9],
}

impl RuleSet {
    pub fn from_counts(birth: &[u8], survive: &[u8]) -> Self {
        let mut rule = RuleSet {
            birth: [false; 9],
            survive: [false; 9],
        };
        for &n in birth {
            rule.birth[n as usize] = true;
        }
        for &n in survive {
            rule.survive[n as usize] = true;
        }
        rule
    }

    /// Parses Golly-style rule text: `B3/S23` (any case, either order) or the
    /// legacy `S/B` digit form `23/3`. Rejects, rather than silently
    /// mis-simulating, what this engine cannot run: B0 (the sparse step
    /// only visits cells that already have a live neighbour), the `:T`/`:P`
    /// bounded-grid suffix, `V`/`H` neighbourhoods and Generations rules.
    pub fn from_bs_string(text: &str) -> Result<Self, String> {
        let text = text.trim().to_ascii_lowercase();
        if text.contains(':') {
            return Err("bounded-grid suffix (:T/:P) is not supported".into());
        }
        let parts: Vec<&str> = text.split('/').collect();
        let [first, second] = parts[..] else {
            return Err(format!("expected two parts separated by '/', got \"{text}\""));
        };
        let (birth, survive) = match (
            first.strip_prefix('b'),
            second.strip_prefix('s'),
            first.strip_prefix('s'),
            second.strip_prefix('b'),
        ) {
            (Some(b), Some(s), _, _) => (b, s),
            (_, _, Some(s), Some(b)) => (b, s),
            _ => (second, first), // legacy "S/B", digits only
        };
        let mut rule = RuleSet {
            birth: [false; 9],
            survive: [false; 9],
        };
        for (digits, table) in [(birth, &mut rule.birth), (survive, &mut rule.survive)] {
            for ch in digits.chars() {
                match ch.to_digit(10) {
                    Some(n) if n <= 8 => table[n as usize] = true,
                    _ => return Err(format!("unsupported character '{ch}' in rule \"{text}\"")),
                }
            }
        }
        if rule.birth[0] {
            return Err("B0 rules are not supported".into());
        }
        Ok(rule)
    }

    /// Formats as standard "B.../S..." notation.
    pub fn to_bs_string(self) -> String {
        let b: String = (0..=8)
            .filter(|&n| self.birth[n])
            .map(|n| char::from(b'0' + n as u8))
            .collect();
        let s: String = (0..=8)
            .filter(|&n| self.survive[n])
            .map(|n| char::from(b'0' + n as u8))
            .collect();
        format!("B{b}/S{s}")
    }
}

/// Long-term behavior from random-soup testing (LifeWiki/community consensus).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Class {
    /// Stays turbulent without settling or unbounded growth.
    Chaotic,
    /// Grows without bound.
    Explosive,
    /// Settles into still lifes and oscillators.
    Stable,
}

impl Class {
    pub const ALL: [Class; 3] = [Class::Chaotic, Class::Explosive, Class::Stable];

    pub fn label(self) -> &'static str {
        match self {
            Class::Chaotic => "chaotic",
            Class::Explosive => "explosive",
            Class::Stable => "stable",
        }
    }
}

pub struct Preset {
    pub name: &'static str,
    pub birth: &'static [u8],
    pub survive: &'static [u8],
    pub class: Class,
}

pub const PRESETS: &[Preset] = &[
    Preset {
        name: "Conway's Life",
        birth: &[3],
        survive: &[2, 3],
        class: Class::Stable,
    },
    Preset {
        name: "2x2",
        birth: &[3, 6],
        survive: &[1, 2, 5],
        class: Class::Stable,
    },
    Preset {
        name: "34 Life",
        birth: &[3, 4],
        survive: &[3, 4],
        class: Class::Explosive,
    },
    Preset {
        name: "Assimilation",
        birth: &[3, 4, 5],
        survive: &[4, 5, 6, 7],
        class: Class::Stable,
    },
    Preset {
        name: "Coagulations",
        birth: &[3, 7, 8],
        survive: &[2, 3, 5, 6, 7, 8],
        class: Class::Explosive,
    },
    Preset {
        name: "Coral",
        birth: &[3],
        survive: &[4, 5, 6, 7, 8],
        class: Class::Stable,
    },
    Preset {
        name: "Day & Night",
        birth: &[3, 6, 7, 8],
        survive: &[3, 4, 6, 7, 8],
        class: Class::Stable,
    },
    Preset {
        name: "Diamoeba",
        birth: &[3, 5, 6, 7, 8],
        survive: &[5, 6, 7, 8],
        class: Class::Chaotic,
    },
    Preset {
        name: "Flakes",
        birth: &[3],
        survive: &[0, 1, 2, 3, 4, 5, 6, 7, 8],
        class: Class::Explosive,
    },
    Preset {
        name: "Gnarl",
        birth: &[1],
        survive: &[1],
        class: Class::Explosive,
    },
    Preset {
        name: "HighLife",
        birth: &[3, 6],
        survive: &[2, 3],
        class: Class::Stable,
    },
    Preset {
        name: "Long Life",
        birth: &[3, 4, 5],
        survive: &[5],
        class: Class::Stable,
    },
    Preset {
        name: "Maze",
        birth: &[3],
        survive: &[1, 2, 3, 4, 5],
        class: Class::Explosive,
    },
    Preset {
        name: "Mazectric",
        birth: &[3],
        survive: &[1, 2, 3, 4],
        class: Class::Stable,
    },
    Preset {
        name: "Move",
        birth: &[3, 6, 8],
        survive: &[2, 4, 5],
        class: Class::Stable,
    },
    Preset {
        name: "Pseudo Life",
        birth: &[3, 5, 7],
        survive: &[2, 3, 8],
        class: Class::Chaotic,
    },
    Preset {
        name: "Replicator",
        birth: &[1, 3, 5, 7],
        survive: &[1, 3, 5, 7],
        class: Class::Explosive,
    },
    Preset {
        name: "Seeds",
        birth: &[2],
        survive: &[],
        class: Class::Explosive,
    },
    Preset {
        name: "Serviettes",
        birth: &[2, 3, 4],
        survive: &[],
        class: Class::Explosive,
    },
    Preset {
        name: "Stains",
        birth: &[3, 6, 7, 8],
        survive: &[2, 3, 5, 6, 7, 8],
        class: Class::Stable,
    },
    Preset {
        name: "Walled Cities",
        birth: &[4, 5, 6, 7, 8],
        survive: &[2, 3, 4, 5],
        class: Class::Stable,
    },
];

pub fn preset_rule(preset: &Preset) -> RuleSet {
    RuleSet::from_counts(preset.birth, preset.survive)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_golly_spellings_to_the_same_rule() {
        let life = RuleSet::from_counts(&[3], &[2, 3]);
        for text in ["B3/S23", "b3/s23", " S23/B3 ", "23/3"] {
            assert_eq!(RuleSet::from_bs_string(text), Ok(life), "{text}");
        }
    }

    #[test]
    fn every_preset_round_trips_through_its_string() {
        for preset in PRESETS {
            let rule = preset_rule(preset);
            assert_eq!(
                RuleSet::from_bs_string(&rule.to_bs_string()),
                Ok(rule),
                "{}",
                preset.name
            );
        }
    }

    #[test]
    fn rejects_what_the_engine_cannot_run() {
        for text in [
            "",
            "B3",
            "B3/S23/4",
            "B0/S",
            "B3/S23:T30,20",
            "B3/S23V",
            "B39/S23",
            "B3/Sx",
        ] {
            assert!(
                RuleSet::from_bs_string(text).is_err(),
                "{text:?} should be rejected"
            );
        }
    }

    #[test]
    fn no_preset_uses_b0() {
        assert!(PRESETS.iter().all(|p| !p.birth.contains(&0)));
    }
}
