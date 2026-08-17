# Carranta engine, performance targets

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
| Longest road, realistic network, 15 roads | ≤ 100 ns | **91 ns** | met |
| Longest road, realistic, 5 roads | ≤ 40 ns | **32 ns** | met |
| Longest road, realistic, 15 roads, blocked | ≤ 150 ns | **111 ns** | met |
| Longest road, four-player sweep | ≤ 400 ns | **293 ns** | met |
| Longest road, dense/adversarial, 15 roads | ≤ 500 ns | **1 455 ns** | 2.9× off |
| Whole game, all seats after every move | ≤ 10 µs | **6.8 µs** | met |
| Apply one action | ≤ 50 ns | **~35 ns** | met |
| Legal move generation | ≤ 200 ns | **~22 ns** | met |
| State clone (search node) | ≤ 20 ns | **~9 ns** (480 B) | met |
| Full random game, setup → win | ≤ 50 µs | **~130 µs** | see below |
| Self-play throughput, one core | ≥ 20 000 games/s | **~7 700** | see below |
| Batched env step, N=1024 | FFI overhead < per-step cost |, | not built |
| Bot win rate vs random | ≥ 99% | **99.76%** | met |
| Bot decision | instant (sub-ms) | **~3.5 µs** | met |
| Recording overhead | ≤ 5% of play | **within noise** | met |
| Replay a recorded game | ≤ 200 µs | **31–36 µs** | met |
| Full analysis per game | ≤ 1 ms | **~200 µs** | met |

Engine figures come from `cargo run --release --example bench_engine`. They
move **±30% between runs** on this machine. The same binary has measured a
full random game at both 102 µs and 141 µs. Treat every figure here as a
magnitude, and do not read a 20% change between two runs as a regression.
Anything smaller than about 1.5× needs repeat runs before it means anything.

**The whole-game target was set against the wrong action count, and the bot
settles it.** It assumed ~300 actions per game. Measured:

| Play | Actions per game |
|---|---|
| Random | ~1 058 |
| Heuristic vs random | ~322 |
| Four heuristics | ~479 |

Random play inflates the count enormously. A random policy trades maritime
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

State is one fixed-size `Copy` struct of **384 bytes**. A few cache lines, so
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
run after **every** action, resource and piece conservation, one owner per
edge and intersection, and the Distance Rule holding continuously (§5.5).
Rule interactions no hand-written case would reach get exercised that way.

## Trade market

The open market (R-7.19) is the only place a **non-active** seat changes the
state, which breaks the single-agent action stream everything else relies on.
It is resolved by having two views rather than one:

- `legal_into` answers for the seat whose decision it is, unchanged, single
  agent, and what search and training consume.
- `legal_for(seat)` answers for any seat, and is what a live server asks per
  connected player.

`apply` accepts a trade action from a non-active seat, so the server can feed
in offers as they arrive.

**Generated proposals put a single resource type on each side**, up to three
cards, and `apply` accepts any shape. That split is forced by arithmetic:
multisets of size 1–3 drawn from five resources give 55 possibilities a side,
so full mixed-type enumeration is ~3 000 candidates per decision, 2-for-2 is
still ~400. Single-type sides give at most 180 before affordability prunes
them, and cover the offers real play makes. Mixed-type deals remain legal and
acceptable; they simply are not enumerated.

**Every market action names its actor.** `AcceptTrade { offer, by }` rather than
`AcceptTrade(offer)`, because the market is the one place where the actor is
not implied by whose turn it is. An offer from the active player is open to
*several* opponents, and inferring the taker picks the wrong one.

**Acceptance races resolve by re-validation, not by locking.** Actions are
serialised, so first-come is simply whichever `apply` lands first; the loser
re-validates, fails with `OfferStale`, and the offer is pruned. An offer whose
proposer has since spent the cards is rejected rather than executed against the
state it was authored in.

### Trading in self-play

The market is not decoration: a bot tuned in a game where nobody trades learns
strategies that will not transfer. Two changes were needed to make it happen.

**One ply cannot value a proposal.** Making an offer changes nothing until it
is taken, so a brilliant offer and an absurd one score identically. The bot
values a proposal by the swap it *would* produce, discounted because it may be
refused, and computes it from the hand alone, since a trade leaves
production, ports, routes and points untouched.

**Opponents never reach `choose` during another seat's turn**, so nobody was
ever asked to accept. `Policy::accepts` is a separate question, and the driver
settles the market after every action.

The first version then papered the table. Every extra proposal scored a little
positive, so offering was always better than getting on with the turn. What
fixed it is a toll on **asking**, cumulative across the turn:

| | Actions/game | Trades/game | Asks per trade | Games/s |
|---|---|---|---|---|
| No market | 508 | 0 |, | 594 |
| Market, no toll | 2 941 | 37.4 | ~12 | 29 |
| Toll per *live* own offer | 843 | 27.6 | ~12 | 114 |
| Toll per *request made* | **643** | **19.2** | **6.7** | **165** |

The distinction matters more than it looks. A toll on live offers resets the
moment one is taken, so the bot could churn, offer, get taken, offer again at
no cost, indefinitely. A toll on requests made is monotonic within the turn, so
the bar rises with each ask and only a clearly good trade is worth raising. It
is also closer to how a person weighs it: the third request of the same turn
has to be worth more than the first, because people stop listening.

Every candidate proposal carries the same toll, so it never picks *between*
offers. It only decides whether making any is worth more than getting on with
the turn.

Trades happen in 100% of self-play games. `Full` runs ~4× slower than a
market-free game, almost all of it the larger generated action space, fine for
evaluation and data generation, and the reason `Restricted` (328 games/s) and
`Disabled` exist for training.

### What it cost

State grew from 384 to **480 bytes**, and a clone from ~6 ns to **~9 ns**, the
market is 8 offer slots plus per-turn counters.

*(An earlier revision of this document put the clone at ~14 ns and called it a
2× regression. That was a single unlucky sample: repeated runs put it at
8.5–9.8 ns, in line with 25% more bytes. It is a caution about this machine's
variance, not about the market.)*

Still well inside the 20 ns target, but it is dead weight in exactly the modes
search uses, since `Disabled` and `Restricted` are what reinforcement learning
and the LLM player run. If search throughput ever becomes binding, lifting the
market out of `State` into the server layer is the obvious move.

**Per-action engine cost is untouched by the market**, because generation
short-circuits when trading is off: apply ~30 ns, legal generation ~16 ns,
~97 ns per action over a whole random game. The market's cost is paid in the
size of the generated action space, and only in the modes that generate it.

## Heuristic bot

One ply of greedy search: copy the state, apply each legal action, score the
result, take the best. Copying is 384 bytes and applying is ~35 ns, so a whole
decision costs ~3.3 µs over ~6 candidates, instant for every job it has
(§9.3).

The score is **competitive**, `value(me) − best value(any opponent)`, which is
what makes blocking fall out for free: the robber goes to whoever is strongest
rather than wherever is nearest.

| | |
|---|---|
| Win rate vs 3 random opponents | **99.76%** over 5 000 boards × 4 seats |
| Win rate by seat | 99.66 / 99.80 / 99.78 / 99.78%, no positional artefact |
| Decision cost | ~3.5 µs (~6 candidates) |
| Four bots, whole game | ~1.8 ms, ~499 actions |

### How the win rate is measured

Two seeding mistakes were live in the harness until they were looked for, and
both flattered the number in ways that are easy to miss:

**Seat rotated with the board.** Putting the bot in seat `g % 4` on board `g`
gives each seat a *disjoint* quarter of the boards. The overall rate survives
that, but the per-seat breakdown does not: four seats measured on four
different sets of boards cannot be compared to each other, so the line that
was supposed to rule out first-player advantage was measuring board luck
instead. [`duel_random`] now plays every board once **from each seat**, which
holds the board fixed and leaves the seat as the only difference.

**Policy seeded from the game seed.** `Heuristic::new(g)` and the game's own
`State::new(_, g)` both build `Rng::new(g)`, so their streams started
identical, and a policy breaks ties from `Stream::Dice`, the same stream the
game rolls dice from. The bot's tie-breaks therefore tracked the dice. The
effect was small (99.73% → 99.76% after fixing, inside run-to-run noise), but
a strength measurement is exactly the place where a correlation between a
player's choices and the game's randomness cannot be waved through. Policy
seeds are now salted away from the game seed.

The bot's seed is held constant across the four seatings of one board, so the
pairing is as tight as it can be: same terrain, same numbers, same dev deck,
same dice, same tie-break stream, seat varied.

### Two things worth knowing about it

**One ply cannot see a follow-up.** Playing a Militia leads to a robber move a
ply away, so its value has to come from a feature (progress toward Largest
Militia) rather than from the search. Likewise a maritime trade is a net loss
of cards and one ply cannot see the build it enables, without a
`build_progress` feature scoring partial progress toward each cost, the bot
never trades at all. That feature is not decoration; it is what makes trading
happen.

**A proposal has to be priced for the seat that would take it.** One ply cannot
value an offer either: making it changes nothing until somebody accepts, so a
brilliant offer and an absurd one score the same, and scoring the swap it *would*
produce means picking whichever offer flatters the offerer most. That is how the
bot came to ask 1.86 cards for every one it put up. The fix is that the offer is
scored as dead unless some opponent would take it, and whether they would take it
is answerable exactly rather than by guess, because they will judge it by the same
rule this bot uses.

**And a trade is judged on the trade, not on the standings.** Accepting only when
the position improves against the best opponent sounds like hard bargaining and is
a refusal to trade: the offering seat picked the offer it liked best, so its gain
is nearly always the larger. That rule took 1 205 offers out of 8 053 in self-play.
Judging the swap on its two hand values, with a weight for how much the other
side's gain counts against one's own, takes 3 709 of 6 974 and makes fewer dead
offers. One asymmetry in it is deliberate: a seat shedding cards it cannot hold is
getting *safer* rather than stronger, so the discard-limit part of their gain is
not charged against mine. Without that the bot turns down three cards for one from
a player over the limit, which was the first thing a test caught.

**Three actions must be scored without being applied**, or the bot cheats.
Applying `BuyDev` draws the real top of the deck, which would tell it whether
the next card is a Victory Point *before* deciding to buy. Applying a robber
move takes a real random card, which would let it choose the victim whose card
it liked best. Rolling has an outcome that has not happened. Each is scored
from what a player legitimately knows. A fairness bug that only surfaced
because the win rate was being measured.

### Optimisation

Scoring re-evaluated all four seats for every candidate, but most actions, buying, trading, upgrading, road-building, cannot change what an opponent's
position is worth. Hoisting the opponent term out of the candidate loop cut the
decision from 4.4 µs to 3.3 µs and left the win rate unchanged to the digit,
which is the check that it was behaviour-preserving rather than merely faster.

## Longest road

The only algorithmically interesting operation in the engine. Full reasoning is
in the module documentation; the summary is that four structural properties
collapse an NP-hard problem, and the implementation is arranged so the cheapest
test runs first.

| Tier | Condition | Cost | Covers |
|---|---|---|---|
| Euler | ≤2 odd-degree intersections | free, parity is a by-product of the flood | straight chains, bare loops |
| Diameter | acyclic, no split cycle | two linear sweeps | branching trees |
| Search | cycle **and** ≥4 odd junctions | contraction, then bounded DFS | rare |

### Skipping the work entirely

[`Tracker`] keeps every seat's length current and recomputes only what can have
changed: a seat whose own roads moved, or one whose network an opponent has
just built across. The latter detected with a single mask test against the
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
| [`Tracker`], recompute only what can have changed | **6.8 µs** |

The last step is the one that mattered, and it is worth stating why. The
earlier versions built an explicit adjacency graph before deciding anything.
But the adjacency is already implicit in the board: `edge_adj(e)` is a
precomputed mask of the roads sharing an intersection with `e`, so flooding a
component is one table load and one OR per road, in registers. Degree parity
falls out of the same pass by XORing each road's two-intersection mask, bit
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
The dense case is the one still short of target, and it is search-dominated, see the remaining ideas below.

### What was tried and rejected

- **Per-node reachability pruning.** Recomputing genuinely reachable edges at
  each search step rather than counting all unused ones. A much tighter bound,
  and **4× slower** (4.4 µs → 18.5 µs): at this graph size the flood costs more
  than the subtrees it prunes.
- **Odd-degree-first start ordering in the search.** A maximum trail nearly
  always starts at an odd junction, so trying those first should hit the bound
  and exit early. Measured slightly **worse** (1 629 → 1 747 ns), the second
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
  it into `Tracker::leader` and it is a straight loss, 0.6× in the early game,
  1.0× on a cold position, 0.8× over a whole game, because `Tracker` had
  already removed the same redundancy by caching. The two ideas were competing
  for one saving and caching got there first. `leader` computes exact lengths;
  the primitive is kept, documented, and not used by default.
- **Criterion.** Would add the core crate's first dependency for confidence
  intervals we do not need at this signal-to-noise ratio.

### Remaining ideas, in expected order of value

1. **Minimum T-join bound** for the dense case. The current cap takes the
   better of two cheap relaxations; the exact minimum parity repair is a
   T-join, via shortest paths between odd junctions plus a subset-DP matching.
   It is the only remaining idea likely to close the adversarial gap, but note
   the arithmetic before starting: a subset DP over 8–10 odd junctions is on
   the order of 10–20k operations, which at this graph size is comparable to
   the search it would replace. It should be gated on a small odd-junction
   count, and it must be measured rather than assumed.
2. ~~Exploiting that the exact length is rarely needed.~~ Built and measured;
   see the rejected list above. The saving is real but small, and it overlaps
   almost entirely with what `Tracker` already achieves.

## Game records

`carranta-record` stores a game as an ordered event log, replays it as a fold,
and serves it through a per-viewer projection (§7). Measured over 300 recorded
self-play games per mode:

| | Trading off | Open market |
|---|---|---|
| Events per game | 510 | 708 |
| Snapshots per game | 8.5 | 11.6 |
| In-memory size | 27.9 KB | 38.6 KB |
| Play + record | 1 930 µs | 8 254 µs |
| Play, unrecorded | 1 933 µs | 8 453 µs |
| Replay | 31 µs | 36 µs |
| Verify (replay + every snapshot) | 33 µs | 36 µs |
| Project one seat's whole view | 223 µs | 276 µs |

**Recording is free.** The recorder is an observer: it applies the action the
engine would have applied and appends 48 bytes. Both measurements above put a
recorded game marginally *faster* than an unrecorded one, which is measurement
noise saying the same thing, the cost is below what this harness can see. A
test asserts the stronger claim directly: same seed, same bots, with and
without a log, same game.

**Replay is ~55× cheaper than play**, because it skips move generation and
policy evaluation entirely. A fold over recorded actions, with no search for
what is legal and no bot deciding. A million games re-fold in well under a
minute on one core, which is what makes derived analytics regenerable rather
than something to store carefully (H-7).

**Projection costs ~7× a replay.** It recomputes each seat's longest route for
every event, since route length is public derived state (§4.3). That is the
obvious thing to make incremental with [`Tracker`] if serving ever needs it;
at ~250 µs to project an entire finished game, nothing needs it yet.

### Two design points worth recording

**Randomness is recorded, not re-rolled.** `apply_recorded` reports what an
action resolved; `apply_scripted` supplies it back on replay without touching
the generator (H-1). The seam is narrow because only two things resolve
randomly during play, the dice and the robbery. The board and the deck order
are drawn once at construction and travel in the opening state.

Because a scripted apply leaves the RNG alone, a replayed state matches the
recorded one in every field *but* `rng`. Comparison therefore goes through
`State::same_game_as`, which copies the other side's generator across before
comparing, so every field, including any added later, is checked, and only
the generator is exempt. `apply` itself is unchanged at ~31 ns: the source is
a constant at that call site and folds away.

**`replay()` deliberately refuses the snapshot shortcut.** Seeking from the
last snapshot makes replaying a whole log fast and almost useless. It steps
over the events and trusts the index to stand in for them. A tampered event in
the middle of a log went unnoticed until this was separated: `replay_to(seq)`
seeks and trusts the index, `replay()` folds every event, and `verify()` folds
and checks every snapshot on the way past.

## Analytics

`carranta-analytics` computes §10 over recorded games: dice fairness, the
production decomposition, per-game descriptives, corpus balance, and ratings.
It stores nothing. Replaying a game costs ~35 µs, so a changed metric is
recomputed over the corpus rather than migrated, which is what makes H-7's
"derived events are a materialized view" a practical stance.

A full analysis is **~200 µs per game against ~7 600 µs to play one**, so
re-deriving every metric over a million games is a few minutes on one core.

The statistics are written out rather than pulled in. The workspace stays
dependency-free, and each is tested against values that can be checked
independently: `normal_cdf` and `chi_squared_p` against standard tables, the
least-squares fit against a line it must recover exactly, Benjamini–Hochberg
against a uniform p-value set it must reject entirely.

### Three tests worth naming

**The per-game dice p-value is calibrated.** 400 fair games, and about 5% must
clear p<0.05, because under the null a p-value is uniform. If they did not,
the test would be miscalibrated, and §10.1's entire warning about per-game
significance would rest on nothing.

**The production identity is asserted, not assumed.**
`Actual = E_raw − RobberCost − SupplyDenial + DiceLuck` holds to 1e-9 across
every seat and resource of 30 games. Without it the four terms would be four
separately-computed numbers that happen to sit near each other.

**A sorted roll sequence must fail the audit.** Perfect marginals, obviously
not independent. The distribution checks pass it; the lag-1 autocorrelation and
the runs test catch it. That pair is the reason §10.1 asks for independence
checks at all, and the test is what proves they work rather than merely exist.

The engine's own dice clear the audit on 115 595 pooled rolls: chi-squared 5.8
(p = 0.83), KL 0.000037 bits, worst outcome share off by 0.116 percentage
points, sevens at 0.1661 against 0.1667, lag-1 +0.0012, runs test p = 0.96.

### A harness defect the numbers exposed

The first full report showed a busy market, ~35 offers per seat, and **zero
completed trades**. The bot crate's `settle_market` writes straight to a
`State`, bypassing the recorder, so every completed trade was missing from the
log. The tests passed throughout: none of them asserted that trades appear in a
Full-market game. Fixed by settling through the recorder, which also lets
declines be recorded per offer and seat (H-4) rather than once per pass of the
settle loop.

The same report ranked four *identical* bots, because each agent sat in a fixed
seat. Rotating the agents around the table collapsed the ranking to 1.3σ, not separated, which is the right answer for identical players, and a concrete
demonstration of why A-4 randomises seating.

## Correctness

24 tests. ~40 000 differential comparisons against a brute-force reference, an
obviously-correct exhaustive trail search with no contraction, tiers or bounds, half of them with opponent buildings scattered over the network, and running
up to the full 15-road piece limit so that the search tier and its bounds are
actually exercised. A further ~48 000 checks confirm [`Tracker`]'s cached
answers against full recomputation after every move of simulated games, and
~12 000 more check `longest_road_exceeds` against the exact length at *every*
floor from zero past the true answer. The floored path prunes hardest, so it
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

### Analytics

48 tests. The special functions are checked against published table values, the
statistical tests against sequences whose answer is known by construction
(fair dice, a loaded die, a sorted sequence, a line), and the rating update
against the properties the model is defined by, order monotonicity, σ that
only shrinks, symmetry under a full tie, μ conservation under equal
uncertainty, and convergence onto a known true ordering.

Two limits, stated because they are real: the rating update has **not** been
cross-checked against a reference implementation, and its handling of ties is
counterintuitive in a way that is pinned by a test but not yet resolved. See
§10.7 of the scoping document.

### Records

16 tests over recorded self-play. Replay fidelity is checked against 40 whole
games; snapshot seeking is cross-checked against folding from event zero;
tampering with a recorded die, claiming randomness an action never resolves,
and substituting an illegal action are each asserted to fail *loudly*, with the
sequence number, rather than replaying into a different game.

The redaction tests assert **indistinguishability** rather than checking
fields. Swap two cards between two hands, or reverse the undrawn portion of the
deck, and every other viewer's projection must be byte-identical. A
field-by-field check only ever covers the fields someone remembered, and
silently stops covering a field added later; this formulation covers whatever
the type contains. It is backed by a served type that has no field for another
seat's card identities and none for the deck order, so a leak takes a
deliberate change rather than an oversight, which matters because, per §7.6, a
redaction leak surfaces only when somebody exploits it.

## Method notes

- **Batch timing.** Per-call `Instant::now()` cost and scheduler preemption
  produced 30 µs "worst cases" on pure trees, which are algorithmically
  impossible, measurement artefacts, not outliers.
- **Both shapes reported.** A single mean over random networks hides an ~16×
  spread between realistic and adversarial cases.
- **`black_box` on inputs and outputs**, and the accumulated result is asserted
  against the expected value so the optimiser cannot delete the work.
