//! A feed-forward network, compiled from an arbitrary NEAT topology.
//!
//! ## Why the arithmetic is what it is
//!
//! The workspace's measuring sticks are integer-weighted on purpose: float
//! results can differ across platforms and quietly invalidate a benchmark.
//! Evolved weights want to be continuous, so this walks a narrower line
//! instead: `f64`, restricted to addition, multiplication, division,
//! absolute value and comparison. Every one of those is exactly specified by
//! IEEE 754, bit-for-bit, on every platform Rust targets. What is banished is
//! the maths library: `tanh` and friends are allowed to differ in the last
//! bit between an ARM MacBook and an x86 server, and a network trained on one
//! and serving on the other must be the same player on both.
//!
//! The activation is therefore **softsign**, `x / (1 + |x|)`: bounded like a
//! sigmoid, smooth enough to evolve over, and made only of exact operations.
//!
//! ## Node identity
//!
//! Nodes are named by arbitrary ids rather than indices, because a NEAT
//! genome's hidden nodes are born from historical innovations, not counted
//! from zero. The fixed part of the contract: ids `0..inputs` are the inputs,
//! id `inputs` is the bias (always 1.0), id `inputs + 1` is the single
//! output. Hidden ids are whatever history handed out.

use std::collections::HashMap;

/// One evaluation step: a node and its incoming weighted edges, in an order
/// where everything upstream is already computed.
#[derive(Clone, Debug)]
struct Step {
    slot: usize,
    incoming: Vec<(usize, f64)>,
    /// The output node's sum passes through unsquashed: candidates are ranked
    /// by comparison, and a squash that saturates would erase differences
    /// between clearly distinct positions late in training.
    squash: bool,
}

/// A compiled network, ready to evaluate.
#[derive(Clone, Debug)]
pub struct Net {
    inputs: usize,
    steps: Vec<Step>,
    output_slot: usize,
    slots: usize,
    /// The connection list it was assembled from, kept so a network can be
    /// written back out exactly as it was read.
    links: Vec<(u32, u32, f64)>,
}

fn softsign(x: f64) -> f64 {
    x / (1.0 + x.abs())
}

impl Net {
    /// The bias node's id for a network with this many inputs.
    pub fn bias_id(inputs: usize) -> u32 {
        inputs as u32
    }

    /// The output node's id for a network with this many inputs.
    pub fn output_id(inputs: usize) -> u32 {
        inputs as u32 + 1
    }

    /// Compile a connection list into an evaluation order.
    ///
    /// `None` when the links contain a cycle or name the output as a source:
    /// a NEAT mutation is supposed to refuse those, so failing to compile is a
    /// bug upstream and the caller should treat it as one. Disabled genes are
    /// simply not passed in.
    ///
    /// The order is deterministic: nodes become ready in waves, and each wave
    /// is processed in ascending id order, so the same links always compile to
    /// the same steps.
    pub fn assemble(inputs: usize, links: &[(u32, u32, f64)]) -> Option<Net> {
        let bias = Self::bias_id(inputs);
        let output = Self::output_id(inputs);

        // Slot assignment: inputs first, bias, output, then every hidden id in
        // ascending order.
        let mut ids: Vec<u32> = (0..=output).collect();
        let mut hidden: Vec<u32> = links
            .iter()
            .flat_map(|&(f, t, _)| [f, t])
            .filter(|&n| n > output)
            .collect();
        hidden.sort_unstable();
        hidden.dedup();
        ids.extend(&hidden);
        let slot_of: HashMap<u32, usize> = ids.iter().enumerate().map(|(i, &n)| (n, i)).collect();

        // Incoming edges per non-input node.
        let mut incoming: HashMap<u32, Vec<(u32, f64)>> = HashMap::new();
        for &(from, to, w) in links {
            if from == output || to <= bias {
                return None; // the output feeds nothing; nothing feeds an input
            }
            incoming.entry(to).or_default().push((from, w));
        }
        // Deterministic within a node too: edge order changes nothing
        // mathematically, but float addition is not associative, and the sum
        // must come out identical wherever it is computed.
        for edges in incoming.values_mut() {
            edges.sort_unstable_by_key(|&(from, _)| from);
        }

        // Kahn's algorithm, ascending id order within each wave.
        //
        // A node that appears only as a source, every incoming gene disabled,
        // is a real state a genome passes through: it computes softsign(0),
        // contributes nothing, and needs no step. Nothing waits on it.
        let computed = |from: u32| from > bias && incoming.contains_key(&from);
        let mut waiting: HashMap<u32, usize> = incoming
            .iter()
            .map(|(&to, edges)| {
                let upstream = edges.iter().filter(|&&(f, _)| computed(f)).count();
                (to, upstream)
            })
            .collect();
        let mut ready: Vec<u32> = waiting
            .iter()
            .filter(|&(_, &n)| n == 0)
            .map(|(&id, _)| id)
            .collect();
        ready.sort_unstable();
        let mut order: Vec<u32> = Vec::with_capacity(waiting.len());
        let mut downstream: HashMap<u32, Vec<u32>> = HashMap::new();
        for (&to, edges) in &incoming {
            for &(from, _) in edges {
                if computed(from) {
                    downstream.entry(from).or_default().push(to);
                }
            }
        }
        while let Some(id) = ready.first().copied() {
            ready.remove(0);
            order.push(id);
            if let Some(next) = downstream.get(&id) {
                let mut woke: Vec<u32> = Vec::new();
                for &to in next {
                    let n = waiting.get_mut(&to)?;
                    *n -= 1;
                    if *n == 0 {
                        woke.push(to);
                    }
                }
                woke.sort_unstable();
                // Keep the frontier sorted so the wave order is id order.
                for w in woke {
                    let at = ready.partition_point(|&r| r < w);
                    ready.insert(at, w);
                }
            }
        }
        if order.len() != waiting.len() {
            return None; // a cycle
        }

        let steps = order
            .iter()
            .map(|&id| Step {
                slot: slot_of[&id],
                incoming: incoming[&id]
                    .iter()
                    .map(|&(from, w)| (slot_of[&from], w))
                    .collect(),
                squash: id != output,
            })
            .collect();
        Some(Net {
            inputs,
            steps,
            output_slot: slot_of[&output],
            slots: ids.len(),
            links: links.to_vec(),
        })
    }

    pub fn inputs(&self) -> usize {
        self.inputs
    }

    /// Evaluate the network on one observation.
    ///
    /// A node nothing connects to is 0; an output nothing connects to is 0,
    /// which ranks every candidate equally and is the honest value of a
    /// network that has evolved itself blind.
    pub fn eval(&self, observation: &[f64]) -> f64 {
        debug_assert_eq!(observation.len(), self.inputs);
        let mut v = vec![0.0f64; self.slots];
        v[..self.inputs].copy_from_slice(observation);
        v[self.inputs] = 1.0; // bias
        for step in &self.steps {
            let mut sum = 0.0;
            for &(from, w) in &step.incoming {
                sum += v[from] * w;
            }
            v[step.slot] = if step.squash { softsign(sum) } else { sum };
        }
        v[self.output_slot]
    }

    /// The network as a file somebody can read.
    ///
    /// Weights are printed with Rust's shortest round-trip representation, so
    /// what is written parses back to the identical bits: a champion shipped
    /// to a server is the champion that was trained, not a rounding of it.
    pub fn show(&self, generation: u32) -> String {
        use std::fmt::Write as _;
        let mut out = String::new();
        let _ = writeln!(out, "carranta-net 1");
        let _ = writeln!(out, "generation {generation}");
        let _ = writeln!(out, "inputs {}", self.inputs);
        for &(from, to, w) in &self.links {
            let _ = writeln!(out, "link {from} {to} {w:?}");
        }
        out
    }

    /// Read a network written by [`Net::show`]. Answers the generation too,
    /// which is the version a deployed champion plays under.
    pub fn parse(text: &str) -> Option<(Net, u32)> {
        let mut inputs = None;
        let mut generation = 0u32;
        let mut links = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let mut parts = line.split_whitespace();
            match parts.next()? {
                "carranta-net" => {
                    if parts.next()? != "1" {
                        return None;
                    }
                }
                "generation" => generation = parts.next()?.parse().ok()?,
                "inputs" => inputs = Some(parts.next()?.parse().ok()?),
                "link" => {
                    let from = parts.next()?.parse().ok()?;
                    let to = parts.next()?.parse().ok()?;
                    let w = parts.next()?.parse().ok()?;
                    links.push((from, to, w));
                }
                _ => return None,
            }
        }
        Net::assemble(inputs?, &links).map(|n| (n, generation))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tiny_network_computes_what_arithmetic_says_it_should() {
        // Two inputs, bias 2, output 3, one hidden node 9:
        //   h = softsign(0.5 * x0 + 1.0 * bias)
        //   y = 2.0 * h + 3.0 * x1
        let links = [(0, 9, 0.5), (2, 9, 1.0), (9, 3, 2.0), (1, 3, 3.0)];
        let net = Net::assemble(2, &links).expect("acyclic");
        let x0 = 0.5f64;
        let x1 = -0.25f64;
        let h = {
            let s = 0.5 * x0 + 1.0;
            s / (1.0 + s.abs())
        };
        let want = 2.0 * h + 3.0 * x1;
        assert_eq!(net.eval(&[x0, x1]), want, "exactly, not approximately");
    }

    #[test]
    fn a_cycle_refuses_to_compile() {
        // 9 -> 10 -> 9: a mutation is supposed to prevent this, so compiling
        // must fail loudly rather than evaluate something undefined.
        let links = [(0, 9, 1.0), (9, 10, 1.0), (10, 9, 1.0), (10, 3, 1.0)];
        assert!(Net::assemble(2, &links).is_none());
        // As is feeding an input or reading the output.
        assert!(Net::assemble(2, &[(3, 9, 1.0), (9, 3, 1.0)]).is_none());
        assert!(Net::assemble(2, &[(0, 1, 1.0)]).is_none());
    }

    #[test]
    fn a_node_with_every_incoming_gene_disabled_still_compiles() {
        // Disabling is how NEAT prunes, so a hidden node fed by nothing is a
        // state real genomes pass through. It computes softsign(0), which is
        // zero, and must not wedge the whole network.
        let links = [(9, 3, 5.0), (0, 3, 1.0)];
        let net = Net::assemble(2, &links).expect("an orphan source is fine");
        assert_eq!(net.eval(&[0.25, 0.0]), 0.25, "the orphan contributes zero");
    }

    #[test]
    fn show_and_parse_are_exact_inverses() {
        // A champion shipped to a server must be the champion that trained,
        // so the text format has to round-trip float bits exactly.
        let links = [
            (0, 33, 0.1 + 0.2), // deliberately not representable "nicely"
            (31, 33, -1.7976931348623157e308f64 / 1e10),
            (33, 32, 3.0000000000000004),
            (5, 32, f64::MIN_POSITIVE),
        ];
        let net = Net::assemble(31, &links).expect("acyclic");
        let text = net.show(17);
        let (back, generation) = Net::parse(&text).expect("parses");
        assert_eq!(generation, 17);
        assert_eq!(back.links, net.links, "bit-identical weights");
        let obs: Vec<f64> = (0..31).map(|i| (i as f64) / 31.0).collect();
        assert_eq!(back.eval(&obs), net.eval(&obs));
    }

    #[test]
    fn evaluation_order_cannot_depend_on_link_order() {
        // Float addition is not associative, so the compiler sorts edges: the
        // same genome must evaluate identically however its genes are listed.
        let a = [(0, 9, 0.3), (1, 9, 0.7), (2, 9, 0.11), (9, 3, 1.0)];
        let mut b = a;
        b.reverse();
        let na = Net::assemble(2, &a).expect("acyclic");
        let nb = Net::assemble(2, &b).expect("acyclic");
        for obs in [[0.1, 0.9], [0.5, 0.5], [123.0, -7.0]] {
            assert_eq!(na.eval(&obs), nb.eval(&obs));
        }
    }
}
