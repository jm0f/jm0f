# Carranta engine — performance targets

Targets are deliberately aggressive. Each row carries a **measured baseline**
where one exists, so the gap is visible rather than aspirational. Nothing here
may be quoted as a fact about the engine unless the "measured" column is
filled in.

Reproduce with:

```
cargo run --release --example bench_longest_road
```

Measurements below: x86-64, `opt-level=3`, `lto="fat"`, `codegen-units=1`,
single core, batched timing (per-call `Instant::now()` costs tens of ns and its
scheduler noise swamps a sub-microsecond measurement).

## Targets

| Operation | Target | Measured | Status |
|---|---|---|---|
| Longest road — realistic network, 15 roads | ≤ 100 ns | **364 ns** | 3.6× off |
| Longest road — realistic, 5 roads | ≤ 40 ns | **140 ns** | 3.5× off |
| Longest road — dense/adversarial, 15 roads | ≤ 500 ns | **2 341 ns** | 4.7× off |
| Longest road — four-player sweep | ≤ 400 ns | **1 004 ns** | 2.5× off |
| Apply one action | ≤ 50 ns | — | not built |
| Legal action mask, full turn | ≤ 200 ns | — | not built |
| State clone (MCTS node) | ≤ 20 ns | — | not built |
| Full random game, setup → win | ≤ 50 µs | — | not built |
| Self-play throughput, one core | ≥ 20 000 games/s | — | not built |
| Batched env step, N=1024 | FFI overhead < per-step cost | — | not built |

The full-game target is the demanding one: ~300 actions at ≤ 50 µs means an
average of ~160 ns per action *including* production, legality and scoring.
That is the number every other line has to serve.

## Longest road

The only algorithmically interesting operation in the engine, and the one most
likely to dominate a profile. Full reasoning is in the module documentation;
the summary is that four structural properties collapse an NP-hard problem:

1. Blocked intersections **split** the graph, so opponent buildings become
   shape rather than special cases.
2. Degree-2 chains **contract** to weighted edges — an optimal route never ends
   inside a chain, so chains are traversed whole or not at all. A 15-road
   network with three junctions searches ~4 weighted edges rather than 15.
3. Acyclic components are **weighted diameters** — two sweeps, no search.
4. Components with ≤2 odd-degree junctions are **Eulerian** — the answer is the
   total weight, no search.

Only a component with both a cycle *and* ≥4 odd junctions searches at all, and
there it is capped at `total − (the k−1 lightest chains)`.

### Why the shape of the network matters more than its size

| Network shape | 15 roads |
|---|---|
| Realistic growth (97% acyclic) | 364 ns |
| Uniform-random growth (dense, loop-heavy) | 2 341 ns |

Real play grows roads outward from existing ends, which is overwhelmingly
tree-shaped; loops are rare and small. Benchmarking against uniformly random
networks overstates the cost by ~6×, which is why the benchmark reports both.

### What was tried and rejected

- **Per-node reachability pruning.** Recomputing the genuinely reachable edge
  count at each search step, instead of the count of all unused edges. It is a
  much tighter bound and it made things **4× slower** (4.4 µs → 18.5 µs): at
  this graph size the flood costs more than the subtrees it prunes. Removed.
- **Criterion.** Would add the core crate's first dependency for confidence
  intervals we do not need at this signal-to-noise ratio.

### Remaining ideas, in expected order of value

1. **Incremental caching** — recompute only when a player's roads change, or
   when an opponent's building lands on their network. This is the largest
   practical win by far and belongs in the engine, not this module: it turns
   ~200 calls per game into ~60.
2. **Tighter bound via minimum T-join.** The current cap sheds the `k−1`
   lightest chains; the true minimum parity repair is a T-join, computable by
   subset DP over the (few) odd junctions. Would let the dense case terminate
   at the bound instead of searching. Only worth it if the dense case is
   observed in real play — it costs work on the common path.
3. **Avoiding the split-graph rebuild** when `blocked` is empty, which is the
   common case during most of a game.

## Method notes

- **Batch timing.** Per-call `Instant::now()` cost and scheduler preemption
  produced 30 µs "worst cases" on pure trees, which are algorithmically
  impossible — measurement artefacts, not outliers.
- **Both shapes reported.** A single mean over random networks hides the fact
  that realistic and adversarial cases differ by ~6×.
- **`black_box` on inputs and outputs**, and the accumulated result is asserted
  against the expected value so the optimiser cannot delete the work.
