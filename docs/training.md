# Training on the MacBook

The phase-two run: classic NEAT over the 32-feature observation, in the full
market with mixed offers, on one laptop. Everything here is
`carranta-evolve --method neat`; the phase-one evolution strategy keeps its
own flags and its own checkpoint format and is not covered again.

## Before the first run

Build in release and let the machine tell you what it can do:

```sh
cargo build --release -p carranta-evolve
cargo run --release -p carranta-analytics --example bench_evolution
```

The bench plays *heuristic* games, and its number is an upper bound you will
not see: a network seat evaluates the observation once per candidate action,
and a mixed market at the 2 v 2 caps enumerates hundreds of candidates per
trading decision, so expect network games to run two orders of magnitude
slower than heuristic ones. That is the price of the trading repertoire, and
it is paid per game, so the honest first step is to run three generations and
read the `seconds` column of `history.csv` before promising the run anything:

```sh
cargo run --release -p carranta-evolve -- --method neat --out runs/probe --generations 3
```

On the scoping document's throughput reasoning (E-5), early generations are
cheap because bad genomes are cheap to tell apart; the budget doubles itself
as the population converges, so later generations cost more by design. Plan
in wall-clock days, not games.

## Starting the real run

```sh
caffeinate -i -s cargo run --release -p carranta-evolve -- \
  --method neat --out runs/neat-1 --generations 0
```

- `caffeinate -i -s` holds off idle and system sleep for exactly as long as
  the training process lives. Keep the laptop on power; on battery, macOS
  sleeps on lid-close regardless, so either leave the lid open or stay
  plugged in.
- `--generations 0` runs until told to stop.
- The defaults are the training configuration of record: population 96, the
  full market, mixed offers capped at 2 cards a side, three generated asks
  per seat per turn (E-15). `--give-cap hand` lifts the give cap to whatever
  the hand holds, `--want-cap N` moves the ask side, `--ask-cap N` the
  allowance; all three are part of the market the champion learns, so
  changing any of them mid-line means a new run, not a resumed one.

## While it runs

The run directory is the whole interface:

- `checkpoint.txt`: the entire run state, written atomically after every
  generation. Plain text; format 3 is NEAT with the ask allowance, format 2
  a NEAT run from before it (readable, and resumed uncapped, exactly the run
  it was), and the ES loader and this one refuse each other's files.
- `history.csv`: one row per generation. `best_fitness` is mean finishing
  position (lower is better, the anchor sits near 2.5 by symmetry); `gap`
  and `gap_ci` are the champion's paired match against the anchor (E-16),
  negative ahead; `species`, `champion_nodes` and `champion_genes` say what
  the complexification is doing. The behaviour columns (trades, settlements,
  roads, and so on) come from the sampled games and say *what* changed when
  the rating says something did.
- `champion.net`: the current champion, re-exported every generation, also
  atomically. This file is the deployment artifact and is complete in
  itself: inputs, links, weights and the generation it came from.
- Standings print each generation: every champion is on the ladder under a
  `g<generation>-<id>` label, rated on held-out games it was never selected
  on (E-10), with the heuristic pinned as the scale's zero point (E-11).

To stop cleanly, create a file named `stop` in the run directory:

```sh
touch runs/neat-1/stop
```

The generation in flight finishes, the checkpoint is written, the process
exits. Interrupting it instead costs at most the generation in progress.

To continue:

```sh
caffeinate -i -s cargo run --release -p carranta-evolve -- \
  --method neat --out runs/neat-1 --resume --generations 0
```

Resume is exact: the continued run plays the same games and breeds the same
genomes it would have without the stop, under any `--threads`, on ARM or
x86. Networks evaluate in a fixed order with a rational squash, so a
champion trained on the MacBook plays move-for-move the same games anywhere.

## Deploying a champion

First sit at a table with it locally:

```sh
cargo run --release -p carranta-ui -- --trained runs/neat-1/champion.net
```

Every seat a person does not hold is played by the champion, the table
enumerates the mixed shapes it trained under, and the game file records each
of its chairs as `trained@<generation>`: a distinct player per E-8, so its
rating never pools with the heuristic's or with another checkpoint's.

The live site takes champions through the repository's `bots/` directory:

```sh
cp runs/neat-1/champion.net bots/trained-42.net
git add bots && git commit && git push
```

The image copies `bots/` whole and the server offers every network in it as
a player a lobby can put in a chair; nothing is seated until somebody asks,
and the house bot stays the default. Name the file for the generation so the
directory reads like the roster it is. A past generation comes back out of a
run with `--export` (`--export list` names what a checkpoint holds), and
whether one is worth shipping is `--example versus`, never the run's own
chart.

## What the first long run taught (read before starting another)

The 378-generation run produced constant traders that measured worse than
the heuristic in a paired match, while the run's own `+anchor` column read
+20 to +35 throughout. Both are explained, and both are now built, E-15 and
E-16 in the scoping document.

**E-15, the ask allowance.** The engine generates at most `--ask-cap`
proposals per seat per turn (default 3), and a served table seating a
champion uses the same number. Legality never moves, so people compose
freely; what changes is what a policy can spend, which is what gives an ask
an opportunity cost the fitness can feel. Runs before the allowance resume
uncapped from their format 2 checkpoints, exactly the runs they were; new
runs write format 3.

**E-16, the honest scoreboard.** The `gap` column is a paired anchor-only
match, the `versus` method: every validation seed played in all six
seatings, one observation per seed, 95% interval beside it. Negative is
ahead of the heuristic, and a gap inside its interval has not been shown to
exist. Expect early generations to read +1.5 or worse, because that is where
they really are; the number to celebrate is the first one that clears zero
from below. To judge a champion trained before this at today's rules, use
`--example versus --ask-cap 20`, the market it actually learned.

## What not to do

- Do not hand-edit `checkpoint.txt` or `champion.net`; both are exact-text
  formats where the printed floats *are* the numbers.
- Do not compare fitness across runs with different caps or modes. The
  fitness is a position in a specific market; the rating over the pinned
  anchor is the only number that travels.
- Do not delete `runs/` while a `--resume` is intended: the checkpoint is
  the run. `runs/` is gitignored, so a champion worth keeping must be copied
  out (the deployment step above does exactly that).
