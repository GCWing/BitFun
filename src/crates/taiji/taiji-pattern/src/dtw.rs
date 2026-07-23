use ndarray::Array2;

/// Multi-dimensional DTW engine with Sakoe-Chiba band constraint and LB_Keogh lower bound.
///
/// Each row of the input arrays is one time step; each column is one feature.
/// The `feature_weights` vector scales each feature dimension before distance computation.
pub struct DtwEngine {
    /// Sakoe-Chiba window width (in time steps). 0 disables the band.
    pub window: usize,
    /// Per-feature weight multipliers for the Euclidean distance.
    pub feature_weights: Vec<f64>,
}

impl DtwEngine {
    pub fn new(window: usize, feature_weights: Vec<f64>) -> Self {
        Self {
            window,
            feature_weights,
        }
    }

    // ── weighted Euclidean distance between two rows ──

    fn weighted_dist(&self, a: &ndarray::ArrayView1<f64>, b: &ndarray::ArrayView1<f64>) -> f64 {
        a.iter()
            .zip(b.iter())
            .zip(self.feature_weights.iter())
            .map(|((ai, bi), wi)| wi * (ai - bi).powi(2))
            .sum::<f64>()
            .sqrt()
    }

    // ── exact DTW with Sakoe-Chiba band ──

    /// Compute the DTW distance between `query` (n × d) and `template` (m × d).
    ///
    /// Uses the Sakoe-Chiba band `self.window` to restrict the warping path.
    /// Returns `f64::INFINITY` when the band makes alignment impossible.
    pub fn distance(&self, query: &Array2<f64>, template: &Array2<f64>) -> f64 {
        let n = query.nrows();
        let m = template.nrows();
        let d = query.ncols();
        assert_eq!(d, template.ncols(), "feature dim mismatch");
        assert_eq!(d, self.feature_weights.len(), "weight dim mismatch");

        if n == 0 || m == 0 {
            return if n == 0 && m == 0 { 0.0 } else { f64::INFINITY };
        }

        let w = if self.window == 0 {
            n.max(m)
        } else {
            self.window
        };

        // DTW matrix: (n+1) × (m+1), row 0 / col 0 are padding
        let mut dtw = Array2::from_elem((n + 1, m + 1), f64::INFINITY);
        dtw[[0, 0]] = 0.0;

        for i in 1..=n {
            let j_start = if w >= n.max(m) {
                1
            } else {
                1usize.max(i.saturating_sub(w))
            };
            let j_end = if w >= n.max(m) { m } else { m.min(i + w) };

            for j in j_start..=j_end {
                let cost = self.weighted_dist(&query.row(i - 1), &template.row(j - 1));
                let prev = dtw[[i - 1, j]]
                    .min(dtw[[i, j - 1]])
                    .min(dtw[[i - 1, j - 1]]);
                dtw[[i, j]] = cost + prev;
            }
        }

        dtw[[n, m]]
    }

    // ── LB_Keogh lower bound ──

    /// Compute the LB_Keogh lower bound.
    ///
    /// Guarantee: `lb_keogh(Q, C) ≤ distance(Q, C)` for any Q, C.
    ///
    /// The bound uses the Sakoe-Chiba band to construct per-time-step envelopes
    /// of the template and measures how far query points fall outside those envelopes.
    pub fn lb_keogh(&self, query: &Array2<f64>, template: &Array2<f64>) -> f64 {
        let n = query.nrows();
        let m = template.nrows();
        let d = query.ncols();
        let w = self.window;

        let mut lb_sq = 0.0_f64;

        for i in 0..n {
            // Map query step i to the template window center
            let center = if n == m {
                i
            } else {
                ((i as f64) * ((m - 1) as f64) / ((n - 1).max(1) as f64)).round() as usize
            };

            let j_start = center.saturating_sub(w);
            let j_end = (center + w + 1).min(m);

            for feat in 0..d {
                let qi = query[[i, feat]];
                let wf = self.feature_weights[feat];

                let mut upper = f64::NEG_INFINITY;
                let mut lower = f64::INFINITY;
                for j in j_start..j_end {
                    let tj = template[[j, feat]];
                    if tj > upper {
                        upper = tj;
                    }
                    if tj < lower {
                        lower = tj;
                    }
                }

                if qi > upper {
                    lb_sq += wf * (qi - upper).powi(2);
                } else if qi < lower {
                    lb_sq += wf * (lower - qi).powi(2);
                }
            }
        }

        lb_sq.sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::arr2;

    fn engine() -> DtwEngine {
        DtwEngine::new(3, vec![1.0, 1.0, 1.0])
    }

    #[test]
    fn self_match_is_zero() {
        let e = engine();
        let q = arr2(&[[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]]);
        let d = e.distance(&q, &q);
        assert!(d < 1e-10, "self-match should be 0, got {}", d);
    }

    #[test]
    fn known_distance() {
        let e = engine();
        let q = arr2(&[[0.0, 0.0, 0.0]]);
        let t = arr2(&[[3.0, 4.0, 0.0]]);
        // Weighted Euclidean: sqrt(1*(3-0)² + 1*(4-0)² + 1*(0-0)²) = sqrt(9+16) = 5
        let d = e.distance(&q, &t);
        assert!((d - 5.0).abs() < 1e-10, "expected 5.0, got {}", d);
    }

    #[test]
    fn symmetry() {
        let e = engine();
        let a = arr2(&[[0.1, 0.2, 0.3], [0.4, 0.5, 0.6]]);
        let b = arr2(&[[0.2, 0.3, 0.4], [0.5, 0.6, 0.7], [0.8, 0.9, 1.0]]);
        let d_ab = e.distance(&a, &b);
        let d_ba = e.distance(&b, &a);
        assert!(
            (d_ab - d_ba).abs() < 1e-10,
            "DTW should be symmetric, got {} vs {}",
            d_ab,
            d_ba
        );
    }

    #[test]
    fn lb_keogh_lower_bound_property() {
        let e = DtwEngine::new(2, vec![1.0, 1.0]);
        let q = arr2(&[[0.5, 0.1], [1.2, 0.8], [2.0, 1.5], [1.0, 1.0], [0.3, 0.5]]);
        let t = arr2(&[[0.6, 0.2], [1.0, 0.9], [1.8, 1.6], [1.1, 1.1], [0.4, 0.6]]);
        let lb = e.lb_keogh(&q, &t);
        let dtw = e.distance(&q, &t);
        assert!(
            lb <= dtw + 1e-10,
            "LB_Keogh ({}) must be ≤ DTW distance ({})",
            lb,
            dtw
        );
    }

    #[test]
    fn feature_weights_are_respected() {
        let e = DtwEngine::new(3, vec![10.0, 0.0, 1.0]); // heavily weight first feature
        let q = arr2(&[[1.0, 999.0, 0.0]]); // big diff on 2nd feature but weight=0
        let t = arr2(&[[2.0, 0.0, 0.0]]);
        // Expected: sqrt(10*(1-2)² + 0*(999-0)² + 1*(0-0)²) = sqrt(10) ≈ 3.162
        let d = e.distance(&q, &t);
        assert!((d - 3.16227766).abs() < 1e-6, "got {}", d);
    }
}
