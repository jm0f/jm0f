//! Turning a saved game into the log the analytics read, and reading it.
//!
//! `carranta-analytics` was built against `carranta_record::Log` and has been
//! measured against it since; this is the join between that and what a browser
//! actually played. A saved game is a seed and a list of steps, and a `Log` is
//! those steps with what each one resolved recorded beside it, so the bridge is
//! a replay: hand the moves back to a `Recorder` and let it write down what
//! happened, which is what it is for.

use carranta_analytics::corpus;
use carranta_analytics::dice;
use carranta_analytics::game::{self, Report};
use carranta_analytics::production;
use carranta_analytics::rating::{Model, Pool, Rating};
use carranta_core::state::MAX_PLAYERS;
use carranta_record::{Log, Recorder, SeatId};

use crate::game::Step;
use crate::store::Saved;

/// The names the bots answer to, which are also who the ratings are about.
///
/// One identity per seat rather than one for "the heuristic". They are the same
/// player underneath, so their ratings should converge on each other, but a
/// ranking cannot list the same player three times and a per-seat identity is
/// also what makes the corpus's seat-order balance mean anything.
pub const BOT_NAMES: [&str; 4] = ["Ada", "Bram", "Ines", "Odd"];

/// Who is in a seat, for the rating pool.
///
/// The person is one player across every game on this server; each bot seat is
/// another. Small fixed numbers rather than hashes: this is one local server
/// with five participants, and a number you can read in a file is worth more
/// here than one you cannot.
pub fn seat_player(seat: usize) -> u64 {
    seat as u64
}

pub fn seat_name(seat: usize, human: &str) -> String {
    if seat == 0 {
        let name = human.trim();
        if name.is_empty() { "you".to_string() } else { name.to_string() }
    } else {
        BOT_NAMES[seat.min(BOT_NAMES.len() - 1)].to_string()
    }
}

/// Replay a saved game into the record the analytics were built to read.
///
/// `None` when the moves and this build disagree about the rules, which is the
/// same answer `Session::resume` gives and for the same reason.
pub fn to_log(saved: &Saved) -> Option<Log> {
    let state = crate::game::Session::opening(saved.seats, saved.seed, saved.mode);
    let seats = (0..state.players as usize)
        .map(|s| {
            if s == 0 {
                SeatId::human(seat_player(s))
            } else {
                SeatId::agent(seat_player(s), BOT_NAMES[s], 1)
            }
        })
        .collect();
    let mut rec = Recorder::new(game_number(&saved.id), saved.seed, state, seats);
    for step in &saved.moves {
        match *step {
            Step::Move(action) => {
                rec.apply(action).ok()?;
            }
            // A refusal is an event in the record too, and the reason the
            // analytics can count offers declined at all.
            Step::Passed { offer, by } => rec.decline(offer, by),
        }
    }
    Some(rec.finish_into(saved.winner))
}

/// A number for a game, from its address.
///
/// The record wants a `u64` and the server deals in three groups of letters.
/// Folded rather than parsed, since nothing reads it back: it exists so two
/// games in one corpus are two games.
fn game_number(id: &str) -> u64 {
    id.bytes()
        .fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
            (h ^ b as u64).wrapping_mul(0x0100_0000_01b3)
        })
}

/// What a seat's rating did across one game.
#[derive(Clone, Copy, Debug)]
pub struct Movement {
    pub before: Rating,
    pub after: Rating,
    /// Games this player had behind them going in, which is how much the
    /// number is worth believing.
    pub games: u32,
}

impl Movement {
    /// The change in the conservative estimate, which is the number shown.
    pub fn delta(&self) -> f64 {
        self.after.conservative() - self.before.conservative()
    }
}

/// Everything the analytics page says about one game.
pub struct Study {
    pub report: Report,
    pub production: production::Report,
    pub dice: dice::GameDice,
    /// Where this game's dice sit against every other game recorded here, as a
    /// percentage from 0 to 100, or `None` until there are others to sit
    /// against. A percentile of one game is not a percentile.
    pub dice_percentile: Option<f64>,
    pub corpus_games: usize,
    /// What this result did to each seat's rating.
    pub movement: [Option<Movement>; MAX_PLAYERS],
    /// Seat win rates across the corpus, once there is a corpus.
    pub seat_wins: Option<[f64; MAX_PLAYERS]>,
}

/// Monte Carlo draws behind the dice figure.
///
/// The scoping doc is firm that a chi-squared test is invalid at one game's
/// sample size (§10.1) and that the answer is simulation. Ten thousand is
/// milliseconds and steadies the last digit.
const SIMS: u32 = 10_000;

/// Study one game against every game recorded beside it.
///
/// `history` is every saved game, this one included, oldest first. The ratings
/// are built by replaying all of them in order and reading the pool either side
/// of this one, which is the only honest way to say what a result did: a rating
/// is a function of everything before it.
pub fn study(saved: &Saved, history: &[Saved]) -> Option<Study> {
    let log = to_log(saved)?;
    let report = game::analyse(&log).ok()?;
    let production = production::analyse(&log).ok()?;
    let rolls = dice::rolls(&log);
    let this = dice::analyse_game(&rolls, SIMS, saved.seed);

    // Every other finished game, for the percentile and the seat balance.
    let mut games = corpus::Corpus::new(corpus::Config::of(&log));
    let mut others = 0usize;
    for g in history {
        if g.id == saved.id {
            continue;
        }
        if let Some(l) = to_log(g)
            && games.add(&l, SIMS)
        {
            others += 1;
        }
    }
    let deviations = games.dice_deviations.clone();
    // Out of a hundred, not out of one. `deviation_percentile` answers with a
    // share, which read as "these dice deviated more than 1% of games" on a
    // page where it meant all of them.
    let dice_percentile = (!deviations.is_empty()).then(|| {
        dice::Corpus::from_games(deviations).deviation_percentile(this.kl_bits) * 100.0
    });
    let seat_wins = (others > 0).then(|| games.seat_win_rate());

    // Ratings, in the order the games were played, stopping either side of this
    // one so the movement is this game's and nobody else's.
    let mut pool = Pool::new(Model::default());
    let mut movement = [None; MAX_PLAYERS];
    for g in history {
        if g.id == saved.id {
            let seats = report.players as usize;
            let before: Vec<Rating> = (0..seats).map(|s| pool.rating(seat_player(s))).collect();
            let games: Vec<u32> = (0..seats)
                .map(|s| pool.games_played(seat_player(s)))
                .collect();
            if !pool.record(&log) {
                break;
            }
            for s in 0..seats {
                movement[s] = Some(Movement {
                    before: before[s],
                    after: pool.rating(seat_player(s)),
                    games: games[s],
                });
            }
            continue;
        }
        if let Some(l) = to_log(g) {
            pool.record(&l);
        }
    }

    Some(Study {
        report,
        production,
        dice: this,
        dice_percentile,
        corpus_games: others,
        movement,
        seat_wins,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Session;
    use crate::store::game_id;
    use carranta_core::state::TradeMode;

    /// Play one out and save it, the way the server would.
    fn played(seed: u64) -> Saved {
        let mut s = Session::new(4, seed, TradeMode::Full);
        for _ in 0..500 {
            let v = s.version();
            if s.choices().is_empty() || s.act(0, v).is_err() {
                break;
            }
        }
        let (seats, dealt, mode) = s.table();
        Saved {
            id: game_id(seed),
            seats,
            seed: dealt,
            mode,
            name: "Egon".to_string(),
            dealt: seed,
            winner: s.winner(),
            moves: s.moves().to_vec(),
        }
    }

    #[test]
    fn a_played_game_becomes_a_record_the_analytics_can_read() {
        for seed in 0..6u64 {
            let g = played(seed);
            let log = to_log(&g).expect("the moves replay into a record");
            // The record's own check: replaying the events has to land on every
            // snapshot it took along the way.
            log.verify().expect("the record is consistent with itself");
            let r = game::analyse(&log).expect("and it analyses");
            assert_eq!(r.players, 4);
            assert_eq!(r.winner, g.winner);
            // A game that was played has rolls in it, and the rolls are the
            // dice: 11 outcomes, summing to how many times anybody rolled.
            let total: u32 = r.rolls.iter().sum();
            assert!(total > 0, "seed {seed} rolled nothing");
            assert_eq!(total, dice::rolls(&log).len() as u32);
        }
    }

    #[test]
    fn a_refusal_is_counted_as_one() {
        // The reason refusals are recorded at all: without them the record
        // says nobody ever turned an offer down.
        let mut refused = 0;
        for seed in 0..8u64 {
            let g = played(seed);
            let declines = g
                .moves
                .iter()
                .filter(|s| matches!(s, Step::Passed { .. }))
                .count() as u32;
            let log = to_log(&g).expect("it replays");
            let r = game::analyse(&log).expect("and analyses");
            let counted: u32 = r.offers_declined.iter().sum();
            assert_eq!(counted, declines, "seed {seed}");
            refused += declines;
        }
        assert!(refused > 0, "some game in the range had an offer turned down");
    }

    #[test]
    fn a_rating_moves_by_what_the_game_did_and_by_nothing_else() {
        let history: Vec<Saved> = (0..4u64).map(played).collect();
        let study = study(&history[2], &history).expect("it studies");
        // Every seat that played has a movement, and it is the difference
        // between the pool before this game and after it.
        for seat in 0..4 {
            let m = study.movement[seat].expect("every seat is rated");
            assert!((m.after.conservative() - m.before.conservative() - m.delta()).abs() < 1e-9);
        }
        // The winner gains and somebody loses: a rating update is a
        // redistribution, so they cannot all move the same way.
        let deltas: Vec<f64> = (0..4).map(|s| study.movement[s].unwrap().delta()).collect();
        assert!(deltas.iter().any(|d| *d > 0.0), "somebody gained");
        assert!(deltas.iter().any(|d| *d < 0.0), "somebody lost");
        if let Some(w) = study.report.winner {
            let best = deltas
                .iter()
                .cloned()
                .fold(f64::NEG_INFINITY, f64::max);
            assert!(
                (deltas[w as usize] - best).abs() < 1e-9,
                "the winner gains the most"
            );
        }
        // The games behind each player going in is what says how much to
        // believe the number, and it counts the games before this one.
        assert_eq!(study.movement[0].unwrap().games, 2);
        assert_eq!(study.corpus_games, 3);
    }

    #[test]
    fn the_order_games_were_played_in_is_the_order_they_are_rated_in() {
        // A rating is a function of every game before it, so the history's
        // order is load-bearing rather than cosmetic. Read at the end of the
        // list, a player has every earlier game behind them.
        let history: Vec<Saved> = (30..34u64).map(played).collect();
        let first = study(&history[0], &history).expect("it studies");
        let last = study(&history[3], &history).expect("it studies");
        assert_eq!(first.movement[0].unwrap().games, 0, "nothing before the first");
        assert_eq!(last.movement[0].unwrap().games, 3, "three before the last");
        // And the belief narrows as the games accumulate, which is the whole
        // reason the count is printed beside the figure.
        assert!(
            last.movement[0].unwrap().before.sigma < first.movement[0].unwrap().before.sigma,
            "a rating gets surer as games go by"
        );
    }

    #[test]
    fn the_percentile_is_out_of_a_hundred() {
        // `deviation_percentile` answers with a share of one, and the page says
        // "%". The wildest game in a set has to come out at 100, not at 1.
        let history: Vec<Saved> = (20..26u64).map(played).collect();
        let mut seen: Vec<(String, f64)> = history
            .iter()
            .map(|g| {
                let s = study(g, &history).expect("it studies");
                (g.id.clone(), s.dice_percentile.expect("there are others"))
            })
            .collect();
        for (id, p) in &seen {
            assert!((0.0..=100.0).contains(p), "{id} is at {p}");
        }
        seen.sort_by(|a, b| a.1.total_cmp(&b.1));
        // Six games, so the shares are sixths: the calmest is at 0 and the
        // wildest at 100, which is exactly what a share of one would hide.
        assert_eq!(seen.first().unwrap().1, 0.0);
        assert_eq!(seen.last().unwrap().1, 100.0);
    }

    #[test]
    fn one_game_has_no_percentile_to_be_at() {
        // §10.1: a per-game deviation is presented against recorded games, and
        // with no recorded games there is nothing to present it against.
        let only = played(11);
        let alone = study(&only, std::slice::from_ref(&only)).expect("it studies");
        assert_eq!(alone.dice_percentile, None);
        assert_eq!(alone.corpus_games, 0);
        assert_eq!(alone.seat_wins, None);

        let history: Vec<Saved> = (11..14u64).map(played).collect();
        let among = study(&history[0], &history).expect("it studies");
        let p = among.dice_percentile.expect("with others there is one");
        assert!((0.0..=100.0).contains(&p), "a percentile is a percentile: {p}");
    }
}
