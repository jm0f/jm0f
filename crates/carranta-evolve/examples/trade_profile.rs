//! Where does a champion's head-to-head strength go when it plays the
//! heuristic field?
//!
//! Every champion of the recent lineage beats the shipped one in direct play
//! and trails it against three heuristics. One explanation is the market: a
//! network bred in a field of networks may have learned a trade-heavy style
//! that only pays when the other seats trade back, and the heuristics answer
//! offers by a different rule. If that is the cause it leaves a signature
//! rather than a story, and the signature is per seat: the champion's own
//! offers, how many of them the table takes, and how often it falls back on
//! the bank at four to one when nobody does.
//!
//! So this plays one champion in both seatings, head to head against another
//! network and alone against three heuristics, and reports what its own seat
//! did in each. The comparison is the point: a trade-dependent champion
//! should ask about as often in both and be answered far less in one.

use carranta_analytics::game;
use carranta_bot::net::Net;
use carranta_core::state::{OfferShapes, TradeMode};
use carranta_evolve::arena::{Arena, Brain, NetJob};

/// What one seat did with the market, per game.
#[derive(Default, Clone, Copy)]
struct Market {
    games: f64,
    offers: f64,
    taken: f64,
    declined_by_others: f64,
    supply: f64,
    trades_by_table: f64,
}

impl Market {
    fn per_game(&self) -> (f64, f64, f64, f64, f64) {
        let n = self.games.max(1.0);
        (
            self.offers / n,
            self.taken / n,
            self.declined_by_others / n,
            self.supply / n,
            self.trades_by_table / n,
        )
    }
}

fn load(path: &str) -> Net {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("cannot read {path}: {e}");
        std::process::exit(1);
    });
    match Net::parse(&text) {
        Some((net, _)) => net,
        None => {
            eprintln!("{path} is not a champion network file");
            std::process::exit(1);
        }
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let (mut champion, mut against) = (String::new(), String::new());
    let mut rounds = 60u32;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--champion" => champion = args.next().unwrap_or_default(),
            "--against" => against = args.next().unwrap_or_default(),
            "--rounds" => rounds = args.next().and_then(|v| v.parse().ok()).unwrap_or(60),
            other => {
                eprintln!("unknown flag {other}");
                std::process::exit(2);
            }
        }
    }
    if champion.is_empty() || against.is_empty() {
        eprintln!("--champion FILE and --against FILE are both required");
        std::process::exit(2);
    }

    let arena = Arena {
        mode: TradeMode::Full,
        asks: 3,
        shapes: OfferShapes::Mixed {
            give: Some(2),
            want: 2,
        },
        ..Arena::default()
    };
    let me = load(&champion);
    let them = load(&against);

    // The roster order versus.rs uses: index 0 is the opponent, index 1 the
    // champion, and a seating names roster indices. So a 1 marks a champion
    // seat, and those are the rows to read, all of them.
    let head: Vec<[u32; 4]> = vec![[1, 1, 0, 0], [1, 0, 1, 0], [1, 0, 0, 1]];
    let alone: Vec<[u32; 4]> = vec![[1, 0, 0, 0]];

    for (label, roster, seatings) in [
        (
            "head to head",
            vec![
                Brain::DeepPlanned(them.clone()),
                Brain::DeepPlanned(me.clone()),
            ],
            head,
        ),
        (
            "solo, three heuristics",
            vec![Brain::Anchor, Brain::DeepPlanned(me.clone())],
            alone,
        ),
    ] {
        let mut m = Market::default();
        for r in 0..rounds {
            for seats in &seatings {
                let job = NetJob {
                    seed: 88u64.wrapping_add(r as u64),
                    seats: *seats,
                };
                let (_, log) = arena.play_net_recorded(&roster, &job);
                let Ok(rep) = game::analyse(&log) else {
                    continue;
                };
                // Averaged over the champion's seats, so a head to head with
                // two of them and a solo with one are the same quantity: what
                // one champion seat did in one game.
                let mine: Vec<usize> = (0..rep.players as usize)
                    .filter(|&p| seats[p] == 1)
                    .collect();
                if mine.is_empty() {
                    continue;
                }
                let n = mine.len() as f64;
                m.games += 1.0;
                m.offers += mine.iter().map(|&p| rep.offers_made[p] as f64).sum::<f64>() / n;
                m.taken += mine
                    .iter()
                    .map(|&p| rep.trades_completed[p] as f64)
                    .sum::<f64>()
                    / n;
                m.supply += mine
                    .iter()
                    .map(|&p| rep.supply_trades[p] as f64)
                    .sum::<f64>()
                    / n;
                let others: f64 = (0..rep.players as usize)
                    .filter(|p| seats[*p] != 1)
                    .map(|p| rep.offers_declined[p] as f64)
                    .sum();
                m.declined_by_others += others;
                let table: u32 = (0..rep.players as usize)
                    .map(|p| rep.trades_completed[p])
                    .sum();
                m.trades_by_table += table as f64 / 2.0;
            }
        }
        let (offers, taken, declined, supply, table) = m.per_game();
        println!("{label}  ({:.0} games)", m.games);
        println!("  offers the champion made      {offers:.2}");
        println!("  trades the champion was in    {taken:.2}");
        println!("  declines the others issued    {declined:.2}");
        println!("  bank trades the champion took {supply:.2}");
        println!("  trades at the whole table     {table:.2}");
        if offers > 0.0 {
            println!("  answered per offer            {:.3}", taken / offers);
        }
        println!();
    }
}
