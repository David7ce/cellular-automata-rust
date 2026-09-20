use std::collections::{HashMap, HashSet};

use rand::RngExt;

use crate::rules::RuleSet;

/// A location on the plane. Currently 2D (x, y); see the module-level "3D
/// migration path" note below for what widening this to (x, y, z) would
/// touch.
pub type Cell = (i64, i64);

/// The 2D plane is finite, not infinite: cells outside these bounds can
/// never be painted, stamped, randomized, or born into. This bounds memory
/// and per-generation cost even under explosive rules, and gives the
/// minimap a fixed world to draw. Existing live cells are always within
/// bounds already (nothing can insert one outside), so `next_generation`
/// only needs to filter birth candidates, not survivors.
///
/// Sized to 960x480, a 2:1 rectangle — wider than a plain 16:9 (1.78) to
/// better match the canvas's *actual* usable shape once the top bar's
/// height is subtracted from a typical window (a wider-than-16:9 area, not
/// a pure 16:9 one), which cuts down how much of the canvas ends up as
/// unused pillarbox margin either side of the map.
/// Also chosen to be an exact multiple of the max-zoom cell size
/// (`view::MAX_CELL_SIZE` = 60px divides it evenly on both axes: 16 cells
/// wide, 8 cells tall), so the world's edge lines up cleanly with the grid
/// at max zoom instead of clipping a partial cell. The *minimum* zoom
/// (zoomed all the way out) is dynamic rather than a fixed divisor of this
/// size — see `view::min_cell_size_to_fit_world`, which always shows the
/// whole map regardless of window size.
pub const WORLD_MIN: Cell = (-480, -240);
pub const WORLD_MAX: Cell = (479, 239);

pub fn in_world(cell: Cell) -> bool {
    cell.0 >= WORLD_MIN.0 && cell.0 <= WORLD_MAX.0 && cell.1 >= WORLD_MIN.1 && cell.1 <= WORLD_MAX.1
}

/// Side length (in cells) of one spatial-index chunk. Chosen to be a bit
/// larger than a typical zoomed-in viewport in cells, so a visible-bounds
/// query touches only a handful of chunks.
pub const CHUNK_SIZE: i64 = 32;

fn chunk_of(cell: Cell) -> (i64, i64) {
    (cell.0.div_euclid(CHUNK_SIZE), cell.1.div_euclid(CHUNK_SIZE))
}

pub struct SimState {
    pub live: HashSet<Cell>,
    /// Spatial index mirroring `live`, bucketed by chunk, so rendering can
    /// query only the cells near the viewport instead of scanning every
    /// live cell every frame.
    chunks: HashMap<(i64, i64), HashSet<Cell>>,
    pub rule: RuleSet,
    pub running: bool,
    /// Generations per second.
    pub speed: f32,
    pub generation: u64,
    /// Cells born / that died over the most recent `step()` (or, after
    /// `step_n`, summed across that whole batch).
    pub last_births: u64,
    pub last_deaths: u64,
    accumulator: f32,
}

impl SimState {
    pub fn new(rule: RuleSet) -> Self {
        SimState {
            live: HashSet::new(),
            chunks: HashMap::new(),
            rule,
            running: false,
            speed: 8.0,
            generation: 0,
            last_births: 0,
            last_deaths: 0,
            accumulator: 0.0,
        }
    }

    fn insert_cell(&mut self, cell: Cell) {
        if !in_world(cell) {
            return;
        }
        if self.live.insert(cell) {
            self.chunks.entry(chunk_of(cell)).or_default().insert(cell);
        }
    }

    fn remove_cell(&mut self, cell: Cell) -> bool {
        if !self.live.remove(&cell) {
            return false;
        }
        let key = chunk_of(cell);
        if let Some(bucket) = self.chunks.get_mut(&key) {
            bucket.remove(&cell);
            if bucket.is_empty() {
                self.chunks.remove(&key);
            }
        }
        true
    }

    fn rebuild_chunks(&mut self) {
        self.chunks.clear();
        for &cell in &self.live {
            self.chunks.entry(chunk_of(cell)).or_default().insert(cell);
        }
    }

    pub fn toggle_cell(&mut self, cell: Cell) {
        if !self.remove_cell(cell) {
            self.insert_cell(cell);
        }
    }

    pub fn set_cell(&mut self, cell: Cell, alive: bool) {
        if alive {
            self.insert_cell(cell);
        } else {
            self.remove_cell(cell);
        }
    }

    pub fn stamp(&mut self, cells: &[(i32, i32)], at: Cell) {
        for &(dx, dy) in cells {
            self.insert_cell((at.0 + dx as i64, at.1 + dy as i64));
        }
    }

    pub fn clear(&mut self) {
        self.live.clear();
        self.chunks.clear();
        self.generation = 0;
        self.last_births = 0;
        self.last_deaths = 0;
        self.accumulator = 0.0;
    }

    pub fn randomize(&mut self, min: Cell, max: Cell, density: f32) {
        let min = (min.0.max(WORLD_MIN.0), min.1.max(WORLD_MIN.1));
        let max = (max.0.min(WORLD_MAX.0), max.1.min(WORLD_MAX.1));
        if min.0 > max.0 || min.1 > max.1 {
            return;
        }
        let mut rng = rand::rng();
        for x in min.0..=max.0 {
            for y in min.1..=max.1 {
                if rng.random::<f32>() < density {
                    self.insert_cell((x, y));
                }
            }
        }
    }

    /// Chunk coordinates that currently contain at least one live cell —
    /// a coarse, `O(occupied chunks)` overview of where activity is on the
    /// whole (potentially huge) plane, used by the minimap instead of
    /// iterating every live cell.
    pub fn occupied_chunks(&self) -> impl Iterator<Item = (i64, i64)> + '_ {
        self.chunks.keys().copied()
    }

    /// Live cells within `[min, max]` (inclusive), for rendering only the
    /// visible viewport instead of the whole (potentially huge) live set.
    pub fn cells_in_bounds(&self, min: Cell, max: Cell) -> Vec<Cell> {
        let mut out = Vec::new();
        let (min_cx, min_cy) = chunk_of(min);
        let (max_cx, max_cy) = chunk_of(max);
        for cx in min_cx..=max_cx {
            for cy in min_cy..=max_cy {
                if let Some(bucket) = self.chunks.get(&(cx, cy)) {
                    out.extend(
                        bucket
                            .iter()
                            .copied()
                            .filter(|&(x, y)| x >= min.0 && x <= max.0 && y >= min.1 && y <= max.1),
                    );
                }
            }
        }
        out
    }

    pub fn step(&mut self) {
        let next = next_generation(&self.live, &self.rule);
        // One pass: next.len() == live.len() + births - deaths.
        let births = next.iter().filter(|c| !self.live.contains(c)).count();
        self.last_births = births as u64;
        self.last_deaths = (self.live.len() + births - next.len()) as u64;
        self.live = next;
        self.rebuild_chunks();
        self.generation += 1;
    }

    /// Advances `n` generations at once (n.max(1)), reporting the total
    /// births/deaths across the whole batch in `last_births`/`last_deaths`
    /// rather than just the final generation's.
    pub fn step_n(&mut self, n: u32) {
        let mut total_births = 0u64;
        let mut total_deaths = 0u64;
        for _ in 0..n.max(1) {
            self.step();
            total_births += self.last_births;
            total_deaths += self.last_deaths;
        }
        self.last_births = total_births;
        self.last_deaths = total_deaths;
    }

    /// Advances the simulation according to elapsed time, if running.
    pub fn tick(&mut self, dt: f32) {
        if !self.running || self.speed <= 0.0 {
            return;
        }
        self.accumulator += dt * self.speed;
        let due = self.accumulator as u32;
        let steps = due.min(MAX_STEPS_PER_TICK);
        for _ in 0..steps {
            self.step();
        }
        self.accumulator -= steps as f32;
        if due > MAX_STEPS_PER_TICK {
            // Can't keep up (huge board, or a long stall): drop the backlog
            // instead of freezing the window trying to drain it.
            self.accumulator = self.accumulator.fract();
        }
    }
}

/// Upper bound on generations run by one `tick`, so the UI stays responsive
/// when the requested speed outruns the machine.
const MAX_STEPS_PER_TICK: u32 = 8;

/// The 8 Moore-neighborhood offsets around a 2D cell. Pulled out as a named
/// constant (rather than an inline nested loop) so a future 3D build can
/// swap in the 26-offset 3D Moore neighborhood (`dx/dy/dz in -1..=1`, minus
/// the origin) here alone — `next_generation` itself, `RuleSet` (already
/// generic over "how many neighbors", 0-8 today), and every other consumer
/// of `Cell` would be unaffected by that change.
const NEIGHBOR_OFFSETS: [(i64, i64); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];

pub(crate) fn next_generation(live: &HashSet<Cell>, rule: &RuleSet) -> HashSet<Cell> {
    let mut counts: HashMap<Cell, u8> = HashMap::with_capacity(live.len() * 4);
    for &(x, y) in live {
        for (dx, dy) in NEIGHBOR_OFFSETS {
            *counts.entry((x + dx, y + dy)).or_insert(0) += 1;
        }
    }

    counts
        .into_iter()
        .filter(|&(cell, n)| {
            let alive_next = if live.contains(&cell) {
                rule.survive[n as usize]
            } else {
                rule.birth[n as usize]
            };
            // Existing live cells are always in-world already; this only
            // ever excludes birth candidates just outside the boundary.
            alive_next && in_world(cell)
        })
        .map(|(cell, _)| cell)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rle;

    fn life() -> SimState {
        SimState::new(RuleSet::from_counts(&[3], &[2, 3]))
    }

    fn sim_with(rle_text: &str) -> SimState {
        let mut sim = life();
        sim.stamp(&rle::cells(rle_text), (0, 0));
        sim
    }

    #[test]
    fn rule_is_generic_seeds_keeps_nothing_it_did_not_just_birth() {
        let mut seeds = SimState::new(RuleSet::from_counts(&[2], &[]));
        seeds.stamp(&[(0, 0), (1, 0)], (0, 0));
        let before = seeds.live.clone();
        seeds.step();
        assert!(seeds.live.is_disjoint(&before));
        assert!(!seeds.live.is_empty());
    }

    #[test]
    fn highlife_b6_differs_from_conway() {
        // Ring of 6 live neighbours around an empty centre.
        let ring = [(0, 0), (1, 0), (2, 0), (0, 1), (2, 1), (0, 2)];
        let (mut conway, mut highlife) = (life(), SimState::new(RuleSet::from_counts(&[3, 6], &[2, 3])));
        conway.stamp(&ring, (0, 0));
        highlife.stamp(&ring, (0, 0));
        conway.step();
        highlife.step();
        assert!(!conway.live.contains(&(1, 1)) && highlife.live.contains(&(1, 1)));
    }

    #[test]
    fn nothing_is_born_outside_the_world_and_edges_clip() {
        let mut sim = life();
        sim.stamp(&[(0, 0), (1, 0), (2, 0)], (WORLD_MAX.0 - 1, 0)); // blinker across the east edge
        assert!(sim.live.iter().all(|&c| in_world(c)));
        sim.step_n(5);
        assert!(sim.live.iter().all(|&c| in_world(c)));
    }

    #[test]
    fn step_n_matches_repeated_step_and_zero_advances_one() {
        let (mut a, mut b) = (sim_with("b2o$2o$bo!"), sim_with("b2o$2o$bo!"));
        a.step_n(7);
        for _ in 0..7 {
            b.step();
        }
        assert_eq!((a.live, a.generation), (b.live, b.generation));

        let mut c = sim_with("3o!");
        c.step_n(0);
        assert_eq!(c.generation, 1);
    }

    #[test]
    fn step_reports_births_and_deaths() {
        let mut blinker = sim_with("3o!");
        blinker.step();
        assert_eq!((blinker.last_births, blinker.last_deaths), (2, 2));
        let mut glider = sim_with("bo$2bo$3o!");
        glider.step();
        assert_eq!(
            glider.live.len() as u64,
            5 + glider.last_births - glider.last_deaths
        );
    }

    #[test]
    fn tick_does_a_bounded_amount_of_work_per_call() {
        let mut sim = sim_with("2o$2o!");
        sim.running = true;
        sim.speed = 60.0;
        sim.tick(1000.0);
        assert_eq!(sim.generation, MAX_STEPS_PER_TICK as u64);
        sim.tick(0.0); // backlog was dropped, not carried over
        assert_eq!(sim.generation, MAX_STEPS_PER_TICK as u64);
    }
}
