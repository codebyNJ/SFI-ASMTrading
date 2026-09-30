//! Market clearing: solve for the price that equates aggregate demand to supply.
//!
//! Each agent's mean-variance demand is
//! `x_i = (a_i (p + d) + b_i - (1+r) p) / (lambda * sigma_i^2)`.
//! Because the forecast is linear in `p`, summing `x_i = N` (one share per agent)
//! gives a single linear equation we solve exactly — no iteration needed for the
//! linear-forecast model.
//!
//! Note the forecast magnitude legitimately depends on the price being solved
//! (via the `a_i p` term); what we deliberately do *not* let depend on `p` is the
//! *rule selection* — those condition bits are evaluated on the lagged price in
//! `sim.rs`, which is what removes the circularity.

use crate::agent::Forecast;
use crate::params::Params;

/// Result of clearing: the cleared price and each agent's realized demand.
pub struct Clearing {
    pub price: f64,
    pub demands: Vec<f64>,
}

/// Solve `sum_i x_i = N` for the price. `prev_price` is used as a safe fallback
/// if the demand curve is (near) vertical.
pub fn clear(forecasts: &[Forecast], d: f64, p: &Params, prev_price: f64) -> Clearing {
    let total_supply = forecasts.len() as f64; // one share per agent
    let mut denom = 0.0; // sum w_i (a_i - (1+r))
    let mut known = 0.0; // sum w_i (a_i d + b_i)
    for &(a, b, var) in forecasts {
        let w = 1.0 / (p.lambda * var.max(p.min_variance));
        denom += w * (a - (1.0 + p.r));
        known += w * (a * d + b);
    }

    let raw = if denom.abs() < 1e-9 { prev_price } else { (total_supply - known) / denom };
    let price = if raw.is_finite() { raw.clamp(0.01, p.max_price) } else { prev_price };

    let demands = forecasts
        .iter()
        .map(|&(a, b, var)| {
            let v = var.max(p.min_variance);
            (a * (price + d) + b - price * (1.0 + p.r)) / (p.lambda * v)
        })
        .collect();

    Clearing { price, demands }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dividend::Ree;

    #[test]
    fn homogeneous_ree_forecast_recovers_ree_price() {
        // If every agent uses the REE forecast (a=rho, b=b_ree) with the REE
        // variance, clearing must return exactly the REE price p = f*d + g.
        let p = Params::classic();
        let ree = Ree::new(&p);
        let (a, b) = ree.forecast_ab(&p);
        let forecasts: Vec<Forecast> = (0..p.n_agents).map(|_| (a, b, ree.sigma2)).collect();

        for &d in &[8.0, 10.0, 12.5] {
            let clearing = clear(&forecasts, d, &p, ree.price(d));
            let expected = ree.price(d);
            assert!((clearing.price - expected).abs() < 1e-6, "d={d}: cleared {} vs REE {expected}", clearing.price);
            // Demands should each be ~1 (symmetric one-share holding) and sum to N.
            let sum: f64 = clearing.demands.iter().sum();
            assert!((sum - p.n_agents as f64).abs() < 1e-6, "demand sum {sum}");
            for x in &clearing.demands {
                assert!((x - 1.0).abs() < 1e-6, "demand {x}");
            }
        }
    }

    #[test]
    fn vertical_demand_falls_back_to_prev_price() {
        // a = 1+r makes each demand independent of price (denominator zero).
        let p = Params::classic();
        let forecasts: Vec<Forecast> = vec![(1.0 + p.r, 5.0, 1.0); 10];
        let clearing = clear(&forecasts, 10.0, &p, 123.0);
        assert_eq!(clearing.price, 123.0);
    }
}
