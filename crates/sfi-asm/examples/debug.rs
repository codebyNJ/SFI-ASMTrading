//! Throwaway diagnostic: inspect the price path and demand spread to understand
//! the lag-1 oscillation seen in the verification harness.

use sfi_asm::{Params, Simulation};

fn main() {
    let mut sim = Simulation::new(Params::rational());
    for t in 0..10_000 {
        sim.step(t);
    }
    // Print 30 consecutive prices to see if it's a clean 2-cycle.
    let mut prices = Vec::new();
    for t in 10_000..10_030 {
        sim.step(t);
        prices.push((t, sim.price, sim.ree_price(), sim.current_dividend()));
    }
    println!("   t        price        REE          d");
    for (t, p, r, d) in &prices {
        println!("{t:>6}  {p:>10.3}  {r:>10.3}  {d:>8.4}");
    }

    // Quick demand-sensitivity probe: how big is the denominator of clearing?
    let a_dist = sim.forecast_a_distribution();
    let n = a_dist.len() as f64;
    let mean_a = a_dist.iter().sum::<f64>() / n;
    let min_a = a_dist.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_a = a_dist.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let frac_above = a_dist.iter().filter(|&&a| a > 1.0 + Params::rational().r).count() as f64 / n;
    println!("\nforecast a: mean={mean_a:.3} min={min_a:.3} max={max_a:.3} frac(a>1+r)={frac_above:.3}");
}
