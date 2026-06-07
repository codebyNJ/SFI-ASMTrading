//! Phase-2 experiment: does network topology change the market's stylized facts
//! when agents learn socially (copy rules from neighbors)?
//!
//! Single runs are noisy, so we average each topology over many seeds and report
//! mean +/- standard deviation. Topologies are compared at *matched mean degree*
//! so any difference is attributable to structure, not raw connectivity.
//!
//! Headline observables (the things that define real market "stylized facts"):
//!   - excess kurtosis  -> fat tails
//!   - |return| AC(1)   -> volatility clustering
//!   - mean |deviation| -> persistent mispricing / bubbles
//!   - max  |deviation| -> largest bubble/crash seen
//!
//! Run: `cargo run --release --example experiment`

use sfi_asm::{Params, Simulation, Topology};

const SEEDS: u64 = 12;
const N_AGENTS: usize = 50;
const SOCIAL_PROB: f64 = 0.50;
const MEAN_DEGREE: f64 = 6.0;
const BURN: u64 = 10_000;
const TOTAL: u64 = 60_000;

/// One topology's results across all seeds.
struct Metric {
    samples: Vec<f64>,
}
impl Metric {
    fn new() -> Self {
        Self { samples: Vec::new() }
    }
    fn push(&mut self, x: f64) {
        self.samples.push(x);
    }
    fn mean(&self) -> f64 {
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }
    fn std(&self) -> f64 {
        let m = self.mean();
        let v = self.samples.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / self.samples.len() as f64;
        v.sqrt()
    }
}

struct TopoResult {
    label: &'static str,
    mean_degree: f64,
    max_degree: usize,
    kurtosis: Metric,
    abs_ac1: Metric,
    mean_dev: Metric,
    max_dev: Metric,
    tech_bits: Metric,
}

fn run_topology(label: &'static str, topology: Topology) -> TopoResult {
    let mut res = TopoResult {
        label,
        mean_degree: 0.0,
        max_degree: 0,
        kurtosis: Metric::new(),
        abs_ac1: Metric::new(),
        mean_dev: Metric::new(),
        max_dev: Metric::new(),
        tech_bits: Metric::new(),
    };

    for s in 0..SEEDS {
        let mut p = Params::complex();
        // Complex regime found by the regime sweep: fast GA + short learning memory
        // + no specificity cost. Here the market *does* produce stylized facts, so
        // there is something for topology to modulate.
        p.ga_interval = 100;
        p.spec_cost = 0.0;
        p.theta = 0.05;
        p.n_agents = N_AGENTS;
        p.social_prob = SOCIAL_PROB;
        p.topology = topology;
        p.seed = 1000 + s;

        let mut sim = Simulation::new(p);
        let mut dev_sum = 0.0;
        let mut dev_max: f64 = 0.0;
        let mut count = 0.0;
        for t in 0..TOTAL {
            sim.step(t);
            if t >= BURN {
                let dev = sim.relative_deviation().abs();
                dev_sum += dev;
                dev_max = dev_max.max(dev);
                count += 1.0;
            }
        }
        let st = sim.stats();
        res.kurtosis.push(st.excess_kurtosis);
        res.abs_ac1.push(st.abs_autocorr1);
        res.mean_dev.push(dev_sum / count);
        res.max_dev.push(dev_max);
        res.tech_bits.push(sim.mean_technical_bits());
        // Network structure is the same shape across seeds; record from last run.
        res.mean_degree = sim.network.mean_degree();
        res.max_degree = sim.network.max_degree();
    }
    res
}

fn main() {
    println!("SFI-ASM Phase 2 - network topology vs stylized facts (COMPLEX regime)");
    println!(
        "{N_AGENTS} agents, tau=100, theta=0.05, spec_cost=0, social_prob={SOCIAL_PROB}, \
         mean degree ~{MEAN_DEGREE:.0}, {SEEDS} seeds x {TOTAL} periods\n"
    );

    let runs = [
        run_topology("None      ", Topology::None),
        run_topology("Lattice   ", Topology::Lattice { k: MEAN_DEGREE as usize }),
        run_topology("Random    ", Topology::Random { mean_degree: MEAN_DEGREE }),
        run_topology("Scale-free", Topology::ScaleFree { m: (MEAN_DEGREE as usize) / 2 }),
    ];

    let row = |name: &str, f: &dyn Fn(&TopoResult) -> &Metric| {
        print!("{name:<14}");
        for r in &runs {
            let m = f(r);
            print!("  {:>7.3} +/-{:>6.3}", m.mean(), m.std());
        }
        println!();
    };

    print!("{:<14}", "metric \\ topo");
    for r in &runs {
        print!("  {:>16}", r.label.trim());
    }
    println!();
    println!("{}", "-".repeat(14 + runs.len() * 18));

    print!("{:<14}", "mean degree");
    for r in &runs {
        print!("  {:>9.2} (max {:>3})", r.mean_degree, r.max_degree);
    }
    println!();
    row("kurtosis", &|r| &r.kurtosis);
    row("AC(|r|)", &|r| &r.abs_ac1);
    row("mean|dev|", &|r| &r.mean_dev);
    row("max|dev|", &|r| &r.max_dev);
    row("tech bits", &|r| &r.tech_bits);

    println!("\nfat tails = higher kurtosis; volatility clustering = higher AC(|r|); bubbles = higher max|dev|");
}
