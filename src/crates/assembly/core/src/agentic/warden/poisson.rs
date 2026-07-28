#![cfg(feature = "taiji")]
//! Poisson distribution-based scheduling for Challenge-Poke protocol.
//!
//! The scheduler determines whether a Challenge-Poke message should be sent
//! in the current turn, based on a Poisson process with configurable rate.
//! This produces random inter-poke intervals that average to `rate` rounds.

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// Poisson-distributed event scheduler for Challenge-Poke.
///
/// Uses a deterministic RNG (`StdRng`) seeded at construction for reproducible
/// scheduling sequences. Each call to [`should_poke`] advances an internal
/// round counter and performs a Bernoulli trial with probability `1/rate`.
///
/// Over a large number of rounds, the inter-poke intervals follow a Geometric
/// distribution (the discrete analogue of the Exponential distribution), whose
/// mean converges to `rate`.
///
/// # Example
/// ```
/// use bitfun_core::warden::PoissonScheduler;
///
/// let mut sched = PoissonScheduler::new(6.5, 42);
/// let mut poke_count = 0u64;
/// for _ in 0..1000 {
///     if sched.should_poke() {
///         poke_count += 1;
///     }
/// }
/// // With rate=6.5, ~154 pokes expected in 1000 rounds (1000/6.5 ≈ 153.8)
/// assert!(poke_count > 50, "Expected roughly 154 pokes, got {poke_count}");
/// ```
#[derive(Debug, Clone)]
pub struct PoissonScheduler {
    /// Average number of rounds between pokes (e.g., 6.5 for 5–8 range midpoint).
    rate: f64,
    /// Deterministic CSPRNG for reproducible randomness.
    rng: StdRng,
    /// Monotonically increasing round counter.
    counter: u64,
}

impl PoissonScheduler {
    /// Create a new scheduler with the given average inter-poke interval and RNG seed.
    ///
    /// `rate` is the mean number of rounds between consecutive pokes. The
    /// recommended value from the Challenge-Poke contract is 6.5 (midpoint of 5–8).
    ///
    /// `seed` is used to initialize the deterministic [`StdRng`]. Identical
    /// seeds produce identical scheduling sequences.
    pub fn new(rate: f64, seed: u64) -> Self {
        Self {
            rate,
            rng: StdRng::seed_from_u64(seed),
            counter: 0,
        }
    }

    /// Create a new scheduler with a randomly generated seed.
    ///
    /// Uses system entropy via [`StdRng::from_entropy`] for the initial seed.
    /// Scheduling sequences produced by this constructor are **not** reproducible.
    pub fn new_random(rate: f64) -> Self {
        Self {
            rate,
            rng: StdRng::from_entropy(),
            counter: 0,
        }
    }

    /// Evaluate whether a Challenge-Poke should fire in the current round.
    ///
    /// Each call advances the internal round counter by one. The decision is
    /// a Bernoulli trial with success probability `p = 1 / rate`.
    ///
    /// Returns `true` when the current round is selected for a poke event.
    pub fn should_poke(&mut self) -> bool {
        self.counter += 1;
        let p = 1.0 / self.rate;
        self.rng.gen::<f64>() < p
    }

    /// Reset the scheduler to its initial state.
    ///
    /// The round counter is set back to zero. The RNG is **not** re-seeded,
    /// so the scheduling sequence after a reset diverges from the initial
    /// sequence (the RNG continues from its current state).
    pub fn reset(&mut self) {
        self.counter = 0;
    }

    /// Reset the scheduler with a new seed, fully restoring initial conditions.
    ///
    /// Both the round counter and the RNG are reset, making the subsequent
    /// scheduling sequence identical to a freshly constructed scheduler with
    /// the same `rate` and `seed`.
    pub fn reset_with_seed(&mut self, seed: u64) {
        self.counter = 0;
        self.rng = StdRng::seed_from_u64(seed);
    }

    /// Current round counter value.
    pub fn counter(&self) -> u64 {
        self.counter
    }

    /// Configured average inter-poke interval.
    pub fn rate(&self) -> f64 {
        self.rate
    }

    /// Expected number of pokes after `rounds` turns (i.e., `rounds / rate`).
    pub fn expected_pokes(&self, rounds: u64) -> f64 {
        rounds as f64 / self.rate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_seed_produces_identical_sequence() {
        let mut a = PoissonScheduler::new(6.5, 12345);
        let mut b = PoissonScheduler::new(6.5, 12345);

        for _ in 0..100 {
            assert_eq!(a.should_poke(), b.should_poke());
        }
    }

    #[test]
    fn different_seeds_produce_different_sequences() {
        let mut a = PoissonScheduler::new(6.5, 11111);
        let mut b = PoissonScheduler::new(6.5, 22222);

        let mut same_count = 0u32;
        for _ in 0..100 {
            if a.should_poke() == b.should_poke() {
                same_count += 1;
            }
        }
        // Different seeds should differ in at least some outputs
        assert!(same_count < 100, "Different seeds should diverge");
    }

    #[test]
    fn reset_clears_counter() {
        let mut sched = PoissonScheduler::new(6.5, 42);
        for _ in 0..10 {
            sched.should_poke();
        }
        assert_eq!(sched.counter(), 10);
        sched.reset();
        assert_eq!(sched.counter(), 0);
    }

    #[test]
    fn reset_with_seed_restores_initial_behavior() {
        let mut a = PoissonScheduler::new(6.5, 9999);
        for _ in 0..5 {
            a.should_poke();
        }

        // Reset a with the same seed → should be like freshly created
        a.reset_with_seed(9999);

        let mut b = PoissonScheduler::new(6.5, 9999);

        for i in 0..50 {
            assert_eq!(
                a.should_poke(),
                b.should_poke(),
                "Mismatch at position {i} after reset_with_seed"
            );
        }
    }

    #[test]
    fn empirical_rate_converges_to_expected() {
        let mut sched = PoissonScheduler::new(6.5, 7777);
        let trials = 100_000u64;
        let mut pokes = 0u64;

        for _ in 0..trials {
            if sched.should_poke() {
                pokes += 1;
            }
        }

        let expected = trials as f64 / 6.5;
        let actual = pokes as f64;
        let relative_error = (actual - expected).abs() / expected;

        // Allow 5% relative error for 100k trials
        assert!(
            relative_error < 0.05,
            "Expected ~{expected:.1} pokes in {trials} rounds, got {pokes} (error={relative_error:.3})"
        );
    }

    #[test]
    fn expected_pokes_returns_correct_value() {
        let sched = PoissonScheduler::new(6.5, 42);
        let exp = sched.expected_pokes(1300);
        assert!((exp - 200.0).abs() < f64::EPSILON);
    }

    #[test]
    fn new_random_creates_unique_sequences() {
        let mut a = PoissonScheduler::new_random(6.5);
        let mut b = PoissonScheduler::new_random(6.5);

        let results_a: Vec<bool> = (0..50).map(|_| a.should_poke()).collect();
        let results_b: Vec<bool> = (0..50).map(|_| b.should_poke()).collect();

        // Extremely unlikely that two random seeds produce identical 50-step sequences
        assert_ne!(results_a, results_b);
    }

    #[test]
    fn poke_probability_bounds() {
        // With rate=1.0, p=1.0 → every round should poke
        let mut sched = PoissonScheduler::new(1.0, 42);
        for _ in 0..100 {
            assert!(sched.should_poke(), "rate=1.0 should always poke");
        }

        // With a very high rate, p ≈ 0 → almost never pokes
        let mut sched = PoissonScheduler::new(10_000.0, 42);
        let mut pokes = 0u32;
        for _ in 0..10_000 {
            if sched.should_poke() {
                pokes += 1;
            }
        }
        assert!(pokes < 10, "rate=10000 should rarely poke, got {pokes}");
    }
}
