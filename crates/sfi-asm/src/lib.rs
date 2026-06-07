//! SFI-ASM core engine.
//!
//! A faithful, minimal re-implementation of the Santa Fe Institute Artificial
//! Stock Market (Arthur, Holland, LeBaron, Palmer, Tayler 1996; LeBaron 2002),
//! built engine-first so the science can be unit-tested without any GUI.
//!
//! Phase 1 (this file set): single asset, AR(1) dividend, classifier agents with
//! linear forecasts, exact market clearing, individual learning, and a genetic
//! algorithm. The network-coupling extension (Phase 2) plugs in on top of this.
//!
//! Design decisions that fix the loopholes identified in review:
//! - Market-state bits are evaluated on the *lagged* price `p_{t-1}` to avoid the
//!   circular dependence between rule selection and the price being solved for.
//! - Forecast variance has a floor (`min_variance`) so demand cannot blow up.
//! - Fitness is a *bounded* strength `base - accuracy - cost*specificity`, never a
//!   reciprocal that explodes when a forecast is accidentally perfect.
//! - The default mutation operator is drift-neutral (re-draws a mutated trit from
//!   the stationary symbol distribution), which avoids the Ehrentreich (2002) bias
//!   that made "emergent technical trading" partly an artifact.

/// Number of bits in the market-state descriptor.
///
/// Layout (bit index -> meaning):
/// 0..=5  fundamental: price/dividend ratio `p*r/d` exceeds {0.25,0.5,0.75,0.875,1.0,1.125}
/// 6..=9  technical:   `p > MA(5)`, `p > MA(10)`, `p > MA(100)`, `p > MA(500)`
/// 10     control:     always 1  (drift detector)
/// 11     control:     always 0  (drift detector)
pub const NBITS: usize = 12;

/// Bits 6..=9 are the technical-trading signals. Their aggregate usage / fitness
/// is the central observable for the regime and (later) network experiments.
pub const TECHNICAL_BITS: std::ops::Range<usize> = 6..10;

pub mod rng;
pub mod params;
pub mod dividend;
pub mod classifier;
pub mod agent;
pub mod market;
pub mod network;
pub mod ga;
pub mod stats;
pub mod sim;
pub mod sandbox;

pub use agent::Agent;
pub use classifier::Rule;
pub use dividend::{Dividend, Ree};
pub use network::{Network, Topology};
pub use params::Params;
pub use sandbox::{Cohort, Outcomes, Sandbox, SandboxParams};
pub use rng::Rng;
pub use sim::Simulation;
pub use stats::ReturnStats;
