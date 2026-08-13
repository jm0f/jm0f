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
| Longest road — realistic network, 15 roads | ≤ 100 ns | **91 ns** | met |
| Longest road — realistic, 5 roads | ≤ 40 ns | **32 ns** | met |
| Longest road — realistic, 15 roads, blocked | ≤ 150 ns | **111 ns** | met |
| Longest road — four-player sweep | ≤ 400 ns | **293 ns** | met |
| Longest road — dense/adversarial, 15 roads | ≤ 500 ns | **1 629 ns** | 3.3× off |
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

The only algorithmically interesting operation in the engine. Full reasoning is
in the module documentation; the summary is that four structural properties
collapse an NP-hard problem, and the implementation is arranged so the cheapest
test runs first.

| Tier | Condition | Cost | Covers |
|---|---|---|---|
| Euler | ≤2 odd-degree intersections | free — parity is a by-product of the flood | straight chains, bare loops |
| Diameter | acyclic, no split cycle | two linear sweeps | branching trees |
| Search | cycle **and** ≥4 odd junctions | contraction, then bounded DFS | rare |

### Optimisation history

| Change | Realistic 15 roads |
|---|---|
| First working version | 364 ns |
| Tier reorder (Euler before tree) + padded tables | 321 ns |
| Intersection-space flood, no graph build | 204 ns |
| Edge-space flood with fused parity | **91 ns** |

The last step is the one that mattered, and it is worth stating why. The
earlier versions built an explicit adjacency graph before deciding anything.
But the adjacency is already implicit in the board: `edge_adj(e)` is a
precomputed mask of the roads sharing an intersection with `e`, so flooding a
component is one table load and one OR per road, in registers. Degree parity
falls out of the same pass by XORing each road's two-intersection mask — bit
`v` of the accumulator ends up as `degree(v) & 1`, which is the entire input to
the Euler test. So the common case now answers without building, traversing, or
allocating anything.

Supporting changes: hot lookup tables are padded to 256 entries so a `u8` index
is provably in bounds and the compiler drops the check; contraction and the
explicit graph build were moved *inside* the search tier, since no other tier
needs them.

### Why the shape of the network matters more than its size

| Network shape | 15 roads |
|---|---|
| Realistic growth (97% acyclic) | 91 ns |
| Uniform-random growth (dense, loop-heavy) | 1 629 ns |

Real play grows roads outward from existing ends, which is overwhelmingly
tree-shaped; loops are rare and small. Benchmarking against uniformly random
networks overstates the cost by ~18×, which is why the benchmark reports both.
The dense case is the one still short of target, and it is search-dominated —
see the remaining ideas below.

### What was tried and rejected

- **Per-node reachability pruning.** Recomputing genuinely reachable edges at
  each search step rather than counting all unused ones. A much tighter bound,
  and **4× slower** (4.4 µs → 18.5 µs): at this graph size the flood costs more
  than the subtrees it prunes.
- **Odd-degree-first start ordering in the search.** A maximum trail nearly
  always starts at an odd junction, so trying those first should hit the bound
  and exit early. Measured slightly **worse** (1 629 → 1 747 ns) — the second
  pass costs more than the early exits save.
- **Criterion.** Would add the core crate's first dependency for confidence
  intervals we do not need at this signal-to-noise ratio.

### Remaining ideas, in expected order of value

1. **Incremental caching** — recompute only when a player's roads change, or
   when an opponent's building lands on their network. The largest practical
   win by far, and it belongs in the engine rather than this module: it turns
   ~200 calls per game into ~60.
2. **Tighter bound via minimum T-join** for the dense case. The current cap
   sheds the `k−1` lightest chains; the true minimum parity repair is a T-join,
   computable by subset DP over the few odd junctions. This is the most likely
   route to bringing the adversarial case under target, and it costs nothing on
   the common path because it only runs in the search tier.
3. **Skipping the split-graph rebuild** in the search tier when the component
   has no blocked intersections.

## Correctness

18 tests, including ~40 000 differential comparisons against a brute-force
reference — an obviously-correct exhaustive trail search with no contraction,
tiers or bounds — half of them with opponent buildings scattered over the
network.

That differential test earned its keep. It caught two bugs that every
hand-written case missed, both in blocking:

- The second diameter sweep can legitimately *start* at a blocked
  intersection, since a route may end at one. Refusing to expand it there
  reported a length of 0.
- "In a tree component every blocked intersection is a leaf" is **false**. A
  cycle running *through* a blocked intersection becomes a tree once split, but
  intersection-space traversal still sees the cycle, so the diameter shortcut
  silently under-reported. Those components are now detected and routed to the
  general path.

## Method notes

- **Batch timing.** Per-call `Instant::now()` cost and scheduler preemption
  produced 30 µs "worst cases" on pure trees, which are algorithmically
  impossible — measurement artefacts, not outliers.
- **Both shapes reported.** A single mean over random networks hides an ~18×
  spread between realistic and adversarial cases.
- **`black_box` on inputs and outputs**, and the accumulated result is asserted
  against the expected value so the optimiser cannot delete the work.
