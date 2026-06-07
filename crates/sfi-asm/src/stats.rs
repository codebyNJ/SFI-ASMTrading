//! Rolling return statistics used for regime detection and the BTC-shape target.

use std::collections::VecDeque;

/// Summary statistics over a window of log returns.
#[derive(Clone, Copy, Debug, Default)]
pub struct ReturnStats {
    pub n: usize,
    pub mean: f64,
    pub variance: f64,
    pub std: f64,
    /// Excess kurtosis (0 for a Gaussian; > 0 means fat tails).
    pub excess_kurtosis: f64,
    /// Lag-1 autocorrelation of returns (~0 in efficient markets).
    pub autocorr1: f64,
    /// Lag-1 autocorrelation of |returns| (volatility clustering, > 0 = clustering).
    pub abs_autocorr1: f64,
}

impl ReturnStats {
    /// Compute statistics from a window of log returns.
    pub fn from_returns(returns: &VecDeque<f64>) -> Self {
        let n = returns.len();
        if n < 2 {
            return Self { n, ..Default::default() };
        }
        let nf = n as f64;
        let mean = returns.iter().sum::<f64>() / nf;

        let (mut m2, mut m4) = (0.0, 0.0);
        for &r in returns {
            let dx = r - mean;
            let dx2 = dx * dx;
            m2 += dx2;
            m4 += dx2 * dx2;
        }
        let variance = m2 / nf;
        let std = variance.sqrt();
        let excess_kurtosis = if variance > 1e-300 {
            (m4 / nf) / (variance * variance) - 3.0
        } else {
            0.0
        };

        Self {
            n,
            mean,
            variance,
            std,
            excess_kurtosis,
            autocorr1: lag1_autocorr(returns, mean, variance),
            abs_autocorr1: abs_lag1_autocorr(returns),
        }
    }
}

fn lag1_autocorr(x: &VecDeque<f64>, mean: f64, variance: f64) -> f64 {
    if x.len() < 2 || variance <= 1e-300 {
        return 0.0;
    }
    let mut cov = 0.0;
    for i in 1..x.len() {
        cov += (x[i] - mean) * (x[i - 1] - mean);
    }
    cov / (x.len() as f64) / variance
}

fn abs_lag1_autocorr(x: &VecDeque<f64>) -> f64 {
    let n = x.len();
    if n < 2 {
        return 0.0;
    }
    let nf = n as f64;
    let mean = x.iter().map(|v| v.abs()).sum::<f64>() / nf;
    let mut var = 0.0;
    for &v in x {
        let d = v.abs() - mean;
        var += d * d;
    }
    var /= nf;
    if var <= 1e-300 {
        return 0.0;
    }
    let mut cov = 0.0;
    for i in 1..n {
        cov += (x[i].abs() - mean) * (x[i - 1].abs() - mean);
    }
    cov / nf / var
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;

    #[test]
    fn gaussian_returns_have_near_zero_excess_kurtosis() {
        let mut rng = Rng::new(99);
        let mut w = VecDeque::new();
        for _ in 0..50_000 {
            w.push_back(rng.gaussian());
        }
        let s = ReturnStats::from_returns(&w);
        assert!(s.excess_kurtosis.abs() < 0.15, "excess kurtosis {}", s.excess_kurtosis);
        assert!(s.autocorr1.abs() < 0.03, "ac1 {}", s.autocorr1);
    }
}
