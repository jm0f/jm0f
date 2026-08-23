//! Is a champion actually better than the heuristic, and by how much?
//!
//! ```text
//! cargo run --release -p carranta-evolve --example versus -- --champion champion.net
//! cargo run --release -p carranta-evolve --example versus -- --champion c.net --rounds 2000
//! cargo run --release -p carranta-evolve --example versus -- --champion c.net --against bots/trained-378.net
//! ```
//!
//! `--solo` seats one champion against three of the opponent instead of two
//! against two, rotated through all four chairs of every seed. The paired gap
//! loses its meaning there, so the headline becomes the champion's win share
//! against the 25% a fair four-seat game hands any one chair, plus its mean
//! finishing position against 2.5. This is the domination question, how far
//! under 100% the chance in the game holds a stronger player, and it is also
//! the number a person sitting alone against three bots actually faces.
//!
//! `--against FILE` swaps the heuristic out for another champion file, which
//! is the shipping question asked directly: a candidate does not have to beat
//! the heuristic to earn a chair, it has to beat the champion holding one.
//! Measured against the incumbent, a negative gap with its interval clear of
//! zero *is* an improvement over what is deployed, with nothing inferred
//! through a common yardstick. The heuristic stays the default because it is
//! the pinned ruler every run and every generation is comparable on; this
//! flag is for the head-to-head that ruler cannot give.
//!
//! The training loop already answers a version of this every generation, on
//! held-out games and through the rating (E-10, E-11). This answers it the
//! plain way instead, for a champion file that may have come from anywhere: it
//! plays the thing against the pinned heuristic and reports the gap with a
//! confidence interval, so "better" is a measurement rather than an impression.
//!
//! Three things make the number trustworthy, and all three are the reason this
//! is not just a loop over games:
//!
//! - **Common random numbers.** Every board seed is played in all six ways two
//!   champions can sit among four seats. The champion and the heuristic meet on
//!   identical boards with identical dice, so the difference between them is
//!   not a difference between the boards they happened to get.
//! - **Seat rotation.** Seats are not equal: the first to place has a real
//!   advantage. Playing all six arrangements of a seed cancels it exactly
//!   rather than hoping it averages out.
//! - **A seed is one observation, not six.** The six arrangements of one board
//!   are strongly correlated, so counting them as six independent games would
//!   shrink the confidence interval by about the square root of six and claim
//!   far more certainty than the experiment has. Each seed contributes one
//!   number: the average gap across its six arrangements.
//!
//! The statistic is mean finishing position (E-6), one to four, lower better.
//! Under the null hypothesis that the two play equally well, a seat's expected
//! position is 2.5 and the gap is zero.

use carranta_bot::net::Net;
use carranta_core::state::{OfferShapes, TradeMode};
use carranta_evolve::arena::{Arena, Brain, NetJob};

/// The six ways two champions can sit among four seats.
const ARRANGEMENTS: [[u32; 4]; 6] = [
    [1, 1, 0, 0],
    [1, 0, 1, 0],
    [1, 0, 0, 1],
    [0, 1, 1, 0],
    [0, 1, 0, 1],
    [0, 0, 1, 1],
];

/// The four ways one champion can sit among four seats.
const SOLO: [[u32; 4]; 4] = [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]];

fn main() {
    let mut champion = String::new();
    let mut against = String::new();
    let mut solo = false;
    let mut deep_champion = false;
    let mut deep_against = false;
    let mut rounds = 500usize;
    let mut threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let mut seed = 90_210u64;
    let mut give_cap = Some(2u8);
    let mut want_cap = 2u8;
    let mut ask_cap = 3u8;
    let mut mode = TradeMode::Full;

    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let mut value = || args.next().unwrap_or_default();
        match flag.as_str() {
            "--champion" => champion = value(),
            "--against" => against = value(),
            "--solo" => solo = true,
            // The lookahead question (E-27): wrap that side's network in the
            // beamed two-ply search, so search-vs-greedy is measured with the
            // same weights on both sides.
            "--deep" => deep_champion = true,
            "--deep-against" => deep_against = true,
            "--rounds" => rounds = value().parse().unwrap_or(rounds),
            "--threads" => threads = value().parse().unwrap_or(threads),
            "--seed" => seed = value().parse().unwrap_or(seed),
            "--want-cap" => want_cap = value().parse().unwrap_or(want_cap),
            "--ask-cap" => ask_cap = value().parse().unwrap_or(ask_cap),
            "--give-cap" => {
                let v = value();
                give_cap = if v == "hand" { None } else { v.parse().ok() };
            }
            "--mode" => {
                mode = match value().as_str() {
                    "disabled" => TradeMode::Disabled,
                    "restricted" => TradeMode::Restricted,
                    _ => TradeMode::Full,
                }
            }
            other => {
                eprintln!("unknown option `{other}`");
                std::process::exit(2);
            }
        }
    }
    if champion.is_empty() {
        eprintln!("--champion FILE is required");
        std::process::exit(2);
    }
    let text = std::fs::read_to_string(&champion).unwrap_or_else(|e| {
        eprintln!("cannot read {champion}: {e}");
        std::process::exit(1);
    });
    let Some((net, generation)) = Net::parse(&text) else {
        eprintln!("{champion} is not a champion network file");
        std::process::exit(1);
    };
    // The other side of the table: the pinned heuristic unless --against
    // names a champion file, in which case the incumbent it names.
    let opponent = if against.is_empty() {
        None
    } else {
        let text = std::fs::read_to_string(&against).unwrap_or_else(|e| {
            eprintln!("cannot read {against}: {e}");
            std::process::exit(1);
        });
        let Some(parsed) = Net::parse(&text) else {
            eprintln!("{against} is not a champion network file");
            std::process::exit(1);
        };
        Some(parsed)
    };

    // The market the champion was trained in, which is also the one a table
    // seating it enumerates. Measuring it anywhere else would be measuring a
    // different player: a trading policy judged in a market it cannot trade in
    // is being asked the wrong question.
    // A champion from before the allowance existed trained uncapped; measure
    // it as it was trained with --ask-cap 20.
    let arena = Arena {
        mode,
        asks: ask_cap,
        shapes: match give_cap {
            _ if mode != TradeMode::Full => OfferShapes::SingleType,
            give => OfferShapes::Mixed {
                give,
                want: want_cap,
            },
        },
        ..Arena::default()
    };
    let deepen = |b: Brain, deep: bool| match (b, deep) {
        (Brain::Net(n), true) => Brain::Deep(n),
        (b, _) => b,
    };
    let (them, them_name) = match &opponent {
        Some((net, generation)) => (
            deepen(Brain::Net(net.clone()), deep_against),
            format!(
                "trained@{generation}{}",
                if deep_against { " (deep)" } else { "" }
            ),
        ),
        None => (Brain::Anchor, "the pinned heuristic".to_string()),
    };
    let roster = [them, deepen(Brain::Net(net), deep_champion)];

    let seatings: &[[u32; 4]] = if solo { &SOLO } else { &ARRANGEMENTS };
    let jobs: Vec<NetJob> = (0..rounds)
        .flat_map(|r| {
            seatings.iter().map(move |seats| NetJob {
                seed: seed.wrapping_add(r as u64),
                seats: *seats,
            })
        })
        .collect();

    println!(
        "trained@{generation}{}{} against {}{them_name}",
        if deep_champion { " (deep)" } else { "" },
        if solo { " alone" } else { "" },
        if solo { "three of " } else { "" },
    );
    println!(
        "  {} seeds x {} seatings = {} games, {mode:?} market, {threads} workers",
        rounds,
        seatings.len(),
        jobs.len()
    );
    let began = std::time::Instant::now();
    let outcomes = arena.play_net_all(&roster, &jobs, threads);
    let secs = began.elapsed().as_secs_f64();

    // One number per seed: the champion's mean position less the heuristic's,
    // averaged over the six seatings of that board. Negative means the
    // champion finished ahead.
    let mut gaps = Vec::with_capacity(rounds);
    let mut shares = Vec::with_capacity(rounds);
    let mut champ_positions = 0.0f64;
    let mut house_positions = 0.0f64;
    let mut champion_wins = 0usize;
    let mut decided = 0usize;
    let champions_a_game: f64 = if solo { 1.0 } else { 2.0 };
    for (r, chunk) in outcomes.chunks(seatings.len()).enumerate() {
        let mut gap = 0.0;
        let (mut seed_wins, mut seed_decided) = (0.0, 0.0);
        for (o, seats) in chunk.iter().zip(seatings.iter()) {
            let (mut mine, mut theirs) = (0.0, 0.0);
            for (seat, &who) in seats.iter().enumerate() {
                let p = o.position[seat] as f64;
                if who == 1 {
                    mine += p / champions_a_game;
                } else {
                    theirs += p / (4.0 - champions_a_game);
                }
            }
            // `mine` and `theirs` are already each side's mean position in
            // this game, since the two seats were halved as they were added.
            gap += (mine - theirs) / seatings.len() as f64;
            champ_positions += mine;
            house_positions += theirs;
            if let Some(w) = o.winner {
                decided += 1;
                seed_decided += 1.0;
                if seats[w as usize] == 1 {
                    champion_wins += 1;
                    seed_wins += 1.0;
                }
            }
        }
        let _ = r;
        gaps.push(gap);
        // The same board's win share, as its own observation (E-17). Position
        // and wins can disagree, and the generation 72 champion is why this
        // is printed with an interval rather than as a bare percentage: it
        // finished ahead on position while winning less often, which one
        // number alone would have hidden either way. A board nobody won says
        // nothing, so it counts as even.
        let even = champions_a_game / 4.0;
        shares.push(if seed_decided > 0.0 {
            seed_wins / seed_decided
        } else {
            even
        });
    }

    // Both columns are means over the same seeds, so both intervals are taken
    // the same way.
    let interval = |xs: &[f64]| {
        let n = xs.len() as f64;
        let mean = xs.iter().sum::<f64>() / n;
        let var = xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0);
        let se = (var / n).sqrt();
        (mean, se)
    };
    let (mean, se) = interval(&gaps);
    let t = mean / se;
    let p = two_sided_p(t.abs());
    let half = 1.96 * se;
    let (win_mean, win_se) = interval(&shares);
    let win_null = champions_a_game / 4.0;
    let win_t = (win_mean - win_null) / win_se;
    let win_p = two_sided_p(win_t.abs());
    let win_half = 1.96 * win_se;

    println!(
        "\n  {} games in {secs:.1}s ({:.0} games/s)",
        outcomes.len(),
        outcomes.len() as f64 / secs
    );
    let even = 100.0 * champions_a_game / 4.0;
    println!(
        "  mean finishing position   champion {:.4}   {them_short} {:.4}   (2.5 = even)",
        champ_positions / outcomes.len() as f64,
        house_positions / outcomes.len() as f64,
        them_short = if opponent.is_some() {
            "incumbent"
        } else {
            "heuristic"
        },
    );
    println!(
        "  wins                      champion {champion_wins} of {decided} decided ({:.1}%, {even:.0}% = even)",
        100.0 * champion_wins as f64 / decided.max(1) as f64
    );
    println!(
        "\n  gap {mean:+.4} positions  95% CI [{:+.4}, {:+.4}]",
        mean - half,
        mean + half
    );
    println!("  t = {t:.2} over {} seeds, p {}", gaps.len(), show_p(p));
    // The whole point of the interval: an effect whose interval spans zero has
    // not been shown to exist, however suggestive its midpoint looks.
    let verdict = if mean + half < 0.0 {
        "the champion is better on position, and the interval clears zero"
    } else if mean - half > 0.0 {
        if opponent.is_some() {
            "the incumbent is better on position, and the interval clears zero"
        } else {
            "the heuristic is better on position, and the interval clears zero"
        }
    } else {
        "no gap shown: the interval spans zero, so this is consistent with equal play"
    };
    println!("  {verdict}");

    // Wins, on the same footing. Two of one side sit against two of the
    // other in every seating, so a half is even here and 50% is the null.
    println!(
        "\n  wins {:.1}%  95% CI [{:.1}%, {:.1}%]",
        100.0 * win_mean,
        100.0 * (win_mean - win_half),
        100.0 * (win_mean + win_half)
    );
    println!(
        "  t = {win_t:.2} over {} seeds, p {}",
        shares.len(),
        show_p(win_p)
    );
    let win_verdict = if win_mean - win_half > win_null {
        "the champion wins more often than an even chair, and the interval clears even"
    } else if win_mean + win_half < win_null {
        "the champion wins less often than an even chair, and the interval clears even"
    } else {
        "no difference in wins shown: the interval spans even"
    };
    println!("  {win_verdict}");
}

/// Two-sided p for a t statistic, read off the normal distribution.
///
/// The normal rather than Student's t because the sample is hundreds of seeds
/// and the difference between the two is far below the precision anybody
/// should read into a p value. Hastings' approximation to the error function,
/// accurate to about 1e-7, which is more than the number deserves.
fn two_sided_p(z: f64) -> f64 {
    let t = 1.0 / (1.0 + 0.231_641_9 * z / std::f64::consts::SQRT_2);
    let poly = t
        * (0.319_381_53
            + t * (-0.356_563_782
                + t * (1.781_477_937 + t * (-1.821_255_978 + t * 1.330_274_429))));
    let tail = poly * (-z * z / 2.0).exp() / (2.0 * std::f64::consts::PI).sqrt();
    (2.0 * tail).clamp(0.0, 1.0)
}

/// A p value said the way it should be read: below a threshold, not as a
/// spuriously precise decimal.
fn show_p(p: f64) -> String {
    for cut in [1e-6, 1e-4, 1e-3, 0.01, 0.05] {
        if p < cut {
            return format!("< {cut}");
        }
    }
    format!("= {p:.3}")
}
