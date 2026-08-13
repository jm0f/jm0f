# Carranta

An original trading-and-settlement board game, implemented as a Rust workspace
with no third-party dependencies at all — every crate here builds against `std`
and nothing else.

The design document is [`docs/carranta-scoping.md`](docs/carranta-scoping.md).

## Getting set up

The only prerequisite is a Rust toolchain, **1.87 or newer**.

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"   # or just open a new terminal
rustc --version             # expect 1.87.0 or later
```

If you already have Rust but it predates 1.87, `rustup update` is enough. The
minimum is declared in the workspace manifest, so an older toolchain says so
instead of failing somewhere inside a crate.

Because there is nothing to download from crates.io, the first build is pure
compilation — a couple of minutes on an M1, and no network access at all.

## Playing in the browser

```sh
./play
```

Start that once and leave it. It pulls, builds, opens the board, and then
watches the branch: when a change is pushed it pulls, rebuilds and restarts,
and the page reloads itself. Nothing to repeat and nothing to remember.

This matters because the interface is a compiled binary. A change reaches the
browser only after a rebuild, and clicking **New game** re-deals from the code
already running — it can never show you something just pushed. The page header
carries the commit it was built from, so a stale process is visible rather than
mistaken for a change that did not work.

`./play --once` starts what is checked out and does not watch.

The long way round, if you would rather see the steps:

```sh
cargo run --release -p carranta-ui
```

Either way, open <http://127.0.0.1:8181>. The server binds loopback only: the
game is local and stays local.

```sh
cargo run --release -p carranta-ui -- --port 9000 --seats 3 --mode restricted
```

| flag | meaning | default |
| --- | --- | --- |
| `--port N` | port to listen on | 8181 |
| `--seats N` | 3 or 4 players | 4 |
| `--seed N` | which board to deal | 1 |
| `--mode` | `full`, `restricted`, or `disabled` trading | `full` |

You are seat 0; the other seats are played by the heuristic bot. Every click is
checked by the same rules engine the training runs use, so an illegal move is
refused rather than quietly allowed.

## Training agents

`carranta-train` evolves the heuristic's fifteen weights, using every core on
the machine. It is meant to be started and left alone:

```sh
cargo run --release -p carranta-evolve -- --out runs/first --generations 0
cargo run --release -p carranta-evolve -- --out runs/first --resume
```

`--generations 0` runs until you stop it; `--help` lists the rest. It prints a
line per generation while it runs:

```text
  gen  trials    games    best  median   noise   sep  spread      +anchor   trades  cities   secs
    1      96     4704  2.1667  2.4167  0.2406    NO    3.00   +4.41 +- 2.2     20.0    0.75    5.4
```

`best` and `median` are mean finishing position, so lower is better and 2.5 is
an average seat. `sep` says whether the field could actually be told apart this
generation — when it reads `NO`, the trainer doubles next generation's budget by
itself. `+anchor` is the champion's rating above the pinned heuristic; read it
against the `+-` beside it, because a gain smaller than its own uncertainty is
not a gain. At the end it prints the ladder and which weights evolution moved.

Three files land in `--out`: `checkpoint.txt`, `history.csv` with a row per
generation for charting, and whatever you drop in as `stop` — creating a file by
that name ends the run cleanly after the generation in flight.

A checkpoint is written after every generation, so an interruption costs at most
the generation in progress. Resume is exact rather than approximate: a run
stopped at generation 40 and resumed produces precisely the games it would have
produced had it never stopped, and the checkpoint is plain text you can read,
diff and salvage without the program that wrote it.

Thread count buys time, not a different answer — every genome is measured on the
same boards from the same seats, so one core and twelve give identical results.
Nothing here is architecture-specific, so an Apple-silicon laptop needs no extra
setup; on a fanless machine it is worth measuring four workers against eight,
because sustained throttling can make more workers slower.

## The crates

| crate | what it is |
| --- | --- |
| `carranta-core` | rules, state, and move generation — the whole game in one fixed-size `Copy` struct |
| `carranta-bot` | a heuristic opponent, used as a baseline and as a sparring partner |
| `carranta-record` | game records, replay, verification, and the per-seat redacted view |
| `carranta-analytics` | ratings, production accounting, and statistical tests over recorded games |
| `carranta-evolve` | population training, the ladder, and checkpointing |
| `carranta-ui` | the local browser interface |

## Checks

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```
