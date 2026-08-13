//! What recording costs, and what a game weighs (§7.5).
//!
//! `cargo run --release --example bench_record`

use carranta_bot::{Heuristic, Policy};
use carranta_core::state::TradeMode;
use carranta_core::{Phase, State};
use carranta_record::{Event, Log, Recorder, SeatId, Viewer, fog::project};
use std::time::Instant;

fn bots(seed: u64) -> Vec<Heuristic> {
    (0..4).map(|i| Heuristic::new(seed * 7 + i)).collect()
}

/// One recorded game.
fn recorded(seed: u64, mode: TradeMode) -> Log {
    let mut ps = bots(seed);
    let mut rec = Recorder::new(
        seed,
        seed,
        State::new(4, seed).with_trade_mode(mode),
        (0..4).map(|i| SeatId::agent(i, "heuristic", 1)).collect(),
    );
    let mut buf = Vec::new();
    for _ in 0..20_000 {
        if matches!(rec.state().phase, Phase::GameOver { .. }) {
            break;
        }
        rec.state().legal_into(&mut buf);
        if buf.is_empty() {
            break;
        }
        let seat = rec.state().decider() as usize;
        let a = ps[seat].choose(rec.state(), &buf);
        if rec.apply(a).is_err() {
            break;
        }
    }
    let winner = match rec.state().phase {
        Phase::GameOver { winner } => Some(winner),
        _ => None,
    };
    rec.finish_into(winner)
}

/// The same game with no recorder, for the overhead comparison.
fn plain(seed: u64, mode: TradeMode) {
    let mut ps = bots(seed);
    let mut s = State::new(4, seed).with_trade_mode(mode);
    let mut buf = Vec::new();
    for _ in 0..20_000 {
        if matches!(s.phase, Phase::GameOver { .. }) {
            break;
        }
        s.legal_into(&mut buf);
        if buf.is_empty() {
            break;
        }
        let seat = s.decider() as usize;
        let a = ps[seat].choose(&s, &buf);
        if s.apply(a).is_err() {
            break;
        }
    }
    std::hint::black_box(s.phase);
}

fn main() {
    println!("game records\n");
    println!("in memory: Event = {} B", size_of::<Event>());
    println!("           State = {} B\n", size_of::<State>());

    for mode in [TradeMode::Disabled, TradeMode::Full] {
        let games = 300u64;

        let t = Instant::now();
        for g in 0..games {
            plain(g, mode);
        }
        let bare = t.elapsed().as_secs_f64() / games as f64;

        let t = Instant::now();
        let logs: Vec<Log> = (0..games).map(|g| recorded(g, mode)).collect();
        let with_log = t.elapsed().as_secs_f64() / games as f64;

        let events: usize = logs.iter().map(|l| l.events.len()).sum();
        let snaps: usize = logs.iter().map(|l| l.snapshots.len()).sum();
        let bytes: usize = logs
            .iter()
            .map(|l| l.events.len() * size_of::<Event>() + l.snapshots.len() * size_of::<State>())
            .sum();

        // Replay: the cost analytics pays to re-fold a corpus.
        let t = Instant::now();
        let mut sink = 0u32;
        for l in &logs {
            sink += l.replay().expect("replay").to_act as u32;
        }
        let replay = t.elapsed().as_secs_f64() / games as f64;
        std::hint::black_box(sink);

        // Verify additionally checks every snapshot.
        let t = Instant::now();
        for l in &logs {
            l.verify().expect("verify");
        }
        let verify = t.elapsed().as_secs_f64() / games as f64;

        // Serving one seat: replay plus a redacted position per event.
        let t = Instant::now();
        let mut seen = 0usize;
        for l in &logs {
            seen += project(l, Viewer::Seat(0)).expect("project").len();
        }
        let proj = t.elapsed().as_secs_f64() / games as f64;
        std::hint::black_box(seen);

        println!("{mode:?} market, {games} games:");
        println!("  events/game        {:.0}", events as f64 / games as f64);
        println!("  snapshots/game     {:.1}", snaps as f64 / games as f64);
        println!(
            "  in-memory/game     {:.1} KB   (not the wire format)",
            bytes as f64 / games as f64 / 1024.0
        );
        println!(
            "  play + record      {:.0} µs   (vs {:.0} µs unrecorded, {:+.1}%)",
            with_log * 1e6,
            bare * 1e6,
            (with_log / bare - 1.0) * 100.0
        );
        println!("  replay             {:.0} µs", replay * 1e6);
        println!("  verify             {:.0} µs", verify * 1e6);
        println!("  project one seat   {:.0} µs\n", proj * 1e6);
    }
}
