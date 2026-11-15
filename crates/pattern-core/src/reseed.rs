//! Try again with a different seed until the layout is good enough.
//!
//! The spec's answer to a failed quality gate is a reseed and a retry, and a
//! gate that fails repeatedly is what fetches a human. This is that loop, in
//! one place, because there are two callers who must not disagree: a buyer
//! pressing *shuffle*, and an order assembling itself with nobody watching.
//! Left to themselves they would each invent a retry policy and the unattended
//! one would drift from the one a person actually saw.
//!
//! # It returns the best attempt rather than nothing
//!
//! A run that never clears the bar still hands back its closest effort, with
//! [`Reseeded::acceptable`] false. Returning an error would throw away the
//! information that matters, which is how close it got and in which direction it
//! failed, and an unattended pipeline needs something concrete to escalate
//! with. "Could not do it" is a worse message than "here is the best of eight,
//! still showing a corridor".
//!
//! # Coverage outranks tracking
//!
//! A layout missing a motif the child asked for is the *wrong product*. A
//! layout with a visible corridor is an ugly one. So attempts are compared on
//! coverage first and tracking only to break the tie, and a covered layout is
//! preferred even when an uncovered one looks better.

use glam::Vec2;

use crate::lattice::Lattice;
use crate::scatter::{scatter_with_outcome, ScatterOutcome, ScatterParams};
use crate::tracking::{alignment, WORTH_A_LOOK};

/// How hard to try, and what counts as good enough.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReseedLimits {
    /// Seeds to try before giving up and returning the best so far.
    ///
    /// Each one is a full scatter, so this is the cost knob as well as the
    /// quality one.
    pub attempts: u32,
    /// Reject a layout whose [`alignment`] is above this.
    pub alignment_above: f32,
}

impl Default for ReseedLimits {
    fn default() -> Self {
        Self {
            attempts: 8,
            alignment_above: WORTH_A_LOOK,
        }
    }
}

/// The layout that was chosen, and what it cost to find it.
#[derive(Debug, Clone, PartialEq)]
pub struct Reseeded {
    pub outcome: ScatterOutcome,
    /// The seed that produced it, so the result can be reproduced exactly.
    pub seed: u64,
    pub alignment: f32,
    /// How many seeds were tried, including the one that was kept.
    pub attempts: u32,
    /// Whether it met the bar, or is merely the best of a bad lot.
    ///
    /// False is the signal to fetch a person. It is deliberately not an error:
    /// see the module documentation.
    pub acceptable: bool,
}

/// Scatter repeatedly, keeping the first layout that passes or the best that
/// does not.
///
/// Seeds walk upward from `params.seed`, so the whole run is reproducible from
/// the seed the caller set, which is what lets a buyer be shown a pattern
/// now and sent the identical one at print time.
#[must_use]
pub fn scatter_reseeding(
    params: &ScatterParams,
    tile_size_mm: Vec2,
    lattice: &Lattice,
    limits: ReseedLimits,
) -> Reseeded {
    // Zero attempts is a caller mistake rather than a request for nothing; one
    // scatter is the least that can answer the question asked.
    let attempts = limits.attempts.max(1);
    let mut best: Option<Reseeded> = None;

    for i in 0..attempts {
        let mut attempt = params.clone();
        attempt.seed = params.seed.wrapping_add(u64::from(i));

        let outcome = scatter_with_outcome(&attempt, tile_size_mm, lattice);
        let score = alignment(&outcome.placements, tile_size_mm, lattice);
        let acceptable = outcome.unplaced_required.is_empty() && score <= limits.alignment_above;

        let candidate = Reseeded {
            outcome,
            seed: attempt.seed,
            alignment: score,
            attempts: i + 1,
            acceptable,
        };

        if acceptable {
            return candidate;
        }

        // Coverage first, tracking only to break the tie.
        let better = match &best {
            None => true,
            Some(current) => {
                let missing = candidate.outcome.unplaced_required.len();
                let had = current.outcome.unplaced_required.len();
                missing < had || (missing == had && candidate.alignment < current.alignment)
            }
        };
        if better {
            best = Some(candidate);
        }
    }

    let mut chosen = best.expect("at least one attempt is always made");
    // The count is how many were tried in total, not which one won.
    chosen.attempts = attempts;
    chosen
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mirror::Mirror;
    use crate::scatter::{scatter_with_outcome, MotifWeight};

    const TILE: Vec2 = Vec2::new(300.0, 300.0);

    /// A mix whose rare motifs are required and do not always fit.
    fn params(seed: u64) -> ScatterParams {
        let rare = |id: &str| MotifWeight {
            motif_id: id.into(),
            weight: 1.0,
            required: true,
            natural_size_mm: Vec2::new(60.0, 60.0),
            radius_fraction: 1.0,
        };
        // Chosen by measurement, not taste. Over the eight seeds these tests
        // walk, this mix gives four layouts that pass, three that lose a
        // required motif and one that tracks too hard, so both kinds of
        // failure are live and success is still reachable. At 96 mm and
        // density 0.5 every seed failed coverage and the retry was untestable.
        ScatterParams {
            density: 0.35,
            rotation_jitter_deg: 0.0,
            scale_range: (1.0, 1.0),
            prevent_overlap: true,
            mirror: Mirror::None,
            on_axis_share: 0.0,
            motif_mix: vec![
                MotifWeight {
                    motif_id: "star".into(),
                    weight: 30.0,
                    required: false,
                    natural_size_mm: Vec2::new(22.0, 22.0),
                    radius_fraction: 1.0,
                },
                rare("a"),
                rare("b"),
                rare("c"),
                rare("d"),
            ],
            seed,
        }
    }

    fn lattice() -> Lattice {
        Lattice::block(TILE)
    }

    /// The seeds a run starting here would walk, and what each produced.
    fn walk(from: u64, n: u32) -> Vec<(u64, usize, f32)> {
        (0..n)
            .map(|i| {
                let mut p = params(from.wrapping_add(u64::from(i)));
                p.seed = from.wrapping_add(u64::from(i));
                let o = scatter_with_outcome(&p, TILE, &lattice());
                let a = alignment(&o.placements, TILE, &lattice());
                (p.seed, o.unplaced_required.len(), a)
            })
            .collect()
    }

    #[test]
    fn a_seed_that_fails_coverage_is_retried() {
        // Started at a seed that *does* fail, which is the whole test. Begun
        // at one that happens to pass, this stays green with retrying removed
        // altogether. It did, and only a mutation check noticed.
        let first = scatter_with_outcome(&params(4), TILE, &lattice());
        assert!(
            !first.unplaced_required.is_empty(),
            "seed 4 no longer loses a required motif, so nothing is retried here"
        );

        let got = scatter_reseeding(&params(4), TILE, &lattice(), ReseedLimits::default());
        assert!(got.outcome.unplaced_required.is_empty(), "{got:?}");
        assert!(got.acceptable);
        assert_ne!(got.seed, 4, "kept the seed that failed");
        assert!(got.attempts > 1);
    }

    #[test]
    fn one_attempt_cannot_retry() {
        // The companion to the test above: with no retries allowed, the first
        // seed is kept whatever it produced.
        let limits = ReseedLimits {
            attempts: 1,
            ..ReseedLimits::default()
        };
        let got = scatter_reseeding(&params(1), TILE, &lattice(), limits);
        let first = scatter_with_outcome(&params(1), TILE, &lattice());
        assert_eq!(got.outcome.unplaced_required, first.unplaced_required);
        assert_eq!(got.seed, 1);
        assert_eq!(got.attempts, 1);
    }

    #[test]
    fn an_impossible_bar_returns_the_best_attempt_rather_than_nothing() {
        // Nothing can clear an alignment bar below zero, so every attempt
        // fails and the loop has to choose. It must hand back a layout, say it
        // is not acceptable, and report the full cost.
        let limits = ReseedLimits {
            attempts: 6,
            alignment_above: -1.0,
        };
        let got = scatter_reseeding(&params(1), TILE, &lattice(), limits);

        assert!(!got.acceptable);
        assert_eq!(got.attempts, 6);
        assert!(!got.outcome.placements.is_empty());

        let walked = walk(1, 6);
        let fewest = walked.iter().map(|(_, m, _)| *m).min().expect("six seeds");
        assert_eq!(
            got.outcome.unplaced_required.len(),
            fewest,
            "a layout with more missing motifs was preferred"
        );

        // Among those with the fewest missing, the least tracking wins.
        let best_alignment = walked
            .iter()
            .filter(|(_, m, _)| *m == fewest)
            .map(|(_, _, a)| *a)
            .fold(f32::INFINITY, f32::min);
        assert!(
            (got.alignment - best_alignment).abs() < 1e-6,
            "chose alignment {} when {best_alignment} was available at the \
             same coverage",
            got.alignment
        );
    }

    #[test]
    fn coverage_is_preferred_over_tracking() {
        // The ordering that matters, and it needs a window where the two
        // criteria actually disagree. Seeds 3 and 4 of this fixture do: the
        // first is fully covered and tracks badly, the second loses a motif
        // and looks far better. Most windows contain no such pair, because a seed
        // that is best on both is common, which is why this is pinned to one
        // rather than left to a general sweep. The assertion below says so if
        // the fixture ever drifts.
        let limits = ReseedLimits {
            attempts: 2,
            alignment_above: -1.0,
        };
        let got = scatter_reseeding(&params(3), TILE, &lattice(), limits);
        let walked = walk(3, 2);

        let tempting = walked.iter().any(|(_, missing, a)| {
            *missing > got.outcome.unplaced_required.len() && *a < got.alignment
        });
        assert!(
            tempting,
            "no seed is better on tracking and worse on coverage, so the \
             ordering is untested: {walked:?}"
        );
        assert_eq!(
            got.outcome.unplaced_required.len(),
            walked.iter().map(|(_, m, _)| *m).min().expect("seeds"),
        );
    }

    #[test]
    fn the_same_request_chooses_the_same_seed() {
        let a = scatter_reseeding(&params(99), TILE, &lattice(), ReseedLimits::default());
        let b = scatter_reseeding(&params(99), TILE, &lattice(), ReseedLimits::default());
        assert_eq!(a.seed, b.seed);
        assert_eq!(a.outcome.placements, b.outcome.placements);
    }

    #[test]
    fn zero_attempts_still_produces_a_layout() {
        let limits = ReseedLimits {
            attempts: 0,
            ..ReseedLimits::default()
        };
        let got = scatter_reseeding(&params(1), TILE, &lattice(), limits);
        assert_eq!(got.attempts, 1);
        assert!(!got.outcome.placements.is_empty());
    }
}
