//! Social network over agents. Edges define who an agent can copy rules from
//! during the genetic algorithm (the "cultural" learning channel).
//!
//! We implement four topologies so the experiment can isolate the effect of
//! *structure* at matched mean degree:
//! - `None`      — no edges (recovers the baseline Phase-1 model exactly)
//! - `Lattice`   — regular ring lattice, every node has the same degree `k`
//! - `Random`    — Erdős–Rényi G(n, p), degrees Poisson-distributed
//! - `ScaleFree` — Barabási–Albert preferential attachment, hub-dominated
//!
//! Built deterministically from the simulation's seeded RNG.

use crate::rng::Rng;

/// Which network wiring to use, with its structural parameter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Topology {
    /// No links — agents learn purely individually.
    None,
    /// Ring lattice where each node connects to its `k` nearest neighbors (k even).
    Lattice { k: usize },
    /// Erdős–Rényi random graph with the given target mean degree.
    Random { mean_degree: f64 },
    /// Barabási–Albert scale-free graph; each new node attaches `m` edges.
    ScaleFree { m: usize },
}

impl Default for Topology {
    fn default() -> Self {
        Topology::None
    }
}

/// Undirected graph stored as per-node neighbor lists.
#[derive(Clone, Debug)]
pub struct Network {
    pub neighbors: Vec<Vec<usize>>,
}

impl Network {
    pub fn build(n: usize, topo: Topology, rng: &mut Rng) -> Self {
        let neighbors = match topo {
            Topology::None => vec![Vec::new(); n],
            Topology::Lattice { k } => lattice(n, k),
            Topology::Random { mean_degree } => random(n, mean_degree, rng),
            Topology::ScaleFree { m } => scale_free(n, m, rng),
        };
        Self { neighbors }
    }

    #[inline]
    pub fn neighbors(&self, i: usize) -> &[usize] {
        &self.neighbors[i]
    }

    pub fn mean_degree(&self) -> f64 {
        if self.neighbors.is_empty() {
            return 0.0;
        }
        let total: usize = self.neighbors.iter().map(|n| n.len()).sum();
        total as f64 / self.neighbors.len() as f64
    }

    pub fn max_degree(&self) -> usize {
        self.neighbors.iter().map(|n| n.len()).max().unwrap_or(0)
    }
}

/// Add an undirected edge if not already present.
fn add_edge(adj: &mut [Vec<usize>], i: usize, j: usize) {
    if i == j {
        return;
    }
    if !adj[i].contains(&j) {
        adj[i].push(j);
    }
    if !adj[j].contains(&i) {
        adj[j].push(i);
    }
}

fn lattice(n: usize, k: usize) -> Vec<Vec<usize>> {
    let mut adj = vec![Vec::new(); n];
    if n < 2 {
        return adj;
    }
    let half = (k / 2).max(1);
    for i in 0..n {
        for d in 1..=half {
            add_edge(&mut adj, i, (i + d) % n);
        }
    }
    adj
}

fn random(n: usize, mean_degree: f64, rng: &mut Rng) -> Vec<Vec<usize>> {
    let mut adj = vec![Vec::new(); n];
    if n < 2 {
        return adj;
    }
    // Mean degree d => edge probability p = d / (n-1).
    let p = (mean_degree / (n as f64 - 1.0)).clamp(0.0, 1.0);
    for i in 0..n {
        for j in (i + 1)..n {
            if rng.chance(p) {
                add_edge(&mut adj, i, j);
            }
        }
    }
    adj
}

fn scale_free(n: usize, m: usize, rng: &mut Rng) -> Vec<Vec<usize>> {
    let mut adj = vec![Vec::new(); n];
    if n < 2 {
        return adj;
    }
    let m = m.clamp(1, n - 1);
    let m0 = (m + 1).min(n); // seed clique-ish ring of m0 nodes

    // `repeated` is a multiset of node ids; each appearance is one edge-endpoint,
    // so drawing uniformly from it yields degree-proportional (preferential) picks.
    let mut repeated: Vec<usize> = Vec::new();
    for i in 0..m0 {
        let j = (i + 1) % m0;
        if i < j {
            add_edge(&mut adj, i, j);
            repeated.push(i);
            repeated.push(j);
        }
    }

    for v in m0..n {
        let mut chosen: Vec<usize> = Vec::with_capacity(m);
        let mut guard = 0;
        while chosen.len() < m && guard < 20 * m + 20 {
            guard += 1;
            let t = if repeated.is_empty() {
                rng.below(v)
            } else {
                repeated[rng.below(repeated.len())]
            };
            if t != v && !chosen.contains(&t) {
                chosen.push(t);
            }
        }
        // Fallback fill (e.g. if preferential draws kept colliding).
        let mut cand = 0usize;
        while chosen.len() < m && cand < v {
            if cand != v && !chosen.contains(&cand) {
                chosen.push(cand);
            }
            cand += 1;
        }
        for &t in &chosen {
            add_edge(&mut adj, v, t);
            repeated.push(v);
            repeated.push(t);
        }
    }
    adj
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_topology_has_no_edges() {
        let mut rng = Rng::new(1);
        let net = Network::build(50, Topology::None, &mut rng);
        assert_eq!(net.mean_degree(), 0.0);
        assert_eq!(net.max_degree(), 0);
    }

    #[test]
    fn lattice_is_regular() {
        let mut rng = Rng::new(1);
        let net = Network::build(100, Topology::Lattice { k: 4 }, &mut rng);
        for nbrs in &net.neighbors {
            assert_eq!(nbrs.len(), 4, "ring lattice should be k-regular");
        }
        // Edges must be symmetric and self-loop free.
        for (i, nbrs) in net.neighbors.iter().enumerate() {
            for &j in nbrs {
                assert_ne!(i, j);
                assert!(net.neighbors[j].contains(&i));
            }
        }
    }

    #[test]
    fn random_hits_target_mean_degree_roughly() {
        let mut rng = Rng::new(7);
        let net = Network::build(500, Topology::Random { mean_degree: 6.0 }, &mut rng);
        let d = net.mean_degree();
        assert!((d - 6.0).abs() < 1.0, "mean degree {d}");
    }

    #[test]
    fn scale_free_has_hubs() {
        let mut rng = Rng::new(3);
        let net = Network::build(500, Topology::ScaleFree { m: 2 }, &mut rng);
        let mean = net.mean_degree();
        let max = net.max_degree();
        // Hallmark of scale-free: max degree dwarfs the mean.
        assert!(mean > 3.0 && mean < 5.0, "mean degree {mean}");
        assert!(max as f64 > 5.0 * mean, "max degree {max} vs mean {mean}");
        // No self loops, symmetric.
        for (i, nbrs) in net.neighbors.iter().enumerate() {
            for &j in nbrs {
                assert_ne!(i, j);
                assert!(net.neighbors[j].contains(&i));
            }
        }
    }

    #[test]
    fn build_is_deterministic() {
        let a = Network::build(200, Topology::ScaleFree { m: 3 }, &mut Rng::new(42));
        let b = Network::build(200, Topology::ScaleFree { m: 3 }, &mut Rng::new(42));
        assert_eq!(a.neighbors, b.neighbors);
    }
}
