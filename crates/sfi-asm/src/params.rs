//! Simulation parameters with sane, literature-matched defaults.

use crate::network::Topology;

/// All tunable parameters for one simulation run.
///
/// Defaults follow the classic SFI-ASM calibration (Arthur et al. 1996,
/// LeBaron 2002): `d_bar = 10`, `r = 0.1`, `rho = 0.95`, etc. Crucially the
/// regime is controlled by `ga_interval` (the paper's `tau` / `k`): large = slow
/// learning = rational regime; small = fast learning = complex regime.
#[derive(Clone, Debug)]
pub struct Params {
    // --- market structure ---
    /// Number of agents N. Each holds one share in symmetric equilibrium.
    pub n_agents: usize,
    /// Classifier rules per agent M.
    pub n_rules: usize,
    /// Risk-free rate per period.
    pub r: f64,
    /// Risk aversion (lambda) in mean-variance demand.
    pub lambda: f64,

    // --- dividend AR(1): d_t = d_bar + rho (d_{t-1} - d_bar) + eps ---
    pub d_bar: f64,
    pub rho: f64,
    pub sigma_eps: f64,

    // --- individual learning ---
    /// Learning rate theta for the moving forecast-error variance ("accuracy").
    pub theta: f64,
    /// Initial accuracy (forecast-error variance) of a fresh rule.
    pub init_variance: f64,
    /// Floor on forecast variance so demand `~1/sigma^2` cannot blow up.
    pub min_variance: f64,

    // --- fitness shaping ---
    /// Constant offset so fitness `base - accuracy - cost*specificity` stays positive.
    pub fitness_base: f64,
    /// Penalty per specified (non-#) condition bit; rewards general rules.
    pub spec_cost: f64,

    // --- genetic algorithm ---
    /// GA invocation period per agent (tau). The regime knob.
    pub ga_interval: u64,
    /// Fraction of each agent's rule population replaced per GA round.
    pub gen_fraction: f64,
    /// Per-bit / per-parameter mutation probability.
    pub p_mutation: f64,
    /// Probability a replaced child is produced by crossover (else by mutation-only clone).
    pub crossover_prob: f64,

    // --- rule initialization ---
    /// Probability a condition bit is *specified* (not #) at init. Low = general rules.
    /// Also the target specified-density of the drift-neutral mutation operator.
    pub bit_prob: f64,
    /// Bounds on the forecast slope `a`. The upper bound MUST stay below `1 + r`
    /// (with a margin) so the aggregate demand curve always slopes down in price —
    /// this is the market-stability condition that prevents self-fulfilling price
    /// explosions. The REE value `a = rho` sits comfortably inside this band.
    pub a_min: f64,
    pub a_max: f64,

    // --- social learning (network extension) ---
    /// Network wiring over agents. `None` recovers the baseline (no social learning).
    pub topology: Topology,
    /// Probability that a GA offspring takes a parent from a network neighbor
    /// (rather than from the agent's own population). 0 => purely individual GA.
    pub social_prob: f64,

    // --- numerics / housekeeping ---
    pub max_price: f64,
    pub seed: u64,
}

impl Params {
    /// Classic SFI-ASM calibration. `ga_interval` here is the slow/rational value.
    pub fn classic() -> Self {
        Self {
            n_agents: 25,
            n_rules: 100,
            r: 0.1,
            lambda: 0.5,
            d_bar: 10.0,
            rho: 0.95,
            sigma_eps: 0.0743,
            theta: 1.0 / 75.0,
            init_variance: 4.0,
            min_variance: 1e-3,
            fitness_base: 100.0,
            spec_cost: 0.005,
            ga_interval: 1000,
            gen_fraction: 0.20,
            p_mutation: 0.03,
            crossover_prob: 0.10,
            bit_prob: 0.10,
            a_min: 0.7,
            a_max: 1.0, // < 1 + r = 1.1, with a 0.1 margin for a stable demand slope
            topology: Topology::None,
            social_prob: 0.25,
            max_price: 1.0e6,
            seed: 42,
        }
    }

    /// Rational regime: slow learning (large tau).
    pub fn rational() -> Self {
        Self { ga_interval: 1000, ..Self::classic() }
    }

    /// Complex regime: fast learning (small tau).
    pub fn complex() -> Self {
        Self { ga_interval: 250, ..Self::classic() }
    }
}

impl Default for Params {
    fn default() -> Self {
        Self::classic()
    }
}
