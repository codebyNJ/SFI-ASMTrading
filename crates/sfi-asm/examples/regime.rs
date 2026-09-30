//! Regime finder: sweep the knobs that govern how *rational* the agents are, on
//! the baseline (no network), and look for a regime that produces genuine
//! stylized facts (excess kurtosis > ~1, positive |return| autocorrelation).
//!
//! Rationale: the network experiment can only show a topology effect if the model
//! is capable of emergent phenomena in the first place. Accuracy-greedy agents
//! converge to rational expectations (kurtosis ~ 0); we need to find where the
//! market becomes "complex" before adding the social layer back.
//!
//! Run: `cargo run --release --example regime`

use sfi_asm::{Params, Simulation, Topology};

const SEEDS: u64 = 5;
const N_AGENTS: usize = 50;
const BURN: u64 = 8_000;
const TOTAL: u64 = 40_000;

fn mean(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / v.len() as f64
}

fn run(ga_interval: u64, spec_cost: f64, theta: f64) -> (f64, f64, f64, f64) {
    let mut kurt = Vec::new();
    let mut ac = Vec::new();
    let mut dev = Vec::new();
    let mut tech = Vec::new();
    for s in 0..SEEDS {
        let mut p = Params::classic();
        p.n_agents = N_AGENTS;
        p.topology = Topology::None;
        p.social_prob = 0.0;
        p.ga_interval = ga_interval;
        p.spec_cost = spec_cost;
        p.theta = theta;
        p.seed = 2000 + s;

        let mut sim = Simulation::new(p);
        let mut dev_sum = 0.0;
        let mut count = 0.0;
        for t in 0..TOTAL {
            sim.step(t);
            if t >= BURN {
                dev_sum += sim.relative_deviation().abs();
                count += 1.0;
            }
        }
        let st = sim.stats();
        kurt.push(st.excess_kurtosis);
        ac.push(st.abs_autocorr1);
        dev.push(dev_sum / count);
        tech.push(sim.mean_technical_bits());
    }
    (mean(&kurt), mean(&ac), mean(&dev), mean(&tech))
}

fn main() {
    println!("SFI-ASM regime finder (baseline, no network, {N_AGENTS} agents, {SEEDS} seeds)\n");
    println!(
        "{:>5} {:>9} {:>7}   {:>9} {:>9} {:>9} {:>9}",
        "tau", "spec_cost", "theta", "kurtosis", "AC(|r|)", "mean|dev|", "tech_bits"
    );
    println!("{}", "-".repeat(70));

    let taus = [100u64, 250, 1000];
    let costs = [0.0, 0.005];
    let thetas = [1.0 / 75.0, 0.05, 0.20];

    let mut best = (f64::NEG_INFINITY, String::new());
    for &tau in &taus {
        for &cost in &costs {
            for &theta in &thetas {
                let (k, a, d, t) = run(tau, cost, theta);
                println!("{:>5} {:>9.3} {:>7.3}   {:>9.3} {:>9.3} {:>9.4} {:>9.3}", tau, cost, theta, k, a, d, t);
                if k > best.0 {
                    best = (k, format!("tau={tau}, spec_cost={cost}, theta={theta:.3}"));
                }
            }
        }
    }
    println!("\nmost fat-tailed regime: {}  (excess kurtosis {:.3})", best.1, best.0);
}
