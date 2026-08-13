//! The whole analytics pipeline over a self-play corpus.
//!
//! `cargo run --release -p carranta-analytics --example report`

use carranta_analytics::corpus::{Config, Corpus};
use carranta_analytics::rating::PoolKey;
use carranta_analytics::{dice, game, production};
use carranta_bot::{Heuristic, Policy};
use carranta_core::Action;
use carranta_core::state::{MAX_OFFERS, Phase, State, TradeMode};
use carranta_record::{Log, RULES_VERSION, Recorder, SeatId};
use std::time::Instant;

const AGENTS: [&str; 4] = ["heuristic-a", "heuristic-b", "heuristic-c", "heuristic-d"];

/// Offer everyone entitled to take a live offer the chance to do so, through
/// the recorder so completed trades land in the log.
fn settle(rec: &mut Recorder, bots: &mut [Heuristic]) {
    if rec.state().trade_mode == TradeMode::Disabled || rec.state().offer_count == 0 {
        return;
    }
    let mut declined = [[false; MAX_OFFERS]; 4];
    for _ in 0..16 {
        let mut acted = false;
        #[allow(clippy::needless_range_loop)] // `i` indexes both offers and `declined`
        'outer: for i in 0..rec.state().offer_count as usize {
            for seat in 0..bots.len() {
                if rec.state().offers[i].from as usize == seat {
                    continue;
                }
                let take = Action::AcceptTrade {
                    offer: i as u8,
                    by: seat as u8,
                };
                let mut probe = *rec.state();
                if probe.apply(take).is_err() {
                    continue;
                }
                if bots[seat].accepts(rec.state(), seat, i) {
                    let _ = rec.apply(take);
                    acted = true;
                    break 'outer;
                } else if !declined[seat][i] {
                    declined[seat][i] = true;
                    rec.decline(i as u8, seat as u8);
                }
            }
        }
        if !acted {
            break;
        }
    }
}

/// One recorded game, with the agents rotated around the table.
///
/// Rotation is the point (A-4): without it, an agent's rating would measure
/// its seat rather than its play, and four identical bots would come out
/// ranked.
fn self_play(seed: u64, mode: TradeMode) -> Log {
    let rotation = (seed % 4) as usize;
    let agent_at = |seat: usize| (seat + rotation) % 4;
    let mut bots: Vec<Heuristic> = (0..4)
        .map(|seat| Heuristic::new(seed * 31 + agent_at(seat) as u64 + 1))
        .collect();
    let mut rec = Recorder::new(
        seed,
        seed,
        State::new(4, seed).with_trade_mode(mode),
        (0..4)
            .map(|seat| SeatId::agent(agent_at(seat) as u64, AGENTS[agent_at(seat)], 1))
            .collect(),
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
        let a = bots[seat].choose(rec.state(), &buf);
        if rec.apply(a).is_err() {
            break;
        }
        settle(&mut rec, &mut bots);
    }
    let winner = match rec.state().phase {
        Phase::GameOver { winner } => Some(winner),
        _ => None,
    };
    rec.finish_into(winner)
}

const RESOURCES: [&str; 5] = ["brick", "wood", "wool", "wheat", "ore"];

fn main() {
    let games = 1_000u64;
    let mode = TradeMode::Full;

    println!("carranta analytics — {games} self-play games, {mode:?} market\n");

    let t = Instant::now();
    let logs: Vec<Log> = (0..games).map(|g| self_play(g, mode)).collect();
    let play_time = t.elapsed();

    let t = Instant::now();
    let mut corpus = Corpus::new(Config {
        trade_mode: mode,
        rules_version: RULES_VERSION,
    });
    for log in &logs {
        corpus.add(log, 0);
    }
    let analyse_time = t.elapsed();

    // ---- §10.3 corpus ----
    println!("== corpus ==");
    println!(
        "  games {}   finished {}   mean player-turns {:.1}",
        corpus.games,
        corpus.finished,
        corpus.mean_turns()
    );
    let seats = corpus.seat_win_rate();
    println!(
        "  win rate by seat    {:.1}%  {:.1}%  {:.1}%  {:.1}%   (A-4: first-player advantage)",
        seats[0] * 100.0,
        seats[1] * 100.0,
        seats[2] * 100.0,
        seats[3] * 100.0
    );

    // ---- §10.1b generator audit ----
    let audit = corpus.dice_audit();
    println!("\n== dice: is the generator fair? (§10.1b) ==");
    println!("  pooled rolls        {}", audit.rolls);
    println!(
        "  chi-squared         {:.1}  (p = {:.4}, 10 df)",
        audit.chi_squared, audit.p_value
    );
    println!(
        "  KL from theory      {:.6} bits          <- judge on this, not on p",
        audit.kl_bits
    );
    println!(
        "  worst outcome gap   {:.3} percentage points",
        audit.max_outcome_deviation
    );
    println!(
        "  share of sevens     {:.4}   (expected {:.4})",
        audit.seven_share,
        1.0 / 6.0
    );
    println!(
        "  lag-1 correlation   {:+.4}                 <- marginals alone would miss this",
        audit.lag1_autocorrelation
    );
    println!("  runs test           p = {:.4}", audit.runs_p);

    // ---- §10.1a one game, as a percentile ----
    let reference = corpus.dice_corpus();
    let sample = &logs[0];
    let one = dice::analyse_game(&dice::rolls(sample), dice::DEFAULT_SIMS, 1);
    println!("\n== dice: was game 0 unusual? (§10.1a) ==");
    println!("  rolls {}   sevens {}", one.rolls, one.sevens);
    println!("  deviation           {:.4} bits", one.kl_bits);
    println!(
        "  presented as        \"deviated more than {:.0}% of recorded games\"",
        reference.deviation_percentile(one.kl_bits) * 100.0
    );
    println!(
        "  (p = {:.3}, deliberately not shown to players — across {} games ~5% clear 0.05)",
        one.p_value, games
    );

    // ---- §10.2 production ----
    let prod = production::analyse(sample).expect("production");
    println!("\n== game 0: expected vs actual production (§10.2) ==");
    println!("  seat   E_raw  robber  supply    luck  actual      z");
    for p in 0..4 {
        let d = prod.decompose(p);
        println!(
            "  {p}     {:6.1}  {:6.1}  {:6.1}  {:+6.1}  {:6.0}  {:+5.2}",
            d.e_raw, d.robber_cost, d.supply_denial, d.dice_luck, d.actual, d.luck_z
        );
    }
    println!("  identity residual   {:.2e}", prod.decompose(0).residual());

    let worst = (0..5)
        .map(|r| (r, prod.decompose_resource(0, Some(r))))
        .min_by(|a, b| a.1.luck_z.total_cmp(&b.1.luck_z))
        .unwrap();
    println!(
        "  seat 0 was most starved of {} (z = {:+.2})",
        RESOURCES[worst.0], worst.1.luck_z
    );

    // ---- §10.3 one game ----
    let summary = game::analyse(sample).expect("summary");
    println!("\n== game 0: summary (§10.3) ==");
    println!(
        "  winner {:?}   points {:?}   turns {}",
        summary.winner, summary.vp, summary.turns
    );
    println!(
        "  sevens {}   robber moves {}   robberies {} ({} found an empty hand)",
        summary.sevens,
        summary.robber_moves,
        (0..4).map(|p| summary.stole(p)).sum::<u32>(),
        summary.empty_robberies
    );
    println!(
        "  tile transfers      longest road {}   largest militia {}",
        summary.longest_road_transfers, summary.largest_militia_transfers
    );
    for p in 0..4 {
        let o = summary.opening[p];
        println!(
            "  seat {p}  opening {:2} pips, {} resources, {} ports  |  built {}r {}s {}c  |  \
             offers {:2}, trades {:2}, maritime {:2}  |  robbed of {}",
            o.pips,
            o.diversity,
            o.ports,
            summary.builds[p].roads,
            summary.builds[p].settlements,
            summary.builds[p].cities,
            summary.offers_made[p],
            summary.trades_completed[p],
            summary.maritime_trades[p],
            summary.robbed_of(p),
        );
    }

    // ---- §10.4 luck adjustment ----
    println!("\n== who converts production into points? (§10.4) ==");
    if let Some(fit) = corpus.luck_adjustment() {
        println!(
            "  VP = {:.2} + {:.4} x cards produced     (r² = {:.3}, n = {})",
            fit.fit.intercept, fit.fit.slope, fit.fit.r_squared, fit.fit.n
        );
        println!("  every ~{:.0} cards buys a point", 1.0 / fit.fit.slope);
    }
    for (player, residual, n) in corpus.conversion_residuals() {
        println!(
            "  {:<13} {:+.3} VP above the curve over {n} games",
            AGENTS[player as usize], residual
        );
    }

    // ---- §10.5 ratings ----
    println!("\n== ratings (§10.5, Plackett–Luce) ==");
    println!("  (same policy in every seat, seats rotated — a spread here is tie-break");
    println!("   luck plus noise, so read it against sigma rather than as a ranking)");
    let pool = corpus
        .ratings
        .pool(PoolKey {
            trade_mode: mode,
            rules_version: RULES_VERSION,
        })
        .expect("a pool");
    let board = pool.leaderboard(0);
    for (player, r, n) in &board {
        println!(
            "  {:<13} mu {:5.2}   sigma {:4.2}   shown {:5.2}   ({n} games)",
            AGENTS[*player as usize],
            r.mu,
            r.sigma,
            r.conservative()
        );
    }
    let spread = board[0].1.mu - board[board.len() - 1].1.mu;
    println!(
        "  spread {:.2} mu over {:.2} sigma -> {:.1} sigma apart: {}",
        spread,
        board[0].1.sigma,
        spread / board[0].1.sigma,
        if spread / board[0].1.sigma < 2.0 {
            "not separated"
        } else {
            "separated"
        }
    );

    println!("\n== cost ==");
    println!(
        "  play + record       {:.0} µs/game",
        play_time.as_secs_f64() * 1e6 / games as f64
    );
    println!(
        "  full analysis       {:.0} µs/game   ({:.1}x cheaper than playing)",
        analyse_time.as_secs_f64() * 1e6 / games as f64,
        play_time.as_secs_f64() / analyse_time.as_secs_f64()
    );
}
