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
/// use bitfun_core::agentic::warden::poisson::PoissonScheduler;
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
            rate: sanitize_rate(rate),
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
            rate: sanitize_rate(rate),
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
    ///
    /// # Guard rails
    ///
    /// A rate that is not a positive finite number (`NaN`, `0`, negative, or
    /// `∞`) is treated as "never poke": the scheduler still advances its round
    /// counter but always returns `false`, so a misconfigured `rate` can never
    /// degenerate into a poke on every turn.
    pub fn should_poke(&mut self) -> bool {
        self.counter += 1;
        let p = 1.0 / self.rate;
        self.rng.gen::<f64>() < p
    }

    /// Replace the configured rate.
    ///
    /// The same sanitization as the constructor applies: a non-positive or
    /// non-finite value becomes the never-poke sentinel.
    pub fn set_rate(&mut self, rate: f64) {
        self.rate = sanitize_rate(rate);
    }

    /// Replace the configured rate with an explicit rate cap
    /// (阈值参数配置化：`ai.thresholds.warden.max_rate` replaces the legacy
    /// hard-coded `MAX_RATE = 1000.0`).
    pub fn set_rate_with_cap(&mut self, rate: f64, max_rate: f64) {
        self.rate = sanitize_rate_with_cap(rate, max_rate);
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
    ///
    /// Note (d1-P2-5): under the never-poke sentinel [`NEVER_POKE_RATE`]
    /// (`f64::MAX`) this returns a theoretical value that is positive but
    /// astronomically small (~1e-308 for realistic round counts) — it is a
    /// *statistical* expectation, not a scheduling guarantee. The actual
    /// behaviour is governed by [`Self::should_poke`], whose Bernoulli trial
    /// `1.0 / NEVER_POKE_RATE` underflows to `0.0` and therefore never fires;
    /// the sentinel can never turn into a poke. Callers that disable
    /// Challenge-Poke via `f64::INFINITY` get an exact `0.0` here. This
    /// doc-only clarification keeps the integer math intact.
    pub fn expected_pokes(&self, rounds: u64) -> f64 {
        rounds as f64 / self.rate
    }
}

/// Maximum accepted average inter-poke interval (rounds between pokes).
///
/// A higher rate makes `p = 1/rate` so small that a poke is astronomically
/// unlikely within any realistic session; rejecting such values keeps a
/// misconfigured rate from silently disabling the Challenge-Poke protocol.
/// Callers that intentionally want to disable Challenge-Poke should use
/// `f64::INFINITY` (accepted) or an empty rule set, not an invalid rate.
const MAX_RATE: f64 = 1000.0;

/// Non-poke sentinel for an invalid rate.
///
/// `1.0 / NEVER_POKE_RATE` underflows to `0.0`, so the Bernoulli trial always
/// fails: `rate = 0` (which would otherwise produce `p = ∞`) can never poke on
/// every turn. `expected_pokes` also stays finite and small.
const NEVER_POKE_RATE: f64 = f64::MAX;

/// Map a configured rate to the value used by the Bernoulli trial.
///
/// A rate that is not a positive finite number within [`MAX_RATE`] is mapped
/// to the never-poke sentinel. `f64::INFINITY` is preserved as a legitimate
/// "disable Challenge-Poke" value (a natural `p = 0`).
fn sanitize_rate(rate: f64) -> f64 {
    sanitize_rate_with_cap(rate, MAX_RATE)
}

/// Same as [`sanitize_rate`] but with an explicit rate cap
/// (阈值参数配置化：`ai.thresholds.warden.max_rate`).
pub(crate) fn sanitize_rate_with_cap(rate: f64, max_rate: f64) -> f64 {
    if rate.is_finite() && rate > 0.0 && rate <= max_rate {
        rate
    } else if rate.is_infinite() && rate > 0.0 {
        rate
    } else {
        NEVER_POKE_RATE
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

    #[test]
    fn non_positive_rate_never_pokes() {
        // rate=0 previously produced p = 1/0 = inf → a poke on every turn.
        // The guard maps it to the never-poke sentinel instead.
        for rate in [0.0, -1.0, -1000.0] {
            let mut sched = PoissonScheduler::new(rate, 42);
            assert_eq!(
                sched.rate(),
                f64::MAX,
                "rate {rate} must be sanitized to the never-poke sentinel"
            );
            let mut pokes = 0u32;
            for _ in 0..1000 {
                if sched.should_poke() {
                    pokes += 1;
                }
            }
            assert_eq!(pokes, 0, "rate {rate} must never poke");
        }
    }

    #[test]
    fn non_finite_and_over_limit_rates_never_poke() {
        for rate in [f64::NAN, f64::NEG_INFINITY] {
            let mut sched = PoissonScheduler::new(rate, 42);
            let mut pokes = 0u32;
            for _ in 0..1000 {
                if sched.should_poke() {
                    pokes += 1;
                }
            }
            assert_eq!(pokes, 0, "rate {rate} must never poke");
        }
        // Over the accepted upper bound the rate is rejected: never poke.
        let mut sched = PoissonScheduler::new(5000.0, 42);
        let mut pokes = 0u32;
        for _ in 0..10_000 {
            if sched.should_poke() {
                pokes += 1;
            }
        }
        assert_eq!(pokes, 0, "rate above the cap must never poke");
        // Positive infinity remains a legitimate "disable Challenge-Poke".
        let mut sched = PoissonScheduler::new(f64::INFINITY, 42);
        assert_eq!(sched.rate(), f64::INFINITY);
        let mut pokes = 0u32;
        for _ in 0..10_000 {
            if sched.should_poke() {
                pokes += 1;
            }
        }
        assert_eq!(pokes, 0, "infinite rate must never poke");
    }

    #[test]
    fn set_rate_applies_the_same_sanitization() {
        let mut sched = PoissonScheduler::new(6.5, 42);
        sched.set_rate(0.0);
        assert_eq!(sched.rate(), f64::MAX);
        let mut pokes = 0u32;
        for _ in 0..1000 {
            if sched.should_poke() {
                pokes += 1;
            }
        }
        assert_eq!(pokes, 0, "rate 0 must never poke after set_rate");
        // A valid replacement restores poking.
        sched.set_rate(1.0);
        assert!(sched.should_poke(), "rate=1.0 must always poke");
    }
}
