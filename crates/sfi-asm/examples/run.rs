//! Phase-1 verification harness.
//!
//! Runs the rational (slow GA) and complex (fast GA) regimes and prints the key
//! observables. Expectation: the rational regime tracks the REE benchmark with
//! near-Gaussian returns; the complex regime shows larger deviations, fatter
//! tails (excess kurtosis), and volatility clustering (|return| autocorrelation).
//!
//! Run: `cargo run --release --example run`

use sfi_asm::{Params, Simulation};

fn main() {
    let burn = 5_000u64;
    let total = 50_000u64;

    println!("SFI-ASM Phase 1 — engine verification");
    println!("periods/run = {total} (first {burn} discarded as burn-in)\n");

    for (label, params) in [("RATIONAL (tau=1000)", Params::rational()), ("COMPLEX  (tau= 250)", Params::complex())] {
        let mut sim = Simulation::new(params);

        let mut dev_sum = 0.0;
        let mut dev_max: f64 = 0.0;
        let mut count = 0.0;
        for t in 0..total {
            sim.step(t);
            if t >= burn {
                let dev = sim.relative_deviation().abs();
                dev_sum += dev;
                dev_max = dev_max.max(dev);
                count += 1.0;
            }
        }

        let s = sim.stats();
        println!("== {label} ==");
        println!("  price            : {:>10.3}", sim.price);
        println!("  REE benchmark    : {:>10.3}", sim.ree_price());
        println!("  mean |dev| / REE : {:>10.4}", dev_sum / count);
        println!("  max  |dev| / REE : {:>10.4}", dev_max);
        println!("  return std       : {:>10.5}", s.std);
        println!("  excess kurtosis  : {:>10.4}   (>0 = fat tails)", s.excess_kurtosis);
        println!("  AC(1)            : {:>10.4}   (~0 = no linear predictability)", s.autocorr1);
        println!("  AC(|r|) lag 1    : {:>10.4}   (>0 = volatility clustering)", s.abs_autocorr1);
        println!("  mean technical bits/rule : {:>6.3}", sim.mean_technical_bits());
        println!();
    }
}
