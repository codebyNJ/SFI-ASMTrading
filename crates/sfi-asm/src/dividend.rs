//! AR(1) dividend process and the homogeneous rational-expectations benchmark.

use crate::params::Params;
use crate::rng::Rng;

/// Stochastic dividend `d_t = d_bar + rho (d_{t-1} - d_bar) + eps_t`.
#[derive(Clone, Debug)]
pub struct Dividend {
    pub d_bar: f64,
    pub rho: f64,
    pub sigma_eps: f64,
    pub current: f64,
}

impl Dividend {
    pub fn new(p: &Params) -> Self {
        Self { d_bar: p.d_bar, rho: p.rho, sigma_eps: p.sigma_eps, current: p.d_bar }
    }

    /// Advance one period and return the new dividend.
    pub fn step(&mut self, rng: &mut Rng) -> f64 {
        let eps = rng.gaussian() * self.sigma_eps;
        self.current = self.d_bar + self.rho * (self.current - self.d_bar) + eps;
        self.current
    }
}

/// Homogeneous rational-expectations equilibrium (REE).
///
/// With CARA agents and Gaussian dividends the equilibrium price is linear in the
/// dividend, `p_t = f * d_t + g`, and the equilibrium forecast variance of
/// `p_{t+1} + d_{t+1}` is `sigma2 = (f+1)^2 * sigma_eps^2`. This is the correct
/// "fundamental" benchmark — the README's `P* = d_bar/(r(1-rho))` is wrong; this
/// derivation (see notes in the crate) is what the rational regime should track.
#[derive(Clone, Copy, Debug)]
pub struct Ree {
    pub f: f64,
    pub g: f64,
    pub sigma2: f64,
}

impl Ree {
    pub fn new(p: &Params) -> Self {
        let f = p.rho / (1.0 + p.r - p.rho);
        let sigma2 = (f + 1.0).powi(2) * p.sigma_eps.powi(2);
        let g = ((f + 1.0) * (1.0 - p.rho) * p.d_bar - p.lambda * sigma2) / p.r;
        Self { f, g, sigma2 }
    }

    /// Equilibrium price at dividend `d`.
    pub fn price(&self, d: f64) -> f64 {
        self.f * d + self.g
    }

    /// The REE forecast in `E[p+d]_{t+1} = a*(p_t+d_t) + b` form: `a = rho`.
    pub fn forecast_ab(&self, p: &Params) -> (f64, f64) {
        let a = p.rho;
        let b = (1.0 - p.rho) * ((self.f + 1.0) * p.d_bar + self.g);
        (a, b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dividend_long_run_mean_matches_d_bar() {
        let p = Params::classic();
        let mut rng = Rng::new(1);
        let mut d = Dividend::new(&p);
        let mut sum = 0.0;
        let n = 500_000;
        for _ in 0..n {
            sum += d.step(&mut rng);
        }
        let mean = sum / n as f64;
        assert!((mean - p.d_bar).abs() < 0.05, "mean dividend {mean}");
    }

    #[test]
    fn ree_price_is_sensible() {
        let p = Params::classic();
        let ree = Ree::new(&p);
        // At the mean dividend the REE price sits a touch below the risk-neutral
        // fundamental d_bar/r = 100, due to the risk premium.
        let price = ree.price(p.d_bar);
        assert!(price > 90.0 && price < 100.0, "ree price {price}");
    }
}
