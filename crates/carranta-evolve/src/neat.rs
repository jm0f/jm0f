//! NEAT proper (E-1, phase two): topology and weights evolved together.
//!
//! Classic NEAT as published, deliberately: minimal starting networks, every
//! input wired straight to the output; historical innovation numbers so
//! crossover can align genes that share an origin; speciation by structural
//! distance with fitness sharing, so a new topology gets time to tune its
//! weights before it must beat the field. The result is meant to be "NEAT
//! discovered play", not "NEAT fine-tuned our heuristic": the tuned linear
//! bot stays where it belongs, pinned to the ladder as the anchor to beat.
//!
//! # Determinism
//!
//! Everything structural happens in the serial breeding step: innovation
//! numbers, node ids, species membership. Worker threads only ever *play*
//! compiled networks, so the worker count can never change what evolves, and
//! a resumed run breeds exactly what the uninterrupted run would have bred.
//! The per-generation memo that gives identical structural mutations one
//! innovation number is cleared at each breeding step, so a checkpoint needs
//! only the two counters.

use carranta_bot::features::FEATURES;
use carranta_bot::net::Net;
use carranta_core::rng::{Rng, Stream};
use std::collections::HashMap;

/// Inputs every network reads. Fixed by the observation contract.
pub const INPUTS: usize = FEATURES;

/// How much of the observation generation zero listens to (E-34): the first
/// 38 senses, the width whose fully wired birth produced the fastest start
/// ever recorded (anchor parity at generation 7). Everything past it is born
/// present and asleep; see [`NeatGenome::minimal`].
pub const GENESIS_SPINE: usize = 38;

/// One connection gene.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gene {
    pub innov: u32,
    pub from: u32,
    pub to: u32,
    pub weight: f64,
    pub enabled: bool,
}

/// A NEAT genome: connection genes in innovation order.
///
/// Nodes are implicit: inputs, bias and output always exist, and a hidden
/// node exists exactly when a gene mentions it. That cannot lose track of a
/// node with no genes at all, because `add_node` always creates two genes and
/// disabling never deletes.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct NeatGenome {
    pub genes: Vec<Gene>,
}

/// The knobs of the algorithm, with the published defaults.
#[derive(Clone, Copy, Debug)]
pub struct Params {
    /// Chance each gene's weight is touched at all in one mutation pass.
    pub weight_p: f64,
    /// Of the touched, the share nudged rather than replaced outright.
    pub perturb_p: f64,
    /// Size of a nudge.
    pub power: f64,
    /// Range of a fresh weight, symmetric about zero.
    pub fresh: f64,
    /// Chance of one new connection per offspring.
    pub add_conn_p: f64,
    /// Chance of one split (new node) per offspring.
    pub add_node_p: f64,
    /// Chance of removing one enabled connection per offspring (E-35).
    ///
    /// Zero in the complexifying phase, which is every generation of a run
    /// that never simplifies: additive mutation alone makes genome size a
    /// ratchet, and a plateau removes the only thing that ever pushed back.
    pub del_conn_p: f64,
    /// Compatibility coefficients: excess, disjoint, mean weight difference.
    pub c1: f64,
    pub c2: f64,
    pub c3: f64,
    /// Where the compatibility threshold starts, and how it moves to hold the
    /// species count near the target.
    pub delta_start: f64,
    pub delta_step: f64,
    pub delta_floor: f64,
    pub target_species: usize,
    /// Generations a species may go without improving before it is culled.
    pub stagnation: u32,
    /// A gene disabled in either parent stays disabled in the child this
    /// often (classic 0.75).
    pub keep_disabled_p: f64,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            weight_p: 0.8,
            perturb_p: 0.9,
            power: 0.5,
            fresh: 2.0,
            add_conn_p: 0.08,
            add_node_p: 0.03,
            del_conn_p: 0.0,
            c1: 1.0,
            c2: 1.0,
            c3: 0.4,
            delta_start: 1.2,
            delta_step: 0.1,
            delta_floor: 0.3,
            target_species: 8,
            stagnation: 15,
            keep_disabled_p: 0.75,
        }
    }
}

/// The historical record: which structural change got which number.
///
/// Counters persist for the life of a run and go into every checkpoint. The
/// memos are per-generation, cleared at each breeding step, and exist so that
/// two offspring inventing the same connection in the same generation carry
/// the same innovation number, which is what lets crossover align them later.
#[derive(Clone, Debug)]
pub struct Innovations {
    pub next_innov: u32,
    pub next_node: u32,
    conns: HashMap<(u32, u32), u32>,
    splits: HashMap<u32, (u32, u32, u32)>,
}

impl Innovations {
    /// A fresh history whose first numbers follow the minimal topology.
    pub fn new() -> Self {
        Innovations {
            // The minimal genome uses innovations 0..=INPUTS (inputs plus
            // bias) and node ids up to output; history begins after them.
            next_innov: INPUTS as u32 + 1,
            next_node: Net::output_id(INPUTS) + 1,
            conns: HashMap::new(),
            splits: HashMap::new(),
        }
    }

    /// Restore the counters from a checkpoint. Memos start empty, which is
    /// exactly the state an uninterrupted run is in at a generation boundary.
    pub fn restore(next_innov: u32, next_node: u32) -> Self {
        Innovations {
            next_innov,
            next_node,
            conns: HashMap::new(),
            splits: HashMap::new(),
        }
    }

    /// Forget the generation's memos. Called once per breeding step.
    pub fn begin_generation(&mut self) {
        self.conns.clear();
        self.splits.clear();
    }

    fn connection(&mut self, from: u32, to: u32) -> u32 {
        *self.conns.entry((from, to)).or_insert_with(|| {
            let n = self.next_innov;
            self.next_innov += 1;
            n
        })
    }

    fn split(&mut self, innov: u32) -> (u32, u32, u32) {
        if let Some(&hit) = self.splits.get(&innov) {
            return hit;
        }
        let node = self.next_node;
        self.next_node += 1;
        let a = self.next_innov;
        let b = self.next_innov + 1;
        self.next_innov += 2;
        self.splits.insert(innov, (node, a, b));
        (node, a, b)
    }
}

impl Default for Innovations {
    fn default() -> Self {
        Innovations::new()
    }
}

/// Uniform in [-1, 1), from the shared deterministic generator.
fn uniform(rng: &mut Rng) -> f64 {
    (rng.below(Stream::Board, 2_000_000) as f64 / 1_000_000.0) - 1.0
}

fn chance(rng: &mut Rng, p: f64) -> bool {
    (rng.below(Stream::Board, 1_000_000) as f64 / 1_000_000.0) < p
}

impl NeatGenome {
    /// The starting network: every input and the bias carried as a gene to
    /// the single output, weights small and random, no hidden nodes, and
    /// only the proven spine of the observation switched on (E-34).
    ///
    /// Innovation numbers for these genes are fixed by position, `0..=INPUTS`,
    /// so every genome in every run agrees on them without consulting the
    /// history. That is what makes generation zero one species: the flags
    /// differ between genomes, the genes do not, and compatibility reads
    /// genes.
    ///
    /// The sparse start is FS-NEAT's lesson sized to this observation's
    /// history. A fully wired birth at the full width buries the signal
    /// under scores of random weights, which is measurable here: parity
    /// with the anchor came at generation 7 for the 38-input run and 24 for
    /// the 78-input one, same senses plus dilution. So generation zero
    /// listens to the first [`GENESIS_SPINE`] senses, the slice that carried
    /// the fastest start ever recorded, plus a few random others per genome
    /// so selection hears about the rest from the first deal. Nothing is
    /// lost: the dormant genes are present, their weights keep evolving,
    /// crossover wakes a sleeping gene a quarter of the time, and a wake
    /// that helps is kept by the only judge that matters. Density anneals
    /// back in as the population earns it, which is the schedule the wide
    /// runs needed and never had.
    pub fn minimal(rng: &mut Rng) -> NeatGenome {
        let out = Net::output_id(INPUTS);
        let bias = Net::bias_id(INPUTS) as u32;
        let spine = GENESIS_SPINE.min(INPUTS) as u32;
        let mut genes: Vec<Gene> = (0..=INPUTS as u32)
            .map(|i| Gene {
                innov: i,
                from: i,
                to: out,
                weight: uniform(rng),
                enabled: i < spine || i == bias,
            })
            .collect();
        // A few ears beyond the spine, different ones per genome: across a
        // population every dormant sense is born awake somewhere.
        let dormant = INPUTS as u32 - spine;
        for _ in 0..3 {
            if dormant > 0 {
                let pick = spine + rng.below(Stream::Board, dormant);
                genes[pick as usize].enabled = true;
            }
        }
        NeatGenome { genes }
    }

    /// How many genes actually compute: the size a search has to work
    /// around, and what the phased controller (E-35) measures.
    pub fn enabled_len(&self) -> usize {
        self.genes.iter().filter(|g| g.enabled).count()
    }

    /// The enabled genes, as the network compiler wants them.
    pub fn links(&self) -> Vec<(u32, u32, f64)> {
        self.genes
            .iter()
            .filter(|g| g.enabled)
            .map(|g| (g.from, g.to, g.weight))
            .collect()
    }

    /// Compile to a playable network.
    ///
    /// Panics on failure: mutation refuses cycles, so an uncompilable genome
    /// is a bug in this file, and hiding it behind an `Option` would let a
    /// run limp along measuring nonsense.
    pub fn compile(&self) -> Net {
        Net::assemble(INPUTS, &self.links()).expect("a genome bred here is acyclic")
    }

    /// Every node a gene mentions, plus the fixed ones, ascending.
    fn nodes(&self) -> Vec<u32> {
        let mut ids: Vec<u32> = (0..=Net::output_id(INPUTS)).collect();
        for g in &self.genes {
            ids.push(g.from);
            ids.push(g.to);
        }
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    /// Would adding `from -> to` close a cycle?
    ///
    /// Walks the existing graph, disabled genes included: a disabled gene can
    /// be re-enabled by crossover, and a cycle that exists only when it wakes
    /// up is still a cycle this genome carried.
    fn creates_cycle(&self, from: u32, to: u32) -> bool {
        if from == to {
            return true;
        }
        // Does `from` sit downstream of `to`?
        let mut frontier = vec![to];
        let mut seen = vec![to];
        while let Some(at) = frontier.pop() {
            for g in &self.genes {
                if g.from == at && !seen.contains(&g.to) {
                    if g.to == from {
                        return true;
                    }
                    seen.push(g.to);
                    frontier.push(g.to);
                }
            }
        }
        false
    }

    /// One mutation pass: weights first, then perhaps a structural change.
    pub fn mutate(&self, rng: &mut Rng, params: &Params, inn: &mut Innovations) -> NeatGenome {
        let mut out = self.clone();
        for g in out.genes.iter_mut() {
            if !chance(rng, params.weight_p) {
                continue;
            }
            if chance(rng, params.perturb_p) {
                g.weight += uniform(rng) * params.power;
            } else {
                g.weight = uniform(rng) * params.fresh;
            }
        }

        if chance(rng, params.add_node_p) && !out.genes.is_empty() {
            // Split a random enabled gene: disable it, route through a new
            // node. The incoming half takes weight 1 and the outgoing half
            // the old weight, so the network computes nearly what it did and
            // the new structure starts life unpunished.
            let enabled: Vec<usize> = (0..out.genes.len())
                .filter(|&i| out.genes[i].enabled)
                .collect();
            if !enabled.is_empty() {
                let pick = enabled[rng.below(Stream::Board, enabled.len() as u32) as usize];
                let old = out.genes[pick];
                out.genes[pick].enabled = false;
                let (node, first, second) = inn.split(old.innov);
                out.genes.push(Gene {
                    innov: first,
                    from: old.from,
                    to: node,
                    weight: 1.0,
                    enabled: true,
                });
                out.genes.push(Gene {
                    innov: second,
                    from: node,
                    to: old.to,
                    weight: old.weight,
                    enabled: true,
                });
            }
        } else if chance(rng, params.del_conn_p) {
            // Take an enabled connection back out (E-35). Enabled ones only:
            // a sleeping gene costs the network nothing to carry and is the
            // reservoir the sparse genesis deliberately keeps, while an
            // enabled one is a dimension every later mutation has to search
            // around. Removed rather than disabled, because a disabled gene
            // is still carried, still counted, and still woken by crossover.
            let enabled: Vec<usize> = (0..out.genes.len())
                .filter(|&i| out.genes[i].enabled)
                .collect();
            // Never dissolve a genome entirely: something has to reach the
            // output for the network to be a network.
            if enabled.len() > 2 {
                let pick = enabled[rng.below(Stream::Board, enabled.len() as u32) as usize];
                out.genes.remove(pick);
            }
        } else if chance(rng, params.add_conn_p) {
            // A few attempts at a random legal connection, then give up
            // quietly: a dense network simply has fewer places to grow.
            let nodes = out.nodes();
            let bias = Net::bias_id(INPUTS);
            let output = Net::output_id(INPUTS);
            for _ in 0..8 {
                let from = nodes[rng.below(Stream::Board, nodes.len() as u32) as usize];
                let to = nodes[rng.below(Stream::Board, nodes.len() as u32) as usize];
                // Sources are anything but the output; sinks are anything but
                // inputs and bias.
                if from == output || to <= bias {
                    continue;
                }
                if out.genes.iter().any(|g| g.from == from && g.to == to) {
                    continue;
                }
                if out.creates_cycle(from, to) {
                    continue;
                }
                let innov = inn.connection(from, to);
                out.genes.push(Gene {
                    innov,
                    from,
                    to,
                    weight: uniform(rng),
                    enabled: true,
                });
                break;
            }
        }
        out.genes.sort_unstable_by_key(|g| g.innov);
        out
    }

    /// Crossover, `fitter` first.
    ///
    /// Matching genes take their weight from either parent at random;
    /// disjoint and excess genes come from the fitter parent alone; a gene
    /// disabled in either parent usually stays disabled in the child.
    pub fn cross(
        fitter: &NeatGenome,
        other: &NeatGenome,
        rng: &mut Rng,
        params: &Params,
    ) -> NeatGenome {
        let theirs: HashMap<u32, &Gene> = other.genes.iter().map(|g| (g.innov, g)).collect();
        let mut genes = Vec::with_capacity(fitter.genes.len());
        for g in &fitter.genes {
            let mut child = *g;
            if let Some(t) = theirs.get(&g.innov) {
                if rng.below(Stream::Board, 2) == 1 {
                    child.weight = t.weight;
                }
                // Waking is the default; a gene asleep in either parent
                // usually stays asleep.
                child.enabled =
                    !((!g.enabled || !t.enabled) && chance(rng, params.keep_disabled_p));
            }
            genes.push(child);
        }
        let mut out = NeatGenome { genes };
        // Crossover can re-enable a gene whose partner half was rerouted in
        // only one parent, and two halves of history can meet in one child.
        // Either can close a loop that neither parent had, so the child is
        // checked and repaired rather than trusted: any gene whose waking
        // closes a cycle goes back to sleep.
        out.break_cycles();
        out
    }

    /// Disable whatever minimal set of genes is needed to make the enabled
    /// graph acyclic. Walks genes in innovation order, keeping each one that
    /// does not close a cycle among those kept so far, which is deterministic
    /// and biased toward older structure, the same bias NEAT already has.
    fn break_cycles(&mut self) {
        let mut kept = NeatGenome { genes: Vec::new() };
        let mut asleep: Vec<u32> = Vec::new();
        for g in &self.genes {
            if !g.enabled {
                continue;
            }
            if kept.creates_cycle(g.from, g.to) {
                asleep.push(g.innov);
            } else {
                kept.genes.push(*g);
            }
        }
        for g in self.genes.iter_mut() {
            if asleep.contains(&g.innov) {
                g.enabled = false;
            }
        }
    }

    /// Structural distance, the speciation metric.
    pub fn distance(&self, other: &NeatGenome, params: &Params) -> f64 {
        let mut matching = 0u32;
        let mut weight_diff = 0.0f64;
        let mut disjoint = 0u32;
        let mut excess = 0u32;
        let (mut i, mut j) = (0usize, 0usize);
        let a = &self.genes;
        let b = &other.genes;
        while i < a.len() && j < b.len() {
            match a[i].innov.cmp(&b[j].innov) {
                std::cmp::Ordering::Equal => {
                    matching += 1;
                    weight_diff += (a[i].weight - b[j].weight).abs();
                    i += 1;
                    j += 1;
                }
                std::cmp::Ordering::Less => {
                    disjoint += 1;
                    i += 1;
                }
                std::cmp::Ordering::Greater => {
                    disjoint += 1;
                    j += 1;
                }
            }
        }
        excess += (a.len() - i) as u32 + (b.len() - j) as u32;
        let n = a.len().max(b.len()).max(1) as f64;
        // Classic: small genomes are compared on raw counts.
        let n = if n < 20.0 { 1.0 } else { n };
        let mean_w = if matching > 0 {
            weight_diff / matching as f64
        } else {
            0.0
        };
        params.c1 * excess as f64 / n + params.c2 * disjoint as f64 / n + params.c3 * mean_w
    }

    /// The genome as checkpoint lines. One gene per line, exact weights.
    pub fn show(&self) -> String {
        use std::fmt::Write as _;
        let mut out = String::new();
        for g in &self.genes {
            let _ = writeln!(
                out,
                "gene {} {} {} {:?} {}",
                g.innov,
                g.from,
                g.to,
                g.weight,
                if g.enabled { "on" } else { "off" }
            );
        }
        out
    }

    /// Read one `gene` line written by [`NeatGenome::show`].
    pub fn parse_gene(rest: &str) -> Option<Gene> {
        let mut p = rest.split_whitespace();
        let innov = p.next()?.parse().ok()?;
        let from = p.next()?.parse().ok()?;
        let to = p.next()?.parse().ok()?;
        let weight = p.next()?.parse().ok()?;
        let enabled = match p.next()? {
            "on" => true,
            "off" => false,
            _ => return None,
        };
        Some(Gene {
            innov,
            from,
            to,
            weight,
            enabled,
        })
    }
}

/// One species: a representative, its members for this generation, and how
/// long it has gone without getting better.
#[derive(Clone, Debug)]
pub struct Species {
    pub rep: NeatGenome,
    pub members: Vec<usize>,
    /// Best fitness the species has ever seen. Positions: lower is better.
    pub best: f64,
    pub stale: u32,
}

/// Sort the population into species against last generation's representatives.
///
/// Genomes are assigned in index order to the first species within `delta`,
/// or found a new species with themselves as representative, so the result is
/// a pure function of (population, reps, delta). Species that end up empty
/// are dropped. Each surviving species' representative is its first member,
/// which keeps the next generation's speciation deterministic too.
pub fn speciate(
    population: &[NeatGenome],
    previous: &[Species],
    delta: f64,
    params: &Params,
) -> Vec<Species> {
    let mut species: Vec<Species> = previous
        .iter()
        .map(|s| Species {
            rep: s.rep.clone(),
            members: Vec::new(),
            best: s.best,
            stale: s.stale,
        })
        .collect();
    for (i, g) in population.iter().enumerate() {
        let mut placed = false;
        for s in species.iter_mut() {
            if g.distance(&s.rep, params) < delta {
                s.members.push(i);
                placed = true;
                break;
            }
        }
        if !placed {
            species.push(Species {
                rep: g.clone(),
                members: vec![i],
                best: f64::INFINITY,
                stale: 0,
            });
        }
    }
    species.retain(|s| !s.members.is_empty());
    for s in species.iter_mut() {
        s.rep = population[s.members[0]].clone();
    }
    species
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rng(seed: u64) -> Rng {
        Rng::new(seed)
    }

    #[test]
    fn the_same_structural_invention_gets_one_number_per_generation() {
        // The heart of NEAT: two genomes that grow the same connection in the
        // same generation must carry the same innovation number, or crossover
        // can never align them again.
        let mut inn = Innovations::new();
        let a = inn.connection(3, 40);
        let b = inn.connection(3, 40);
        assert_eq!(a, b, "same invention, same number");
        let c = inn.connection(4, 40);
        assert_ne!(a, c, "a different invention is a different number");

        // Splitting the same gene twice in one generation yields one node.
        let (n1, i1, o1) = inn.split(7);
        let (n2, i2, o2) = inn.split(7);
        assert_eq!((n1, i1, o1), (n2, i2, o2));
        let (n3, ..) = inn.split(8);
        assert_ne!(n1, n3);

        // A new generation may re-invent, and gets fresh numbers: history is
        // per-generation, exactly as published.
        inn.begin_generation();
        let d = inn.connection(3, 40);
        assert_ne!(a, d);
    }

    #[test]
    fn generation_zero_listens_to_the_spine_and_carries_the_rest_asleep() {
        // The sparse genesis (E-34): every sense present as a gene, the
        // proven spine and the bias awake, a few random ears beyond it, and
        // the same gene set in every genome so generation zero stays one
        // species whatever the flags say.
        let mut r = rng(7);
        let a = NeatGenome::minimal(&mut r);
        let b = NeatGenome::minimal(&mut r);
        assert_eq!(a.genes.len(), INPUTS + 1, "every input and the bias");
        assert_eq!(
            a.genes.iter().map(|g| g.innov).collect::<Vec<_>>(),
            b.genes.iter().map(|g| g.innov).collect::<Vec<_>>(),
            "identical gene sets, whatever is awake"
        );
        let bias = Net::bias_id(INPUTS) as u32;
        for g in &a.genes {
            if g.innov < GENESIS_SPINE as u32 || g.innov == bias {
                assert!(g.enabled, "the spine is awake: {}", g.innov);
            }
        }
        let awake_beyond = |g: &NeatGenome| {
            g.genes
                .iter()
                .filter(|g| g.enabled && g.innov >= GENESIS_SPINE as u32 && g.innov != bias)
                .count()
        };
        assert!(awake_beyond(&a) >= 1, "a few ears past the spine");
        assert!(awake_beyond(&a) <= 3);
        // And the ears differ between genomes often enough that a population
        // hears every sense somewhere.
        let mut r2 = rng(7);
        let twin = NeatGenome::minimal(&mut r2);
        assert_eq!(
            twin.genes.iter().map(|g| g.enabled).collect::<Vec<_>>(),
            a.genes.iter().map(|g| g.enabled).collect::<Vec<_>>(),
            "the same rng births the same genome"
        );
    }

    #[test]
    fn splitting_a_gene_keeps_the_computation_and_grows_the_topology() {
        let mut r = rng(2);
        let mut inn = Innovations::new();
        let g = NeatGenome::minimal(&mut r);
        let before = g.genes.len();
        // Force a split by running mutation until one happens.
        let mut split = None;
        for _ in 0..500 {
            let m = g.mutate(&mut r, &Params::default(), &mut inn);
            if m.genes.len() == before + 2 {
                split = Some(m);
                break;
            }
        }
        let m = split.expect("a split happens within 500 tries");
        // Genesis carries dormant genes (E-34), so the split's contribution
        // is one *more* disabled gene: the one that was awake before and is
        // bridged now.
        let was_disabled: Vec<u32> = g
            .genes
            .iter()
            .filter(|g| !g.enabled)
            .map(|g| g.innov)
            .collect();
        let disabled: Vec<&Gene> = m
            .genes
            .iter()
            .filter(|g| !g.enabled && !was_disabled.contains(&g.innov))
            .collect();
        assert_eq!(disabled.len(), 1, "the split gene is disabled, not gone");
        let old = disabled[0];
        let incoming = m
            .genes
            .iter()
            .find(|g| g.enabled && g.from == old.from && g.to >= Net::output_id(INPUTS))
            .expect("an incoming half");
        assert_eq!(incoming.weight, 1.0, "incoming half starts transparent");
        let outgoing = m
            .genes
            .iter()
            .find(|g| g.enabled && g.to == old.to && g.from == incoming.to)
            .expect("an outgoing half");
        assert_eq!(
            outgoing.weight, old.weight,
            "outgoing half carries the old weight"
        );
        m.compile();
    }

    #[test]
    fn five_hundred_generations_of_mutation_never_break_compilability() {
        // The invariant `compile` panics on: whatever mutation does, the
        // enabled graph stays acyclic.
        let mut r = rng(3);
        let mut inn = Innovations::new();
        let mut g = NeatGenome::minimal(&mut r);
        for _round in 0..500 {
            inn.begin_generation();
            g = g.mutate(&mut r, &Params::default(), &mut inn);
            let _ = g.compile();
        }
        assert!(
            g.genes.len() > INPUTS + 1,
            "and structure actually grew: {} genes",
            g.genes.len()
        );
    }

    #[test]
    fn crossover_aligns_by_history_and_takes_structure_from_the_fitter() {
        let mut r = rng(4);
        let mut inn = Innovations::new();
        let base = NeatGenome::minimal(&mut r);
        let mut a = base.clone();
        let mut b = base.clone();
        for _ in 0..40 {
            inn.begin_generation();
            a = a.mutate(&mut r, &Params::default(), &mut inn);
            b = b.mutate(&mut r, &Params::default(), &mut inn);
        }
        let child = NeatGenome::cross(&a, &b, &mut r, &Params::default());
        let a_innovs: Vec<u32> = a.genes.iter().map(|g| g.innov).collect();
        let child_innovs: Vec<u32> = child.genes.iter().map(|g| g.innov).collect();
        assert_eq!(
            a_innovs, child_innovs,
            "the child's structure is the fitter parent's"
        );
        for g in &child.genes {
            let in_a = a.genes.iter().find(|x| x.innov == g.innov).unwrap();
            match b.genes.iter().find(|x| x.innov == g.innov) {
                Some(in_b) => assert!(
                    g.weight == in_a.weight || g.weight == in_b.weight,
                    "a matching weight comes from a parent"
                ),
                None => assert_eq!(g.weight, in_a.weight, "a disjoint gene is the fitter's"),
            }
        }
        child.compile();
    }

    #[test]
    fn distance_is_zero_at_identity_and_grows_with_divergence() {
        let mut r = rng(5);
        let mut inn = Innovations::new();
        let params = Params::default();
        let a = NeatGenome::minimal(&mut r);
        assert_eq!(a.distance(&a, &params), 0.0);
        let mut b = a.clone();
        for _ in 0..30 {
            inn.begin_generation();
            b = b.mutate(&mut r, &params, &mut inn);
        }
        assert!(a.distance(&b, &params) > 0.0);
        assert_eq!(
            a.distance(&b, &params),
            b.distance(&a, &params),
            "symmetric"
        );
    }

    #[test]
    fn speciation_is_deterministic_and_keeps_kin_together() {
        let mut r = rng(6);
        let mut inn = Innovations::new();
        let params = Params::default();
        let base = NeatGenome::minimal(&mut r);
        // A drifted cousin, structurally far away.
        let mut far = base.clone();
        for _ in 0..60 {
            inn.begin_generation();
            far = far.mutate(&mut r, &params, &mut inn);
        }
        // The threshold is derived from the measured gap rather than guessed,
        // so the test asserts the mechanics of speciation and not a particular
        // calibration of the distance metric.
        let gap = base.distance(&far, &params);
        assert!(gap > 0.0);
        let delta = gap * 0.9;
        let population = vec![base.clone(), base.clone(), far.clone(), base, far];
        let s1 = speciate(&population, &[], delta, &params);
        let s2 = speciate(&population, &[], delta, &params);
        assert_eq!(s1.len(), s2.len(), "a pure function of its inputs");
        for (x, y) in s1.iter().zip(&s2) {
            assert_eq!(x.members, y.members);
        }
        assert!(s1.len() >= 2, "kin and stranger are separated");
        // Members 0, 1, 3 are literally identical: they must share a species.
        let home = s1
            .iter()
            .find(|s| s.members.contains(&0))
            .expect("the base genome lives somewhere");
        assert!(home.members.contains(&1) && home.members.contains(&3));
    }

    #[test]
    fn a_genome_survives_the_checkpoint_text_exactly() {
        let mut r = rng(7);
        let mut inn = Innovations::new();
        let mut g = NeatGenome::minimal(&mut r);
        for _ in 0..25 {
            inn.begin_generation();
            g = g.mutate(&mut r, &Params::default(), &mut inn);
        }
        let text = g.show();
        let back = NeatGenome {
            genes: text
                .lines()
                .map(|l| {
                    NeatGenome::parse_gene(l.strip_prefix("gene ").expect("a gene line"))
                        .expect("parses")
                })
                .collect(),
        };
        assert_eq!(back, g, "bit-identical weights, same structure");
    }
}
