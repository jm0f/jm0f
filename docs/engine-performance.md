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
| Longest road — dense/adversarial, 15 roads | ≤ 500 ns | **1 455 ns** | 2.9× off |
| Whole game, all seats after every move | ≤ 10 µs | **6.8 µs** | met |
| Apply one action | ≤ 50 ns | **~35 ns** | met |
| Legal move generation | ≤ 200 ns | **~22 ns** | met |
| State clone (search node) | ≤ 20 ns | **~14 ns** (480 B) | met |
| Full random game, setup → win | ≤ 50 µs | **~130 µs** | see below |
| Self-play throughput, one core | ≥ 20 000 games/s | **~7 700** | see below |
| Batched env step, N=1024 | FFI overhead < per-step cost | — | not built |
| Bot win rate vs random | ≥ 99% | **99.81%** | met |
| Bot decision | instant (sub-ms) | **~3.3 µs** | met |

Engine figures come from `cargo run --release --example bench_engine`. They
move ±20% between runs on this machine, so treat them as magnitudes.

**The whole-game target was set against the wrong action count, and the bot
settles it.** It assumed ~300 actions per game. Measured:

| Play | Actions per game |
|---|---|
| Random | ~1 058 |
| Heuristic vs random | ~322 |
| Four heuristics | ~479 |

Random play inflates the count enormously — a random policy trades maritime
constantly and builds badly, dragging games out. Competent play is roughly
half that, and the original ~300 estimate was close to right for a game where
someone is actually trying to win.

At ~123 ns per action, a 479-action game costs **~59 µs** of engine time,
which is just outside the 50 µs target rather than 2.6× off it. Take the
per-action line as the real measure; the whole-game row should be read against
~479 actions, not the ~1 058 that random play produces.

The full-game target is the demanding one: ~300 actions at ≤ 50 µs means an
average of ~160 ns per action *including* production, legality and scoring.
That is the number every other line has to serve.

## Engine

State is one fixed-size `Copy` struct of **384 bytes** — a few cache lines, so
cloning a node for search is a `memcpy` and measures ~6 ns. Occupancy lives in
bitboards (one `u128` per player over the 72 edges, one `u64` each for
settlements and cities over the 54 intersections), which is what makes "do I
hold this port", "which of my roads touch here" and the Distance Rule single
mask operations rather than scans.

Legal moves are *generated*, not filtered: a search or policy needs the whole
legal set every step, and building it directly is cheaper than proposing and
rejecting. The two are kept honest by a test asserting that every generated
action is accepted by `apply`.

Correctness rests on random playouts: 300 full games with `assert_invariants`
run after **every** action — resource and piece conservation, one owner per
edge and intersection, and the Distance Rule holding continuously (§5.5).
Rule interactions no hand-written case would reach get exercised that way.

## Trade market

The open market (R-7.19) is the only place a **non-active** seat changes the
state, which breaks the single-agent action stream everything else relies on.
It is resolved by having two views rather than one:

- `legal_into` answers for the seat whose decision it is — unchanged, single
  agent, and what search and training consume.
- `legal_for(seat)` answers for any seat, and is what a live server asks per
  connected player.

`apply` accepts a trade action from a non-active seat, so the server can feed
in offers as they arrive.

**Generated proposals are one-card-for-one-card even in `Full` mode.** The space
of well-formed offers is combinatorial and cannot be enumerated, so the
generated set stays bounded while `apply` still accepts any shape a human
composes. That is the seam between a policy's action space and a person's.

**Acceptance races resolve by re-validation, not by locking.** Actions are
serialised, so first-come is simply whichever `apply` lands first; the loser
re-validates, fails with `OfferStale`, and the offer is pruned. An offer whose
proposer has since spent the cards is rejected rather than executed against the
state it was authored in.

### What it cost

State grew from 384 to **480 bytes** and a clone from ~6 ns to **~14 ns** — the
market is 8 offer slots plus per-turn counters. Still inside the 20 ns target,
but it is dead weight in exactly the modes search uses, since `Disabled` and
`Restricted` are what reinforcement learning and the LLM player run. If search
throughput ever becomes binding, lifting the market out of `State` and into the
server layer is the obvious move.

## Heuristic bot

One ply of greedy search: copy the state, apply each legal action, score the
result, take the best. Copying is 384 bytes and applying is ~35 ns, so a whole
decision costs ~3.3 µs over ~6 candidates — instant for every job it has
(§9.3).

The score is **competitive**, `value(me) − best value(any opponent)`, which is
what makes blocking fall out for free: the robber goes to whoever is strongest
rather than wherever is nearest.

| | |
|---|---|
| Win rate vs 3 random opponents | **99.81%** over 20 000 games, seat rotated |
| Win rate by seat | 99% in all four — no positional artefact |
| Decision cost | ~3.3 µs (~6 candidates) |
| Four bots, whole game | ~1.5 ms, ~479 actions |

### Two things worth knowing about it

**One ply cannot see a follow-up.** Playing a Militia leads to a robber move a
ply away, so its value has to come from a feature (progress toward Largest
Militia) rather than from the search. Likewise a maritime trade is a net loss
of cards and one ply cannot see the build it enables — without a
`build_progress` feature scoring partial progress toward each cost, the bot
never trades at all. That feature is not decoration; it is what makes trading
happen.

**Three actions must be scored without being applied**, or the bot cheats.
Applying `BuyDev` draws the real top of the deck, which would tell it whether
the next card is a Victory Point *before* deciding to buy. Applying a robber
move takes a real random card, which would let it choose the victim whose card
it liked best. Rolling has an outcome that has not happened. Each is scored
from what a player legitimately knows — a fairness bug that only surfaced
because the win rate was being measured.

### Optimisation

Scoring re-evaluated all four seats for every candidate, but most actions —
buying, trading, upgrading, road-building — cannot change what an opponent's
position is worth. Hoisting the opponent term out of the candidate loop cut the
decision from 4.4 µs to 3.3 µs and left the win rate at exactly 99.81%, which
is the check that it was behaviour-preserving rather than merely faster.

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

### Skipping the work entirely

[`Tracker`] keeps every seat's length current and recomputes only what can have
changed: a seat whose own roads moved, or one whose network an opponent has
just built across — the latter detected with a single mask test against the
intersection's roads, and usually true for nobody. That is a 3.6× saving on the
whole-game figure above, and it compounds with every per-call improvement
below rather than competing with them.

### Optimisation history

| Change | Realistic 15 roads |
|---|---|
| First working version | 364 ns |
| Tier reorder (Euler before tree) + padded tables | 321 ns |
| Intersection-space flood, no graph build | 204 ns |
| Edge-space flood with fused parity | **91 ns** |

And at the level that actually matters, the whole-game cost of keeping every
seat's road length current across 80 building moves:

| Change | Whole game |
|---|---|
| Recompute all four seats after every move | 24.0 µs |
| [`Tracker`] — recompute only what can have changed | **6.8 µs** |

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
| Uniform-random growth (dense, loop-heavy) | 1 455 ns |

Real play grows roads outward from existing ends, which is overwhelmingly
tree-shaped; loops are rare and small. Benchmarking against uniformly random
networks overstates the cost by ~16×, which is why the benchmark reports both.
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
- **Flooring the search with the incumbent's length.** The Longest Road tile
  moves only on a strict lead (R-10.6), so the holder's length looked like a
  floor every rival must clear, and every prune in the search is already keyed
  on the best length so far. Built as `longest_road_exceeds` and measured:

  | Floor on a realistic 15-road network | Per call |
  |---|---|
  | 0 (equivalent to the exact call) | 101.8 ns |
  | the network's own length (must still prove it) | 97.5 ns |
  | 99, so every network is dismissed unseen | 86.9 ns |

  Even the *best* case is only ~15% faster, because the component flood runs
  either way and a floor skips only the tier work, which is already cheap. Wire
  it into `Tracker::leader` and it is a straight loss — 0.6× in the early game,
  1.0× on a cold position, 0.8× over a whole game — because `Tracker` had
  already removed the same redundancy by caching. The two ideas were competing
  for one saving and caching got there first. `leader` computes exact lengths;
  the primitive is kept, documented, and not used by default.
- **Criterion.** Would add the core crate's first dependency for confidence
  intervals we do not need at this signal-to-noise ratio.

### Remaining ideas, in expected order of value

1. **Minimum T-join bound** for the dense case. The current cap takes the
   better of two cheap relaxations; the exact minimum parity repair is a
   T-join, via shortest paths between odd junctions plus a subset-DP matching.
   It is the only remaining idea likely to close the adversarial gap — but note
   the arithmetic before starting: a subset DP over 8–10 odd junctions is on
   the order of 10–20k operations, which at this graph size is comparable to
   the search it would replace. It should be gated on a small odd-junction
   count, and it must be measured rather than assumed.
2. ~~Exploiting that the exact length is rarely needed.~~ Built and measured;
   see the rejected list above. The saving is real but small, and it overlaps
   almost entirely with what `Tracker` already achieves.

## Correctness

24 tests. ~40 000 differential comparisons against a brute-force reference — an
obviously-correct exhaustive trail search with no contraction, tiers or bounds
— half of them with opponent buildings scattered over the network, and running
up to the full 15-road piece limit so that the search tier and its bounds are
actually exercised. A further ~48 000 checks confirm [`Tracker`]'s cached
answers against full recomputation after every move of simulated games, and
~12 000 more check `longest_road_exceeds` against the exact length at *every*
floor from zero past the true answer — the floored path prunes hardest, so it
is where a bound set too tight would surface as a short answer. `Tracker::leader`
is checked against a naive compute-everything-then-compare implementation across
16 000 simulated positions, ties and the five-road threshold included.

Both bounds in `shed_bound` are correctness-critical: a bound below the true
maximum would make the search stop early and silently return a short answer.
The differential tests are what stands behind them.

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
- **Both shapes reported.** A single mean over random networks hides an ~16×
  spread between realistic and adversarial cases.
- **`black_box` on inputs and outputs**, and the accumulated result is asserted
  against the expected value so the optimiser cannot delete the work.
