//! Confidence-bucket weight calibrator.
//! Tracks per-bucket accuracy from backtest results and provides
//! confidence-adjusted accuracy lookups for downstream recalibration.

/// A single confidence bucket tracking accuracy in that range.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConfidenceBucket {
    pub range: (f64, f64),
    pub accuracy: f64,
    pub sample_count: usize,
}

/// Confidence-binned accuracy tracker.
///
/// Divides [0.0, 1.0] into `num_buckets` equal-width ranges and
/// accumulates per-bucket accuracy as correct/incorrect outcomes
/// arrive from backtest feedback.
pub struct WeightCalibrator {
    pub buckets: Vec<ConfidenceBucket>,
}

impl WeightCalibrator {
    /// Create a calibrator with `num_buckets` equal-width ranges.
    /// Typical value is 10 (0.0–0.1, 0.1–0.2, …, 0.9–1.0).
    pub fn new(num_buckets: usize) -> Self {
        assert!(num_buckets > 0, "num_buckets must be positive");
        let mut buckets = Vec::with_capacity(num_buckets);
        for i in 0..num_buckets {
            let lower = i as f64 / num_buckets as f64;
            let upper = (i + 1) as f64 / num_buckets as f64;
            buckets.push(ConfidenceBucket {
                range: (lower, upper),
                accuracy: 0.0,
                sample_count: 0,
            });
        }
        Self { buckets }
    }

    /// Map a confidence value (0.0–1.0) to its bucket index.
    pub fn bucket_index(&self, confidence: f64) -> usize {
        let c = confidence.clamp(0.0, 1.0);
        let idx = (c * self.buckets.len() as f64).floor() as usize;
        idx.min(self.buckets.len() - 1)
    }

    /// Record a backtest outcome: the signal had `confidence` and was `correct`.
    pub fn record(&mut self, confidence: f64, correct: bool) {
        let idx = self.bucket_index(confidence);
        let bucket = &mut self.buckets[idx];
        let total_correct = bucket.accuracy * bucket.sample_count as f64;
        bucket.sample_count += 1;
        bucket.accuracy = if correct {
            (total_correct + 1.0) / bucket.sample_count as f64
        } else {
            total_correct / bucket.sample_count as f64
        };
    }

    /// Look up the tracked accuracy for a given confidence level.
    pub fn accuracy_at(&self, confidence: f64) -> f64 {
        let idx = self.bucket_index(confidence);
        self.buckets[idx].accuracy
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ten_buckets_correct_ranges() {
        let cal = WeightCalibrator::new(10);
        assert_eq!(cal.buckets.len(), 10);

        // Bucket 0: [0.0, 0.1]
        assert!((cal.buckets[0].range.0 - 0.0).abs() < f64::EPSILON);
        assert!((cal.buckets[0].range.1 - 0.1).abs() < f64::EPSILON);

        // Bucket 9: [0.9, 1.0]
        assert!((cal.buckets[9].range.0 - 0.9).abs() < f64::EPSILON);
        assert!((cal.buckets[9].range.1 - 1.0).abs() < f64::EPSILON);

        // Bucket 5: [0.5, 0.6]
        assert!((cal.buckets[5].range.0 - 0.5).abs() < f64::EPSILON);
        assert!((cal.buckets[5].range.1 - 0.6).abs() < f64::EPSILON);
    }

    #[test]
    fn test_bucket_index_boundaries() {
        let cal = WeightCalibrator::new(10);

        assert_eq!(cal.bucket_index(0.0), 0);
        assert_eq!(cal.bucket_index(0.05), 0);
        assert_eq!(cal.bucket_index(0.099), 0);
        assert_eq!(cal.bucket_index(0.1), 1);
        assert_eq!(cal.bucket_index(0.5), 5);
        assert_eq!(cal.bucket_index(0.95), 9);
        assert_eq!(cal.bucket_index(1.0), 9);
        assert_eq!(cal.bucket_index(1.5), 9); // clamped
        assert_eq!(cal.bucket_index(-0.5), 0); // clamped
    }

    #[test]
    fn test_record_and_accuracy_at() {
        let mut cal = WeightCalibrator::new(10);

        // Record: bucket for 0.05
        cal.record(0.05, true);
        assert_eq!(cal.buckets[0].sample_count, 1);
        assert!((cal.buckets[0].accuracy - 1.0).abs() < f64::EPSILON);

        // Record: another at same bucket, wrong
        cal.record(0.05, false);
        assert_eq!(cal.buckets[0].sample_count, 2);
        assert!((cal.buckets[0].accuracy - 0.5).abs() < f64::EPSILON);

        // accuracy_at
        assert!((cal.accuracy_at(0.05) - 0.5).abs() < f64::EPSILON);
        assert_eq!(cal.accuracy_at(0.85), 0.0); // untouched bucket
    }

    #[test]
    fn test_all_buckets_initial_accuracy_zero() {
        let cal = WeightCalibrator::new(10);
        for bucket in &cal.buckets {
            assert_eq!(bucket.accuracy, 0.0);
            assert_eq!(bucket.sample_count, 0);
        }
    }
}
