use bevy::platform::time::Instant;

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use crate::{
    WfcCellBans, WfcCellDebug, WfcContradiction, WfcDebugSnapshot, WfcFailure, WfcFailureReason,
    WfcGlobalConstraint, WfcObservationHeuristic, WfcRequest, WfcSolution, WfcSolveStats,
    WfcTileCountConstraint, WfcTileGrid, WfcTileId, WfcTileVariant,
};

use super::{bitset::DomainBits, grid::CompiledGrid, rules::CompiledRuleset};

#[allow(clippy::result_large_err)]
pub fn solve_wfc(request: &WfcRequest) -> Result<WfcSolution, WfcFailure> {
    Solver::new(request)?.solve()
}

/// A snapshot of one cell's state during step-by-step solving.
#[derive(Clone, Debug)]
pub struct WfcStepCell {
    pub possible_count: u32,
    pub collapsed: Option<WfcTileVariant>,
}

/// A snapshot of the entire grid between observation steps.
#[derive(Clone, Debug)]
pub struct WfcStepSnapshot {
    pub cells: Vec<WfcStepCell>,
    pub observation_count: u32,
    pub last_observed_position: Option<bevy::prelude::UVec3>,
    pub finished: bool,
    pub failed: bool,
}

/// Step-by-step WFC solver that yields intermediate snapshots.
///
/// Create with [`WfcStepSolver::new`], then call [`step`](WfcStepSolver::step)
/// repeatedly until the returned snapshot has `finished == true`.
pub struct WfcStepSolver<'a> {
    inner: Solver<'a>,
    constraints_applied: bool,
}

impl<'a> WfcStepSolver<'a> {
    /// Create a new step solver for the given request.
    #[allow(clippy::result_large_err)]
    pub fn new(request: &'a WfcRequest) -> Result<Self, WfcFailure> {
        Ok(Self {
            inner: Solver::new(request)?,
            constraints_applied: false,
        })
    }

    /// Perform one observation+propagation step and return the current state.
    ///
    /// Returns `Ok(snapshot)` with `snapshot.finished == true` when the solve
    /// is complete, or `Err(failure)` if a contradiction cannot be recovered.
    #[allow(clippy::result_large_err)]
    pub fn step(&mut self) -> Result<WfcStepSnapshot, WfcFailure> {
        if !self.constraints_applied {
            self.constraints_applied = true;
            if let Err(reason) = self.inner.apply_input_constraints() {
                return Err(self.inner.finish_failure(reason));
            }
        }

        if self.inner.possible_counts.iter().all(|count| *count == 1) {
            return Ok(self.snapshot(true, false));
        }

        let Some(cell) = self.inner.select_observation_cell() else {
            return Ok(self.snapshot(true, false));
        };

        let mut ordered_choices = self.inner.weighted_choice_order(cell);
        let Some(chosen_tile) = ordered_choices.pop() else {
            self.inner
                .record_contradiction(cell, "selected cell has no remaining tiles");
            return Err(self.inner.finish_failure(SolveError::Contradiction));
        };

        self.inner.stats.observation_count = self.inner.stats.observation_count.saturating_add(1);
        self.inner.last_observed = Some(cell);
        self.inner.decisions.push(DecisionFrame {
            trail_len: self.inner.trail.len(),
            cell,
            alternatives: ordered_choices,
        });

        if self
            .inner
            .assign_single(cell, chosen_tile)
            .and_then(|_| self.inner.propagate())
            .is_err()
        {
            match self.inner.backtrack() {
                Ok(()) => {}
                Err(reason) => return Err(self.inner.finish_failure(reason)),
            }
        }

        let finished = self.inner.possible_counts.iter().all(|count| *count == 1);
        Ok(self.snapshot(finished, false))
    }

    /// Finish the solver and produce the final `WfcSolution`.
    ///
    /// Only valid to call after a step returned `finished == true`.
    #[allow(clippy::result_large_err)]
    pub fn finish(mut self) -> Result<WfcSolution, WfcFailure> {
        if self.inner.possible_counts.iter().all(|count| *count == 1) {
            Ok(self.inner.finish_solution())
        } else {
            Err(self.inner.finish_failure(SolveError::Contradiction))
        }
    }

    fn snapshot(&self, finished: bool, failed: bool) -> WfcStepSnapshot {
        let cells = self
            .inner
            .domains
            .iter()
            .map(|domain| {
                let count = domain.count() as u32;
                let collapsed = if count == 1 {
                    domain
                        .first_one()
                        .map(|index| self.inner.rules.tile_variant(index))
                } else {
                    None
                };
                WfcStepCell {
                    possible_count: count,
                    collapsed,
                }
            })
            .collect();

        WfcStepSnapshot {
            cells,
            observation_count: self.inner.stats.observation_count,
            last_observed_position: self
                .inner
                .last_observed
                .map(|index| self.inner.grid.position_of(index)),
            finished,
            failed,
        }
    }

    /// Get the current tile id for a collapsed cell at a given flat index.
    pub fn tile_at(&self, index: usize) -> Option<WfcTileId> {
        let domain = self.inner.domains.get(index)?;
        if domain.count() == 1 {
            domain.first_one().map(|i| self.inner.rules.tile_id(i))
        } else {
            None
        }
    }
}

#[derive(Clone)]
struct CompiledTileCountConstraint {
    tile_mask: DomainBits,
    min_count: Option<u32>,
    max_count: Option<u32>,
}

#[derive(Clone)]
struct TrailEntry {
    cell: usize,
    previous_domain: DomainBits,
    previous_count: u16,
}

#[derive(Clone)]
struct DecisionFrame {
    trail_len: usize,
    cell: usize,
    alternatives: Vec<usize>,
}

enum SolveError {
    InvalidRequest(String),
    Contradiction,
    BacktrackLimitReached,
    UnsatisfiedGlobalConstraint,
}

struct Solver<'a> {
    request: &'a WfcRequest,
    grid: CompiledGrid,
    rules: CompiledRuleset,
    domains: Vec<DomainBits>,
    possible_counts: Vec<u16>,
    queue: Vec<usize>,
    trail: Vec<TrailEntry>,
    decisions: Vec<DecisionFrame>,
    global_constraints: Vec<CompiledTileCountConstraint>,
    rng: ChaCha8Rng,
    stats: WfcSolveStats,
    last_observed: Option<usize>,
    last_contradiction: Option<WfcContradiction>,
    started_at: Instant,
}

impl<'a> Solver<'a> {
    #[allow(clippy::result_large_err)]
    fn new(request: &'a WfcRequest) -> Result<Self, WfcFailure> {
        let grid = CompiledGrid::new(
            request.ruleset.topology,
            request.grid_size,
            request.boundary_stitching,
        )
        .map_err(|message| invalid_failure(request, message))?;
        let rules = CompiledRuleset::compile(&request.ruleset)
            .map_err(|message| invalid_failure(request, message))?;

        if rules.topology() != grid.topology() {
            return Err(invalid_failure(
                request,
                "request grid and ruleset topology do not match".to_string(),
            ));
        }

        let full_domain = rules.full_domain();
        let domains = vec![full_domain.clone(); grid.total_cells()];
        let possible_counts = vec![rules.tile_count() as u16; grid.total_cells()];
        let global_constraints = compile_global_constraints(request, &rules)?;

        Ok(Self {
            request,
            grid,
            rules,
            domains,
            possible_counts,
            queue: Vec::new(),
            trail: Vec::new(),
            decisions: Vec::new(),
            global_constraints,
            rng: ChaCha8Rng::seed_from_u64(request.seed.0),
            stats: WfcSolveStats::default(),
            last_observed: None,
            last_contradiction: None,
            started_at: Instant::now(),
        })
    }

    #[allow(clippy::result_large_err)]
    fn solve(mut self) -> Result<WfcSolution, WfcFailure> {
        if let Err(reason) = self.apply_input_constraints() {
            return Err(self.finish_failure(reason));
        }

        loop {
            if self.possible_counts.iter().all(|count| *count == 1) {
                return Ok(self.finish_solution());
            }

            let Some(cell) = self.select_observation_cell() else {
                return Ok(self.finish_solution());
            };

            let mut ordered_choices = self.weighted_choice_order(cell);
            let Some(chosen_tile) = ordered_choices.pop() else {
                self.record_contradiction(cell, "selected cell has no remaining tiles");
                return Err(self.finish_failure(SolveError::Contradiction));
            };

            self.stats.observation_count = self.stats.observation_count.saturating_add(1);
            self.last_observed = Some(cell);
            self.decisions.push(DecisionFrame {
                trail_len: self.trail.len(),
                cell,
                alternatives: ordered_choices,
            });

            if self
                .assign_single(cell, chosen_tile)
                .and_then(|_| self.propagate())
                .is_err()
            {
                match self.backtrack() {
                    Ok(()) => {}
                    Err(reason) => return Err(self.finish_failure(reason)),
                }
            }
        }
    }

    fn apply_input_constraints(&mut self) -> Result<(), SolveError> {
        for fixed in &self.request.fixed_cells {
            let Some(cell) = self.grid.index_of(fixed.position) else {
                return Err(self.invalid_contradiction(format!(
                    "fixed cell {:?} is outside the requested grid",
                    fixed.position
                )));
            };
            let allowed_tiles = self
                .rules
                .mask_for_tiles(&[fixed.tile])
                .map_err(|message| self.invalid_contradiction(message))?;
            self.intersect_domain(cell, &allowed_tiles, "fixed cell assignment")?;
        }

        for bans in &self.request.banned_cells {
            self.apply_bans(bans)?;
        }

        for border in &self.request.border_constraints {
            let allowed = self
                .rules
                .mask_for_tiles(&border.allowed_tiles)
                .map_err(|message| self.invalid_contradiction(message))?;
            for cell in 0..self.grid.total_cells() {
                if self.grid.is_on_border(cell, border.border) {
                    self.intersect_domain(cell, &allowed, "border constraint")?;
                }
            }
        }

        self.propagate()
    }

    fn apply_bans(&mut self, bans: &WfcCellBans) -> Result<(), SolveError> {
        let Some(cell) = self.grid.index_of(bans.position) else {
            return Err(self.invalid_contradiction(format!(
                "banned cell {:?} is outside the requested grid",
                bans.position
            )));
        };
        let banned_mask = self
            .rules
            .mask_for_tiles(&bans.banned_tiles)
            .map_err(|message| self.invalid_contradiction(message))?;
        let restricted = self.domains[cell].difference(&banned_mask);
        self.restrict_domain(cell, restricted, "per-cell tile bans")
    }

    fn propagate(&mut self) -> Result<(), SolveError> {
        while let Some(cell) = self.queue.pop() {
            let source_domain = self.domains[cell].clone();
            for &direction in self.grid.directions() {
                let Some(neighbor) = self.grid.neighbor(cell, direction) else {
                    continue;
                };

                let mut supported = DomainBits::empty(self.rules.tile_count());
                for tile_index in source_domain.iter_ones() {
                    supported.or_assign(self.rules.allowed_mask(direction, tile_index));
                }

                let mut next_neighbor = self.domains[neighbor].clone();
                if next_neighbor.and_assign(&supported) {
                    self.stats.propagation_count = self.stats.propagation_count.saturating_add(1);
                    self.apply_domain_change(
                        neighbor,
                        next_neighbor,
                        "adjacency propagation removed unsupported tiles",
                    )?;
                }
            }
        }

        self.check_global_constraints()
    }

    fn select_observation_cell(&mut self) -> Option<usize> {
        let mut best_cell = None;
        let mut best_score = f32::INFINITY;
        let mut ties = 0u32;

        for cell in 0..self.domains.len() {
            if self.possible_counts[cell] <= 1 {
                continue;
            }

            let score = match self.request.settings.observation_heuristic {
                WfcObservationHeuristic::MinimumRemainingValues => {
                    self.possible_counts[cell] as f32
                }
                WfcObservationHeuristic::MinimumEntropy => self.entropy(cell),
            };

            if score + 1e-6 < best_score {
                best_score = score;
                best_cell = Some(cell);
                ties = 1;
            } else if (score - best_score).abs() <= 1e-6 {
                ties = ties.saturating_add(1);
                if self.rng.random_range(0..ties) == 0 {
                    best_cell = Some(cell);
                }
            }
        }

        best_cell
    }

    fn weighted_choice_order(&mut self, cell: usize) -> Vec<usize> {
        let mut candidates: Vec<usize> = self.domains[cell].iter_ones().collect();
        let mut ordered = Vec::with_capacity(candidates.len());

        while !candidates.is_empty() {
            let total_weight: f32 = candidates
                .iter()
                .map(|index| self.rules.weight(*index))
                .sum();
            let mut target = self.rng.random::<f32>() * total_weight.max(f32::EPSILON);
            let mut picked_slot = candidates.len().saturating_sub(1);
            for (slot, tile_index) in candidates.iter().copied().enumerate() {
                target -= self.rules.weight(tile_index);
                if target <= 0.0 {
                    picked_slot = slot;
                    break;
                }
            }
            ordered.push(candidates.remove(picked_slot));
        }

        ordered.reverse();
        ordered
    }

    fn assign_single(&mut self, cell: usize, tile_index: usize) -> Result<(), SolveError> {
        let singleton = DomainBits::singleton(self.rules.tile_count(), tile_index);
        self.intersect_domain(cell, &singleton, "observation collapsed a cell")
    }

    fn restrict_domain(
        &mut self,
        cell: usize,
        next_domain: DomainBits,
        note: &str,
    ) -> Result<(), SolveError> {
        if next_domain == self.domains[cell] {
            return Ok(());
        }
        self.apply_domain_change(cell, next_domain, note)
    }

    fn intersect_domain(
        &mut self,
        cell: usize,
        mask: &DomainBits,
        note: &str,
    ) -> Result<(), SolveError> {
        let mut next_domain = self.domains[cell].clone();
        next_domain.and_assign(mask);
        self.restrict_domain(cell, next_domain, note)
    }

    fn apply_domain_change(
        &mut self,
        cell: usize,
        next_domain: DomainBits,
        note: &str,
    ) -> Result<(), SolveError> {
        let next_count = next_domain.count() as u16;
        self.trail.push(TrailEntry {
            cell,
            previous_domain: self.domains[cell].clone(),
            previous_count: self.possible_counts[cell],
        });
        self.domains[cell] = next_domain;
        self.possible_counts[cell] = next_count;
        self.queue.push(cell);

        if next_count == 0 {
            self.record_contradiction(cell, note);
            return Err(SolveError::Contradiction);
        }

        Ok(())
    }

    fn backtrack(&mut self) -> Result<(), SolveError> {
        while let Some(mut frame) = self.decisions.pop() {
            self.rollback(frame.trail_len);
            while let Some(next_tile) = frame.alternatives.pop() {
                self.stats.backtrack_count = self.stats.backtrack_count.saturating_add(1);
                if self.stats.backtrack_count > self.request.settings.max_backtracks {
                    return Err(SolveError::BacktrackLimitReached);
                }

                self.rollback(frame.trail_len);
                self.last_observed = Some(frame.cell);
                self.stats.observation_count = self.stats.observation_count.saturating_add(1);
                self.decisions.push(DecisionFrame {
                    trail_len: frame.trail_len,
                    cell: frame.cell,
                    alternatives: frame.alternatives.clone(),
                });

                if self
                    .assign_single(frame.cell, next_tile)
                    .and_then(|_| self.propagate())
                    .is_ok()
                {
                    return Ok(());
                }

                self.decisions.pop();
            }
        }

        Err(SolveError::Contradiction)
    }

    fn rollback(&mut self, target_len: usize) {
        self.queue.clear();
        while self.trail.len() > target_len {
            if let Some(entry) = self.trail.pop() {
                self.domains[entry.cell] = entry.previous_domain;
                self.possible_counts[entry.cell] = entry.previous_count;
            }
        }
    }

    fn check_global_constraints(&mut self) -> Result<(), SolveError> {
        for constraint in &self.global_constraints {
            let guaranteed = self
                .domains
                .iter()
                .filter(|domain| domain.is_singleton() && domain.intersects(&constraint.tile_mask))
                .count() as u32;
            let possible = self
                .domains
                .iter()
                .filter(|domain| domain.intersects(&constraint.tile_mask))
                .count() as u32;

            if constraint.max_count.is_some_and(|max| guaranteed > max) {
                let target = self.last_observed.unwrap_or(0);
                self.record_contradiction(target, "tile-count maximum exceeded");
                return Err(SolveError::UnsatisfiedGlobalConstraint);
            }
            if constraint.min_count.is_some_and(|min| possible < min) {
                let target = self.last_observed.unwrap_or(0);
                self.record_contradiction(target, "tile-count minimum became unreachable");
                return Err(SolveError::UnsatisfiedGlobalConstraint);
            }
        }

        Ok(())
    }

    fn entropy(&self, cell: usize) -> f32 {
        if self.possible_counts[cell] <= 1 {
            return 0.0;
        }
        let mut weight_sum = 0.0f32;
        let mut weight_log_sum = 0.0f32;
        for tile_index in self.domains[cell].iter_ones() {
            let weight = self.rules.weight(tile_index);
            weight_sum += weight;
            weight_log_sum += weight * weight.ln();
        }
        weight_sum.ln() - weight_log_sum / weight_sum
    }

    fn record_contradiction(&mut self, cell: usize, note: &str) {
        self.stats.contradiction_count = self.stats.contradiction_count.saturating_add(1);
        let remaining_candidates = self.domains[cell]
            .iter_ones()
            .map(|index| self.rules.tile_id(index))
            .collect();
        let remaining_variants = self.domains[cell]
            .iter_ones()
            .map(|index| self.rules.tile_variant(index))
            .collect();
        self.last_contradiction = Some(WfcContradiction {
            position: self.grid.position_of(cell),
            last_observed_cell: self.last_observed.map(|index| self.grid.position_of(index)),
            remaining_candidates,
            remaining_variants,
            decision_depth: self.decisions.len() as u32,
            note: note.to_string(),
        });
    }

    fn invalid_contradiction(&mut self, message: String) -> SolveError {
        self.last_contradiction = None;
        SolveError::InvalidRequest(message)
    }

    fn finish_solution(&mut self) -> WfcSolution {
        self.stats.elapsed_ms = self.started_at.elapsed().as_secs_f32() * 1000.0;
        let tiles = self
            .domains
            .iter()
            .map(|domain| {
                self.rules
                    .tile_id(domain.first_one().expect("solved domains are set"))
            })
            .collect::<Vec<_>>();
        let rotations = self
            .domains
            .iter()
            .map(|domain| {
                self.rules
                    .tile_rotation(domain.first_one().expect("solved domains are set"))
            })
            .collect::<Vec<_>>();
        let grid = WfcTileGrid {
            topology: self.grid.topology(),
            size: self.grid.size(),
            tiles,
            rotations,
        };
        let signature = grid.signature();
        let debug = self
            .request
            .settings
            .capture_debug_snapshot
            .then(|| self.build_debug_snapshot());
        WfcSolution {
            seed: self.request.seed,
            grid,
            stats: self.stats.clone(),
            debug,
            signature,
        }
    }

    fn finish_failure(&mut self, reason: SolveError) -> WfcFailure {
        self.stats.elapsed_ms = self.started_at.elapsed().as_secs_f32() * 1000.0;
        let failure_reason = match reason {
            SolveError::InvalidRequest(_) => WfcFailureReason::InvalidRequest,
            SolveError::Contradiction => WfcFailureReason::Contradiction,
            SolveError::BacktrackLimitReached => WfcFailureReason::BacktrackLimitReached,
            SolveError::UnsatisfiedGlobalConstraint => {
                WfcFailureReason::UnsatisfiedGlobalConstraint
            }
        };
        let message = match reason {
            SolveError::InvalidRequest(message) => message,
            SolveError::Contradiction => {
                "solver reached a contradiction and exhausted all backtracking choices".to_string()
            }
            SolveError::BacktrackLimitReached => format!(
                "solver exceeded the configured backtrack budget of {}",
                self.request.settings.max_backtracks
            ),
            SolveError::UnsatisfiedGlobalConstraint => {
                if let Some(contradiction) = &self.last_contradiction {
                    contradiction.note.clone()
                } else {
                    "a global or authoring constraint could not be satisfied".to_string()
                }
            }
        };
        let debug = self
            .request
            .settings
            .capture_debug_snapshot
            .then(|| self.build_debug_snapshot());
        WfcFailure {
            reason: failure_reason,
            seed: self.request.seed,
            topology: self.grid.topology(),
            grid_size: self.grid.size(),
            stats: self.stats.clone(),
            contradiction: self.last_contradiction.clone(),
            debug,
            message,
        }
    }
}

#[allow(clippy::result_large_err)]
fn compile_global_constraints(
    request: &WfcRequest,
    rules: &CompiledRuleset,
) -> Result<Vec<CompiledTileCountConstraint>, WfcFailure> {
    let mut compiled = Vec::new();
    for constraint in &request.global_constraints {
        match constraint {
            WfcGlobalConstraint::TileCount(WfcTileCountConstraint {
                tile,
                min_count,
                max_count,
            }) => {
                if let (Some(min), Some(max)) = (min_count, max_count)
                    && min > max
                {
                    return Err(invalid_failure(
                        request,
                        format!("tile-count constraint for {:?} has min > max", tile),
                    ));
                }
                let tile_mask = rules
                    .mask_for_tiles(&[*tile])
                    .map_err(|message| invalid_failure(request, message))?;
                compiled.push(CompiledTileCountConstraint {
                    tile_mask,
                    min_count: *min_count,
                    max_count: *max_count,
                });
            }
        }
    }
    Ok(compiled)
}

fn invalid_failure(request: &WfcRequest, message: String) -> WfcFailure {
    WfcFailure {
        reason: WfcFailureReason::InvalidRequest,
        seed: request.seed,
        topology: request.ruleset.topology,
        grid_size: request.grid_size,
        stats: WfcSolveStats::default(),
        contradiction: None,
        debug: None,
        message,
    }
}
impl<'a> Solver<'a> {
    fn build_debug_snapshot(&self) -> WfcDebugSnapshot {
        let cells = self
            .domains
            .iter()
            .enumerate()
            .map(|(cell, domain)| {
                let possible_variants = domain
                    .iter_ones()
                    .map(|index| self.rules.tile_variant(index))
                    .collect::<Vec<_>>();
                let possible_tiles = domain
                    .iter_ones()
                    .map(|index| self.rules.tile_id(index))
                    .collect::<Vec<_>>();
                let collapsed_variant = domain
                    .first_one()
                    .map(|index| self.rules.tile_variant(index));
                let collapsed_tile = collapsed_variant.map(|variant| variant.tile);
                WfcCellDebug {
                    position: self.grid.position_of(cell),
                    possible_tiles: dedupe_tile_ids(possible_tiles),
                    possible_variants,
                    possible_count: self.possible_counts[cell] as u32,
                    entropy: self.entropy(cell),
                    collapsed_tile,
                    collapsed_variant,
                }
            })
            .collect();

        WfcDebugSnapshot {
            cells,
            last_observed_cell: self.last_observed.map(|index| self.grid.position_of(index)),
            contradiction: self.last_contradiction.clone(),
        }
    }
}

fn dedupe_tile_ids(tile_ids: Vec<crate::WfcTileId>) -> Vec<crate::WfcTileId> {
    let mut deduped = Vec::new();
    for tile_id in tile_ids {
        if !deduped.contains(&tile_id) {
            deduped.push(tile_id);
        }
    }
    deduped
}
