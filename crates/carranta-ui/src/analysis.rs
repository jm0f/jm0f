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
        if name.is_empty() {
            "you".to_string()
        } else {
            name.to_string()
        }
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
    id.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
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
    /// Where they stood in the whole pool either side of this game, best
    /// first, or `None` before they had a rating to stand anywhere with.
    ///
    /// The pool, not the table: a rating is a claim about every player on this
    /// server, and a place at one table of four says nothing about it.
    pub rank_before: Option<usize>,
    pub rank_after: Option<usize>,
}

/// Where a player stands in the pool, best first, counting from one.
///
/// `None` until they have played, since an unrated player is not last, they
/// are absent. Ranked on the conservative estimate, which is the figure the
/// page shows, so a rank and the number beside it cannot disagree.
fn rank_of(pool: &Pool, seat: usize) -> Option<usize> {
    pool.leaderboard(0)
        .iter()
        .position(|(p, _, _)| *p == seat_player(seat))
        .map(|i| i + 1)
}

impl Movement {
    /// The change in the conservative estimate, which is the number shown.
    pub fn delta(&self) -> f64 {
        self.after.conservative() - self.before.conservative()
    }
}

/// Where a seat's points came from (R-11.3).
///
/// The five things that score and nothing else: settlements one each, cities
/// two, the two tiles two apiece, and a victory point card one. Roads score
/// nothing and neither does a militia played, however many of either there are.
///
/// Read off the final position rather than counted from what was built, which
/// is a different number: a settlement upgraded to a city stopped being a
/// settlement, and was still built.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Points {
    pub settlements: Scored,
    pub cities: Scored,
    pub cards: Scored,
    pub longest_road: Scored,
    pub largest_militia: Scored,
}

/// One of the five, as how many and as what they were worth.
///
/// Both, because they are different numbers and the page wants each: three
/// cities is three things and six points, and printing either alone loses the
/// other.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Scored {
    /// How many stood at the end. One or nought for the two tiles, which are
    /// held rather than counted.
    pub held: u32,
    /// What those were worth.
    pub points: u32,
}

impl Scored {
    /// `held` of a thing worth `each`.
    fn each(held: u32, each: u32) -> Self {
        Scored {
            held,
            points: held * each,
        }
    }
}

impl Points {
    /// The five, in the order the result table reads them.
    pub fn parts(&self) -> [Scored; 5] {
        [
            self.settlements,
            self.cities,
            self.cards,
            self.longest_road,
            self.largest_militia,
        ]
    }

    /// What they add to, which has to be the seat's true total.
    pub fn total(&self) -> u32 {
        self.parts().iter().map(|s| s.points).sum()
    }
}

/// Break the final position into what scored, seat by seat.
fn points_of(state: &carranta_core::state::State, seats: usize) -> [Points; MAX_PLAYERS] {
    use carranta_core::state::DevCard;
    let mut out = [Points::default(); MAX_PLAYERS];
    for (p, slot) in out.iter_mut().enumerate().take(seats) {
        *slot = Points {
            settlements: Scored::each(state.settlements[p].count_ones(), 1),
            cities: Scored::each(state.cities[p].count_ones(), 2),
            cards: Scored::each(state.dev_held[p][DevCard::VictoryPoint as usize] as u32, 1),
            longest_road: Scored::each((state.longest_road == Some(p as u8)) as u32, 2),
            largest_militia: Scored::each((state.largest_militia == Some(p as u8)) as u32, 2),
        };
    }
    out
}

/// Every card that reached a hand or left it, by what moved it.
///
/// The categories are the whole of the game's economy: nothing gains or loses a
/// card except through one of these, which is what makes the ledger balance.
/// What came in less what went out is what is still in the hand at the end, and
/// `Ledger::balances` checks exactly that.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Ledger {
    // ---- in ----
    /// Paid by the board on a roll, and by the second opening settlement.
    pub production: u32,
    /// Taken with an Invention card (R-9.9).
    pub invention: u32,
    /// Taken from everyone with a Monopoly (R-9.8).
    pub monopoly: u32,
    /// Taken from a hand by the robber, on a seven or a militia (R-6.4).
    pub stolen: u32,
    /// Arrived in a trade, with a person or with the supply.
    pub traded_in: u32,
    // ---- out ----
    /// Spent on a road, a settlement, a city or a development card.
    pub built: u32,
    /// Thrown away to a seven (R-6.2).
    pub discarded: u32,
    /// Taken out of this hand by the robber.
    pub robbed: u32,
    /// Taken out of this hand by somebody else's Monopoly.
    pub monopolised: u32,
    /// Left in a trade.
    pub traded_out: u32,
    /// Still in hand when the game ended.
    pub held: u32,
}

impl Ledger {
    /// Cards that arrived, however they arrived.
    pub fn came_in(&self) -> u32 {
        self.production + self.invention + self.monopoly + self.stolen + self.traded_in
    }

    /// Cards that left, however they left.
    pub fn went_out(&self) -> u32 {
        self.built + self.discarded + self.robbed + self.monopolised + self.traded_out
    }

    /// The whole claim: what came in, less what went out, is what is left.
    ///
    /// If this is ever false there is a way to move a card that the ledger does
    /// not know about, and every figure in it is short by that much.
    pub fn balances(&self) -> bool {
        self.came_in() == self.went_out() + self.held
    }

    /// In and out, in the order the card reads them.
    pub fn rows(&self) -> [(&'static str, u32, bool); 10] {
        [
            ("production", self.production, true),
            ("invention", self.invention, true),
            ("monopoly", self.monopoly, true),
            ("stolen", self.stolen, true),
            ("traded in", self.traded_in, true),
            ("built", self.built, false),
            ("discarded", self.discarded, false),
            ("robbed", self.robbed, false),
            ("monopolised", self.monopolised, false),
            ("traded out", self.traded_out, false),
        ]
    }
}

/// Follow every card through the game.
///
/// Read off the hands rather than off the rules: each move is applied and the
/// hands are compared either side of it, so a card that moved is counted
/// whether or not this function knows why it moved. Only the *reason* comes
/// from the action, and the match over it is exhaustive, so a new action
/// cannot be added without deciding where its cards belong.
///
/// Gross rather than net, per resource. A trade of two wheat for one ore is one
/// card in and two out, and a net hand size would call it one card out and lose
/// both figures.
fn ledger_of(saved: &Saved) -> [Ledger; MAX_PLAYERS] {
    use carranta_core::action::Action;

    let mut state = crate::game::Session::opening(saved.seats, saved.seed, saved.mode);
    let seats = state.players as usize;
    let mut out = [Ledger::default(); MAX_PLAYERS];
    for step in &saved.moves {
        let Step::Move(action) = *step else { continue };
        let actor = state.to_act as usize;
        let before = state.hand;
        if state.apply(action).is_err() {
            break;
        }
        for (p, led) in out.iter_mut().enumerate().take(seats) {
            let up: u32 = (0..5)
                .map(|r| u32::from(state.hand[p][r].saturating_sub(before[p][r])))
                .sum();
            let down: u32 = (0..5)
                .map(|r| u32::from(before[p][r].saturating_sub(state.hand[p][r])))
                .sum();
            if up == 0 && down == 0 {
                continue;
            }
            match action {
                // The board paying out, on a roll or on the second settlement
                // of the opening, which pays for the hexes it touches.
                Action::Roll | Action::PlaceSettlement(_) => led.production += up,
                Action::PlayInvention(_) => led.invention += up,
                // One seat takes and the rest lose, in the same action.
                Action::PlayMonopoly(_) => {
                    led.monopoly += up;
                    led.monopolised += down;
                }
                Action::MoveRobber { .. } => {
                    led.stolen += up;
                    led.robbed += down;
                }
                Action::Discard { .. } => led.discarded += down,
                // Both sides of a trade move both ways, which is why this is
                // counted per resource rather than as a change in hand size.
                Action::Trade { .. } | Action::AcceptTrade { .. } => {
                    led.traded_in += up;
                    led.traded_out += down;
                }
                Action::BuildRoad(_)
                | Action::BuildSettlement(_)
                | Action::BuildCity(_)
                | Action::BuyDev => led.built += down,
                // Nothing else moves a card. A militia played moves the robber,
                // and the robbery is its own action; road building pays in
                // roads; an offer made or withdrawn moves nothing at all.
                Action::PlaceRoad(_)
                | Action::PlayMilitia
                | Action::PlayRoadBuilding
                | Action::ProposeTrade { .. }
                | Action::WithdrawTrade { .. }
                | Action::EndTurn => {
                    debug_assert!(false, "{action:?} moved cards for seat {p} (actor {actor})");
                }
            }
        }
    }
    for (p, led) in out.iter_mut().enumerate().take(seats) {
        led.held = state.hand[p].iter().map(|n| u32::from(*n)).sum();
    }
    out
}

/// Production against expectation, turn by turn.
///
/// Cumulative on both counts, so each line only ever climbs and the gap between
/// a pair of them is everything that has happened to that seat so far. A
/// per-turn figure would be nearly all zeroes with occasional spikes: most
/// turns pay a given player nothing.
///
/// Expected is the pips through the buildings standing when the dice were
/// thrown, at fair odds, with the robber ignored (§10.2's `e_raw`). So a seat
/// under blockade watches its actual line fall away from its expected one,
/// which is the robber's cost drawn rather than tabulated.
#[derive(Clone, Debug, Default)]
pub struct Series {
    /// Cards collected by the end of each turn, per seat and resource.
    pub actual: Vec<[[u32; 5]; MAX_PLAYERS]>,
    /// What a fair pair owed them over the same turns.
    pub expected: Vec<[[f64; 5]; MAX_PLAYERS]>,
}

impl Series {
    pub fn turns(&self) -> usize {
        self.actual.len()
    }

    /// The largest number either line reaches, which is the axis both share.
    ///
    /// One axis for both, or the gap between a pair of lines would be a
    /// picture of two different scales rather than of a difference.
    pub fn ceiling(&self, seats: usize) -> f64 {
        let top = |a: f64, b: f64| if b > a { b } else { a };
        let actual = self
            .actual
            .last()
            .map(|row| {
                (0..seats)
                    .map(|p| f64::from(row[p].iter().sum::<u32>()))
                    .fold(0.0, top)
            })
            .unwrap_or(0.0);
        let expected = self
            .expected
            .last()
            .map(|row| {
                (0..seats)
                    .map(|p| row[p].iter().sum::<f64>())
                    .fold(0.0, top)
            })
            .unwrap_or(0.0);
        top(actual, expected)
    }

    /// The same for one seat, read a resource at a time.
    pub fn ceiling_of(&self, seat: usize) -> f64 {
        let top = |a: f64, b: f64| if b > a { b } else { a };
        let a = self
            .actual
            .last()
            .map(|row| row[seat].iter().map(|n| f64::from(*n)).fold(0.0, top))
            .unwrap_or(0.0);
        let e = self
            .expected
            .last()
            .map(|row| row[seat].iter().copied().fold(0.0, top))
            .unwrap_or(0.0);
        top(a, e)
    }
}

/// Follow production and its expectation across the game.
///
/// Sampled at the end of every turn, on the same boundaries `turns_of` uses, so
/// a point on this chart and a row in that table are talking about the same
/// turn. The setup is left out for the same reason: it comes before anybody has
/// a turn to take, and the second settlement's payout lands in the first turn's
/// figure instead.
type Sampled = (
    Series,
    Cover,
    Vec<[u32; MAX_PLAYERS]>,
    Vec<[u32; MAX_PLAYERS]>,
    Vec<[f64; MAX_PLAYERS]>,
    Robber,
    Hands,
    Built,
);

/// One turn's worth of everything read off the board rather than off the moves:
/// where the robber is sitting, how full every hand is, and who is stuck.
fn watch(
    state: &carranta_core::state::State,
    seats: usize,
    robber: &mut Robber,
    hands: &mut Hands,
    built: &mut Built,
) {
    use carranta_core::topology::{HEX_COUNT, hex_vertices};
    /// A hand this size or smaller survives a seven (R-6.2).
    const KEEP: u32 = 7;

    robber.turns += 1;
    hands.turns += 1;
    let sitting = state.robber as usize;
    if sitting < HEX_COUNT {
        robber.dwell[sitting] += 1;
        let corners = hex_vertices(state.robber);
        for p in 0..seats {
            // A robber costs a seat nothing unless it sits on something they
            // built on. The desert needs no special case: nobody builds around a
            // hex that pays nothing, so it falls out on its own.
            if (state.settlements[p] | state.cities[p]) & corners != 0 {
                robber.blocked[p] += 1;
            }
        }
    }
    for p in 0..seats {
        if state.hand[p].iter().map(|n| u32::from(*n)).sum::<u32>() > KEEP {
            hands.over[p] += 1;
        }
    }

    built.turns += 1;
    for p in 0..seats {
        // Able to pay and unable to build, which is a different thing from
        // saving up: those cards cannot be spent, and they sit in the hand
        // waiting for a seven to take half of them. Both halves matter in each
        // case: nowhere to build is only a problem for a seat that could have
        // paid, and a full hand is only stuck if there is nothing to spend it on.
        use carranta_core::action::{CITY_COST, ROAD_COST, SETTLEMENT_COST};
        let cannot = [
            state.holds(p, &SETTLEMENT_COST)
                && (state.settlement_spots(p, false) == 0 || state.settlements_left[p] == 0),
            // A city is an upgrade, so the wall is having no settlement of your
            // own left standing to upgrade, or no city pieces left.
            state.holds(p, &CITY_COST) && (state.settlements[p] == 0 || state.cities_left[p] == 0),
            state.holds(p, &ROAD_COST) && (state.road_spots(p) == 0 || state.roads_left[p] == 0),
        ];
        for (kind, blocked) in cannot.iter().enumerate() {
            if *blocked {
                built.stuck[p][kind] += 1;
            }
        }
        // The length is recomputed rather than remembered: a road's length can
        // *fall* when somebody builds a settlement through the middle of it
        // (R-10.3), so the last value is the only one that is the answer.
        built.chain[p] =
            carranta_core::longest_road::longest_road(state.roads[p], state.blocking(p));
    }
}

fn series_of(saved: &Saved) -> Sampled {
    use carranta_core::action::Action;
    use carranta_core::state::Phase;

    let mut state = crate::game::Session::opening(saved.seats, saved.seed, saved.mode);
    let seats = state.players as usize;
    let mut out = Series::default();
    // Coverage and the score are sampled here rather than in replays of their
    // own, so a turn on one chart is beyond argument the same turn on another.
    // Coverage reads the robber where it stands at the end of the turn. A militia
    // played mid-turn moves the robber before that turn's roll, so for that one
    // turn the sample can differ from the board the roll actually faced. Left
    // alone deliberately: every other figure here is an end-of-turn board, and
    // one sample in a hundred and fifty moving by one hex is worth less than two
    // clocks on one page.
    let mut cover = Cover::default();
    let mut score: Vec<[u32; MAX_PLAYERS]> = Vec::new();
    let mut engine: Vec<[f64; MAX_PLAYERS]> = Vec::new();
    let mut robber = Robber::default();
    let mut hands = Hands::default();
    let mut built = Built::default();
    // What the board would pay each seat on one roll, given the buildings
    // standing at that moment. The engine itself rather than what it earned:
    // read off the board, so no number of rolls or dice can move it.
    let sized = |state: &carranta_core::state::State| -> [f64; MAX_PLAYERS] {
        let owed = production::expectation(state);
        core::array::from_fn(|p| owed[p].iter().sum())
    };
    // The true score, hidden cards and all, which is what the result table
    // reports and therefore what the last point of the chart has to equal.
    let scored = |state: &carranta_core::state::State| -> [u32; MAX_PLAYERS] {
        let p = points_of(state, seats);
        core::array::from_fn(|i| p[i].total())
    };
    // And the score the table could see: the same total less the victory point
    // cards nobody else knew about. The gap between the two is the whole tension
    // of an endgame, and only the true score is in the result.
    let mut seen: Vec<[u32; MAX_PLAYERS]> = Vec::new();
    let visible = |state: &carranta_core::state::State| -> [u32; MAX_PLAYERS] {
        let p = points_of(state, seats);
        core::array::from_fn(|i| p[i].total() - p[i].cards.points)
    };
    let mut actual = [[0u32; 5]; MAX_PLAYERS];
    let mut expected = [[0.0f64; 5]; MAX_PLAYERS];
    let mut playing = false;
    for step in &saved.moves {
        let Step::Move(action) = *step else { continue };
        // The board as it stood when the dice were thrown is what the roll was
        // owed, so the expectation is read before the roll lands.
        let owed = matches!(action, Action::Roll)
            .then(|| carranta_analytics::production::expectation(&state));
        let before = state.hand;
        // A discard names its own player and its own card, so it needs no diffing.
        if let Action::Discard { player, resource } = action
            && usize::from(player) < seats
        {
            hands.thrown[usize::from(player)][resource as usize] += 1;
        }
        // Which of the four things this move bought, if any, and whose it was.
        let buying = match action {
            Action::BuildRoad(_) => Some(0),
            Action::BuildSettlement(_) => Some(1),
            Action::BuildCity(_) => Some(2),
            Action::BuyDev => Some(3),
            _ => None,
        };
        let buyer = state.to_act as usize;
        // What a road is worth is the difference it makes, so both halves of the
        // difference are read either side of the move: the spots the seat may
        // build a settlement on, and the length of their longest run.
        let reach = (buying == Some(0) && buyer < seats).then(|| {
            (
                state.settlement_spots(buyer, false).count_ones(),
                carranta_core::longest_road::longest_road(
                    state.roads[buyer],
                    state.blocking(buyer),
                ),
            )
        });
        if state.apply(action).is_err() {
            break;
        }
        if let Some((spots, chain)) = reach {
            let now = state.settlement_spots(buyer, false).count_ones();
            let long = carranta_core::longest_road::longest_road(
                state.roads[buyer],
                state.blocking(buyer),
            );
            let opened = now > spots;
            let stretched = long > chain;
            built.spots[buyer] += now.saturating_sub(spots);
            built.opened[buyer] += u32::from(opened);
            built.stretched[buyer] += u32::from(stretched);
            built.idle[buyer] += u32::from(!opened && !stretched);
        }
        if let Some(kind) = buying
            && buyer < seats
        {
            built.pieces[buyer][kind] += 1;
            // The price is read off the hand rather than from the rules table, so
            // a free road from a Road Building card costs what it actually cost,
            // which is nothing.
            let paid: u32 = (0..5)
                .map(|res| u32::from(before[buyer][res].saturating_sub(state.hand[buyer][res])))
                .sum();
            built.spent[buyer][kind] += paid;
        }
        if let Some(owed) = owed {
            for p in 0..seats {
                for res in 0..5 {
                    expected[p][res] += owed[p][res];
                }
            }
        }
        // Only the board pays production. A trade or a steal moves cards
        // between hands and adds nothing to what the board produced.
        if matches!(action, Action::Roll | Action::PlaceSettlement(_)) {
            for p in 0..seats {
                for res in 0..5 {
                    let got = u32::from(state.hand[p][res].saturating_sub(before[p][res]));
                    actual[p][res] += got;
                    // The opening settlement's payout is a certainty rather
                    // than a wager: it pays what it touches, once, with no
                    // dice involved. So it is owed exactly what it paid.
                    // Counting it in one line and not the other would offset
                    // every seat by a few cards for the whole game.
                    if owed.is_none() {
                        expected[p][res] += f64::from(got);
                    }
                }
            }
        }
        if playing && action == Action::EndTurn {
            out.actual.push(actual);
            out.expected.push(expected);
            cover.live.push(production::coverage(&state, true));
            cover.open.push(production::coverage(&state, false));
            cover
                .each
                .push(production::coverage_by_resource(&state, true));
            score.push(scored(&state));
            seen.push(visible(&state));
            engine.push(sized(&state));
            watch(&state, seats, &mut robber, &mut hands, &mut built);
        }
        playing |= matches!(state.phase, Phase::PreRoll);
    }
    // The winning turn never ends, so its cards would be missing from the last
    // point and the chart would stop short of what the ledger counts. A closing
    // sample carries them, and is only added when there is something to carry.
    if out.actual.last() != Some(&actual) {
        out.actual.push(actual);
        out.expected.push(expected);
        cover.live.push(production::coverage(&state, true));
        cover.open.push(production::coverage(&state, false));
        cover
            .each
            .push(production::coverage_by_resource(&state, true));
        score.push(scored(&state));
        seen.push(visible(&state));
        engine.push(sized(&state));
        watch(&state, seats, &mut robber, &mut hands, &mut built);
    }
    robber.spots_of(&state);
    (out, cover, score, seen, engine, robber, hands, built)
}

/// How fast a seat's engine grew, fitted as a rate compounding turn by turn.
///
/// The premise, which is worth stating because it is an assumption and not a
/// fact: an economy that compounds beats one that is merely large. Cards a turn
/// buys buildings, buildings buy more cards a turn, and a seat whose rate keeps
/// climbing arrives at the end of the game with an engine the others cannot
/// catch. So the figure to rate an economy on is not its size but its slope.
///
/// Fitted on the **engine** rather than on the cards that arrived: what one
/// roll was worth to that seat, given the buildings standing at the time. The
/// cards that arrived are that engine plus the dice, and rating them would rate
/// the dice. It is read off the board rather than off the payouts for the same
/// reason: a turn that happened to hold no roll cannot make an engine look
/// smaller than it was.
///
/// Fitted in logs, which is what makes it a growth *rate*: a straight line
/// through the log of a rate is a rate multiplying by a constant every turn.
/// `fit` is how straight that line actually was, and it is the honest half of
/// the number, because compounding in this game is bounded on both ends: the
/// opening is a standing start, buildings run out, and the game stops at ten
/// points. A low fit means the growth figure is average steepness rather than
/// a law the seat was obeying.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Growth {
    /// What the rate multiplied by each turn, less one: .02 is two percent a
    /// turn.
    pub per_turn: f64,
    /// Turns for the rate to double at that growth, if it held. `None` when it
    /// was flat or falling, since nothing that is not growing doubles.
    pub doubling: Option<f64>,
    /// Expected cards a turn across the first quarter of the game.
    pub early: f64,
    /// And across the last, which is the engine the game ended with.
    pub late: f64,
    /// How much of the variation in the log of the engine a straight line
    /// explains, from 0 to 1. High means the engine really was multiplying.
    pub fit: f64,
    /// The same for a straight line through the engine itself rather than its
    /// log: high means the engine was growing by a constant amount a turn.
    ///
    /// Both, because over the range a real game covers, one and a half to two
    /// and a half times the opening, the log of a straight ramp is very nearly
    /// straight too. A good log fit on its own therefore does not distinguish
    /// compounding from steady accretion, which is the exact claim this whole
    /// figure exists to make, so the honest thing is to fit both and say which
    /// one the engine actually looked like.
    pub fit_line: f64,
    /// Turns the fit ran over, which is what a doubling time has to be read
    /// against: a doubling five games away is not a fact about this game.
    pub turns: usize,
}

/// Which shape an engine grew in, on the evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// It multiplied: the log fit beats the straight one by a clear margin.
    Compounding,
    /// It climbed by about the same amount every turn.
    Steady,
    /// It barely moved, so neither line describes anything.
    Flat,
}

impl Growth {
    /// What this engine looked like, which decides how much the growth figure
    /// beside it is worth.
    ///
    /// The margin is deliberately wide. Two fits within a hundredth of each
    /// other are not evidence for either shape, and calling that compounding
    /// would be the whole error this method exists to avoid.
    pub fn shape(&self) -> Shape {
        const MARGIN: f64 = 0.05;
        if self.per_turn < 0.001 || self.fit.max(self.fit_line) < 0.3 {
            Shape::Flat
        } else if self.fit > self.fit_line + MARGIN {
            Shape::Compounding
        } else {
            Shape::Steady
        }
    }

    /// Whether the growth figure is describing a shape the engine really had.
    pub fn believable(&self) -> bool {
        self.fit >= 0.5 && self.shape() != Shape::Flat
    }
}

/// Fit one seat's growth, or `None` when there is not enough game to fit.
///
/// A handful of turns can be fitted and cannot be believed, so a short game
/// declines to answer rather than answering badly.
pub fn growth_of(engine: &[[f64; MAX_PLAYERS]], seat: usize) -> Option<Growth> {
    /// Fewer turns than this and the fit is a line through noise.
    const ENOUGH: usize = 16;
    /// An engine smaller than this is not running, and its log is enormous.
    const FLOOR: f64 = 0.02;

    // The last turn is dropped: a game ends the moment somebody reaches the
    // target, so the winning turn is a part turn, and the board it was won on
    // is the board of the turn before anyway.
    let turns = engine.len().saturating_sub(1);
    if turns < ENOUGH {
        return None;
    }
    let size: Vec<f64> = (0..turns).map(|i| engine[i][seat]).collect();

    // The two ends, a quarter of the game each, which is what somebody means by
    // the engine they started with and the one they finished with.
    let quarter = (turns / 4).max(2);
    let mean = |xs: &[f64]| xs.iter().sum::<f64>() / xs.len() as f64;
    let early = mean(&size[..quarter]);
    let late = mean(&size[turns - quarter..]);

    // Least squares on the log of the engine, over the turns it was running.
    let pts: Vec<(f64, f64)> = size
        .iter()
        .enumerate()
        .filter(|(_, r)| **r > FLOOR)
        .map(|(i, r)| (i as f64, r.ln()))
        .collect();
    if pts.len() < ENOUGH {
        return None;
    }
    let n = pts.len() as f64;
    let (mx, my) = (
        pts.iter().map(|(x, _)| x).sum::<f64>() / n,
        pts.iter().map(|(_, y)| y).sum::<f64>() / n,
    );
    let sxx: f64 = pts.iter().map(|(x, _)| (x - mx) * (x - mx)).sum();
    let sxy: f64 = pts.iter().map(|(x, y)| (x - mx) * (y - my)).sum();
    let syy: f64 = pts.iter().map(|(_, y)| (y - my) * (y - my)).sum();
    if sxx <= 0.0 {
        return None;
    }
    let slope = sxy / sxx;
    // R squared, which for a straight line is the square of the correlation.
    // No p-value anywhere near this (§10.1).
    let fit = if syy > 0.0 {
        (sxy * sxy / (sxx * syy)).clamp(0.0, 1.0)
    } else {
        0.0
    };
    // And the same fit against the engine itself, which is the rival account of
    // the same numbers: not multiplying, just climbing.
    let straight: Vec<(f64, f64)> = size
        .iter()
        .enumerate()
        .map(|(i, r)| (i as f64, *r))
        .collect();
    let fit_line = r_squared(&straight);
    Some(Growth {
        per_turn: slope.exp() - 1.0,
        doubling: (slope > 1e-9).then(|| std::f64::consts::LN_2 / slope),
        early,
        late,
        fit,
        fit_line,
        turns,
    })
}

/// How much of the variation in `y` a straight line through it explains.
fn r_squared(pts: &[(f64, f64)]) -> f64 {
    let n = pts.len() as f64;
    if n < 2.0 {
        return 0.0;
    }
    let (mx, my) = (
        pts.iter().map(|(x, _)| x).sum::<f64>() / n,
        pts.iter().map(|(_, y)| y).sum::<f64>() / n,
    );
    let sxx: f64 = pts.iter().map(|(x, _)| (x - mx) * (x - mx)).sum();
    let sxy: f64 = pts.iter().map(|(x, y)| (x - mx) * (y - my)).sum();
    let syy: f64 = pts.iter().map(|(_, y)| (y - my) * (y - my)).sum();
    if sxx <= 0.0 || syy <= 0.0 {
        return 0.0;
    }
    (sxy * sxy / (sxx * syy)).clamp(0.0, 1.0)
}

/// How long a development card sat in hand before it was played.
///
/// The cards table says how many of each kind were bought and how many played.
/// It cannot say *when*: a militia played the turn it was drawn and a militia held
/// for forty turns are the same row, and they are not the same decision. One is a
/// seven happening to somebody; the other is a player waiting for the robber to be
/// worth moving.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Waits {
    /// Turns held before being played, one list a kind, in deck order.
    pub held: [Vec<u32>; 5],
    /// Cards of each kind still in a hand when the game ended, and how long they
    /// had been there. A card held to the end is a decision too, and a mean over
    /// played cards alone would quietly leave it out.
    pub kept: [Vec<u32>; 5],
}

impl Waits {
    /// The mean wait for a kind that was played, or `None` if none was.
    pub fn mean(&self, kind: usize) -> Option<f64> {
        let held = &self.held[kind];
        (!held.is_empty())
            .then(|| held.iter().map(|t| f64::from(*t)).sum::<f64>() / held.len() as f64)
    }

    /// The longest a card of this kind waited before it was played.
    pub fn longest(&self, kind: usize) -> Option<u32> {
        self.held[kind].iter().copied().max()
    }
}

/// Match every card played back to the turn it was drawn on.
///
/// Cards of a kind are interchangeable, so a play is matched to the *oldest*
/// unplayed card of that kind. Any other convention would be arbitrary in the
/// same way and read worse: first in, first out is what a hand of identical cards
/// means.
///
/// The kind drawn is read off the hand rather than from the action, since buying
/// a card is one action whatever it turns out to be.
fn waits_of(saved: &Saved) -> Waits {
    use carranta_core::action::Action;
    use carranta_core::state::{DevCard, Phase};

    let mut state = crate::game::Session::opening(saved.seats, saved.seed, saved.mode);
    let seats = state.players as usize;
    let mut out = Waits::default();
    // Drawn and not yet played, per seat and kind: the turns they arrived on.
    let mut queue: Vec<[Vec<u32>; 5]> = vec![Default::default(); MAX_PLAYERS];
    let mut turn = 0u32;
    let mut playing = false;
    for step in &saved.moves {
        let Step::Move(action) = *step else { continue };
        let actor = state.to_act as usize;
        let before = state.dev_held;
        let played = match action {
            Action::PlayMilitia => Some(DevCard::Militia as usize),
            Action::PlayRoadBuilding => Some(DevCard::RoadBuilding as usize),
            Action::PlayInvention(_) => Some(DevCard::Invention as usize),
            Action::PlayMonopoly(_) => Some(DevCard::Monopoly as usize),
            _ => None,
        };
        if state.apply(action).is_err() {
            break;
        }
        let at = turn.max(1);
        if actor < seats {
            if matches!(action, Action::BuyDev) {
                // Whichever count went up is the card that was drawn.
                for kind in 0..5 {
                    if state.dev_held[actor][kind] > before[actor][kind] {
                        queue[actor][kind].push(at);
                    }
                }
            }
            if let Some(kind) = played {
                // The oldest of that kind, or nothing to match against if the
                // records disagree, which a replay would already have refused.
                if !queue[actor][kind].is_empty() {
                    let drawn = queue[actor][kind].remove(0);
                    out.held[kind].push(at.saturating_sub(drawn));
                }
            }
        }
        if playing && action == Action::EndTurn {
            turn += 1;
        }
        if matches!(state.phase, Phase::PreRoll) && !playing {
            playing = true;
            turn = 1;
        }
    }
    // Whatever is left was held to the end.
    let last = turn.max(1);
    for seat in queue.iter().take(seats) {
        for (kind, drawn) in seat.iter().enumerate() {
            for at in drawn {
                out.kept[kind].push(last.saturating_sub(*at));
            }
        }
    }
    out
}

/// What was actually *in* the offers a seat made.
///
/// The trades card counts offers made, withdrawn and turned down. It cannot say
/// why: a seat nobody would deal with and a seat asking two cards for one are
/// completely different problems wearing the same three counts. This is the ask.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Asks {
    /// Offers this seat put on the table.
    pub offers: [u32; MAX_PLAYERS],
    /// Cards asked for across those offers, and cards put up for them.
    pub wanted: [u32; MAX_PLAYERS],
    pub given: [u32; MAX_PLAYERS],
    /// Offers of theirs that somebody took.
    pub taken: [u32; MAX_PLAYERS],
    /// Cards asked for and cards put up, resource by resource.
    ///
    /// What a seat was short of, in its own words. The production card says what
    /// the board failed to pay them; this says what they went looking for, which
    /// is not always the same thing and is the more interesting of the two when
    /// they differ.
    pub wanted_each: [[u32; 5]; MAX_PLAYERS],
    pub given_each: [[u32; 5]; MAX_PLAYERS],
    /// Offers addressed to one seat rather than to the table.
    ///
    /// The generator only ever makes open offers, on purpose: an addressed offer
    /// multiplies the action space by the number of opponents for no gain to a
    /// search. So this is nought in every bot game, and a human client may still
    /// address one, which is why it is counted rather than assumed away.
    pub addressed: [u32; MAX_PLAYERS],
}

impl Asks {
    /// Cards asked for each card put up, or `None` for a seat that never
    /// offered anything.
    ///
    /// One is an even swap. Above one is a seat asking to come out ahead, which
    /// is every seat's right and also the likeliest reason nobody took it.
    pub fn ask(&self, seat: usize) -> Option<f64> {
        (self.given[seat] > 0).then(|| f64::from(self.wanted[seat]) / f64::from(self.given[seat]))
    }
}

/// Read the offers, and who took them, off the moves.
///
/// An acceptance names the offer rather than the offerer, so the live market is
/// consulted before the move is applied: afterwards the offer it names is gone.
fn asks_of(saved: &Saved) -> Asks {
    use carranta_core::action::Action;

    let mut state = crate::game::Session::opening(saved.seats, saved.seed, saved.mode);
    let seats = state.players as usize;
    let mut out = Asks::default();
    for step in &saved.moves {
        let Step::Move(action) = *step else { continue };
        match action {
            Action::ProposeTrade { by, to, give, want } if usize::from(by) < seats => {
                let p = usize::from(by);
                out.offers[p] += 1;
                out.addressed[p] += u32::from(to.is_some());
                out.given[p] += give.iter().map(|n| u32::from(*n)).sum::<u32>();
                out.wanted[p] += want.iter().map(|n| u32::from(*n)).sum::<u32>();
                for res in 0..5 {
                    out.given_each[p][res] += u32::from(give[res]);
                    out.wanted_each[p][res] += u32::from(want[res]);
                }
            }
            Action::AcceptTrade { offer, .. } => {
                if let Some(o) = state.live_offers().get(usize::from(offer)) {
                    let from = usize::from(o.from);
                    if from < seats {
                        out.taken[from] += 1;
                    }
                }
            }
            _ => {}
        }
        if state.apply(action).is_err() {
            break;
        }
    }
    out
}

/// What each seat built, what it cost, and what stopped it.
///
/// The ledger says a seat spent forty-six cards on building. Roads, settlements,
/// cities and development cards are four different decisions, the split is in the
/// moves, and one number for all of them says nothing about which game the seat
/// was playing.
///
/// The last two figures are not spending at all. A road network's length is what
/// the longest road tile is contested on, and it is the one thing a seat builds
/// that the score table cannot show unless they win it. And a seat that could
/// afford a settlement with nowhere legal to put it was not saving up: it was
/// stuck, which is a real and otherwise invisible way to lose a game.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Built {
    /// Pieces and cards bought, per seat: roads, settlements, cities, cards.
    pub pieces: [[u32; 4]; MAX_PLAYERS],
    /// Cards spent on each of those, in the same order.
    pub spent: [[u32; 4]; MAX_PLAYERS],
    /// The longest continuous road each seat finished with (R-10.3).
    pub chain: [u32; MAX_PLAYERS],
    /// What each road did, per seat: opened at least one new settlement spot,
    /// lengthened the longest chain, neither.
    ///
    /// A road can do both, so the first two overlap and only `idle` is exclusive.
    /// A road that does neither is not necessarily wasted, since a network can be
    /// grown towards a spot two roads away, but a seat whose roads are mostly
    /// idle was building without a plan or building into a wall.
    pub opened: [u32; MAX_PLAYERS],
    pub stretched: [u32; MAX_PLAYERS],
    pub idle: [u32; MAX_PLAYERS],
    /// Settlement spots the roads opened, counted as they arrived.
    pub spots: [u32; MAX_PLAYERS],
    /// Turns ended able to afford a thing and unable to build it, one count per
    /// kind: a settlement with nowhere legal, a city with no settlement of their
    /// own to upgrade, a road with nowhere to put it or none left.
    pub stuck: [[u32; 3]; MAX_PLAYERS],
    /// Turns sampled, for the share.
    pub turns: usize,
}

impl Built {
    /// The four kinds, in the order they are stored.
    pub const KINDS: [&'static str; 4] = ["roads", "settlements", "cities", "cards"];

    /// The three things a seat can be stuck on, in the order they are stored.
    pub const STUCK: [&'static str; 3] = ["a settlement", "a city", "a road"];

    /// Cards this seat spent on building, all four kinds together. The ledger's
    /// `built` row by another route, which is what makes it a check.
    pub fn spent_all(&self, seat: usize) -> u32 {
        self.spent[seat].iter().sum()
    }
}

/// One thing that happened, and when.
///
/// Enough to answer "what was going on around turn ninety", which is the question
/// every other chart on the page provokes and none of them can answer: a line
/// steps up and nothing says why. Buildings, cards and the two tiles, which is
/// everything that changes a score or an engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Event {
    /// The turn it landed in, counting the way the turns table counts.
    pub turn: u32,
    pub seat: usize,
    pub what: Happened,
}

/// The kinds of thing worth a mark on a timeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Happened {
    Settlement,
    City,
    /// A development card bought. What it turned out to be is the deck's
    /// business and the card cards' business, not the timeline's.
    Card,
    /// Longest road or largest militia arriving. Two points changing hands, and
    /// the only marks here that can also be lost.
    Tile,
}

impl Happened {
    /// A class for the mark, and a word for its tooltip.
    ///
    /// The classes are prefixed because the obvious names are taken and taking
    /// them again is a bug that has now happened twice: `mark` is the header's
    /// wordmark, and `tile` is the hex in the opening card, whose `width` and
    /// `height` apply to an SVG rect as CSS and silently flattened every one of
    /// these to nothing.
    pub fn mark(self) -> (&'static str, &'static str) {
        match self {
            Happened::Settlement => ("beat-house", "a settlement"),
            Happened::City => ("beat-city", "a city"),
            Happened::Card => ("beat-card", "a development card"),
            Happened::Tile => ("beat-tile", "a tile"),
        }
    }
}

/// Read the timeline off the moves.
///
/// The two tiles are not moves at all, they are consequences, so they are read
/// by watching who holds them either side of every action rather than by looking
/// for an action that grants them.
fn events_of(saved: &Saved) -> Vec<Event> {
    use carranta_core::action::Action;
    use carranta_core::state::Phase;

    let mut state = crate::game::Session::opening(saved.seats, saved.seed, saved.mode);
    let mut out = Vec::new();
    let mut turn = 0u32;
    let mut playing = false;
    let (mut road, mut militia) = (state.longest_road, state.largest_militia);
    for step in &saved.moves {
        let Step::Move(action) = *step else { continue };
        let by = state.to_act as usize;
        if state.apply(action).is_err() {
            break;
        }
        // Setup placements come before anybody has a turn, so they are turn one:
        // the opening card is where they are really reported.
        let at = turn.max(1);
        let what = match action {
            Action::BuildSettlement(_) | Action::PlaceSettlement(_) => Some(Happened::Settlement),
            Action::BuildCity(_) => Some(Happened::City),
            Action::BuyDev => Some(Happened::Card),
            _ => None,
        };
        if let Some(what) = what {
            out.push(Event {
                turn: at,
                seat: by,
                what,
            });
        }
        // A tile arriving is a change of holder, whoever's action caused it.
        for (held, was) in [
            (state.longest_road, &mut road),
            (state.largest_militia, &mut militia),
        ] {
            if held != *was {
                if let Some(now) = held {
                    out.push(Event {
                        turn: at,
                        seat: now as usize,
                        what: Happened::Tile,
                    });
                }
                *was = held;
            }
        }
        if playing && action == Action::EndTurn {
            turn += 1;
        }
        if matches!(state.phase, Phase::PreRoll) && !playing {
            playing = true;
            turn = 1;
        }
    }
    out
}

/// Where a game's wall-clock time went, by the kind of decision it went on.
///
/// The clock in a version 2 file is stamped per move, and the page only ever
/// added it up per seat: a hundred and fifty turns of "twelve seconds each" and
/// no way to ask what the twelve seconds were spent on. The recorded dimension
/// was there and unused, which made it the cheapest gap on the page to close.
///
/// Time is charged to the move that *ends* the wait, not the one before it: the
/// gap between two stamps is somebody deciding what to do next, and what they
/// decided is the move that lands. The very first stamp measures from the deal,
/// which is the setup thinking about its own first placement.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Spent {
    /// Milliseconds and moves, one bucket a kind, in [`KINDS`] order.
    pub by_kind: [(u32, u32); KINDS.len()],
}

/// The kinds a decision is charged to, in the order they are worth reading.
pub const KINDS: [&str; 8] = [
    "setup",
    "rolling",
    "building",
    "development cards",
    "trading",
    "the robber",
    "discarding",
    "ending a turn",
];

impl Spent {
    /// Every millisecond charged to something.
    pub fn total(&self) -> u32 {
        self.by_kind.iter().map(|(ms, _)| ms).sum()
    }

    /// Which bucket a move belongs in.
    fn kind_of(action: &carranta_core::action::Action) -> usize {
        use carranta_core::action::Action as A;
        match action {
            A::PlaceSettlement(_) | A::PlaceRoad(_) => 0,
            A::Roll => 1,
            A::BuildRoad(_) | A::BuildSettlement(_) | A::BuildCity(_) => 2,
            A::BuyDev
            | A::PlayMilitia
            | A::PlayRoadBuilding
            | A::PlayInvention(_)
            | A::PlayMonopoly(_) => 3,
            A::Trade { .. }
            | A::ProposeTrade { .. }
            | A::AcceptTrade { .. }
            | A::WithdrawTrade { .. } => 4,
            A::MoveRobber { .. } => 5,
            A::Discard { .. } => 6,
            A::EndTurn => 7,
        }
    }
}

/// Split a game's clock by what the time was spent deciding.
///
/// `None` for a game saved before there was a clock, which reads perfectly well
/// and simply has nothing to say about time.
fn spent_of(saved: &Saved) -> Option<Spent> {
    if saved.times.is_empty() || saved.times.len() != saved.moves.len() {
        return None;
    }
    let mut out = Spent::default();
    let mut last = 0u32;
    for (step, at) in saved.moves.iter().zip(&saved.times) {
        let waited = at.saturating_sub(last);
        last = *at;
        // A refusal is a decision somebody took time over too, and it is a
        // trading decision: it is an offer being turned down.
        let kind = match step {
            Step::Move(action) => Spent::kind_of(action),
            Step::Passed { .. } => 4,
        };
        out.by_kind[kind].0 += waited;
        out.by_kind[kind].1 += 1;
    }
    Some(out)
}

/// Where the robber sat, and who it sat on.
///
/// The sankey says who took cards from whom, which is the robber's *other* job.
/// This is the blockade: a robber on a hex nobody is building on costs nobody
/// anything, and a robber parked on the wheat 8 for thirty turns decides a game
/// without ever stealing a card. The two are separate facts and only one of them
/// was on the page.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Robber {
    /// Turns spent on each hex, hex by hex.
    pub dwell: [u32; carranta_core::topology::HEX_COUNT],
    /// Turns spent on a hex touching each seat's buildings, which is the only
    /// robber position that costs that seat anything.
    pub blocked: [u32; MAX_PLAYERS],
    /// Turns sampled, so a share has a denominator.
    pub turns: usize,
    /// The hexes it sat on, longest first.
    pub spots: Vec<Spot>,
}

/// One hex the robber sat on, named the way a player would name it.
///
/// The number and what the hex makes, not the hex index: "the wheat 8" is a
/// thing somebody remembers and "hex 11" is not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spot {
    pub number: u8,
    pub resource: Option<usize>,
    pub turns: u32,
}

impl Robber {
    /// Where it sat, longest first, resolved against the board it sat on.
    fn spots_of(&mut self, state: &carranta_core::state::State) {
        let mut out: Vec<(usize, Spot)> = self
            .dwell
            .iter()
            .enumerate()
            .filter(|(_, turns)| **turns > 0)
            .map(|(h, turns)| {
                (
                    h,
                    Spot {
                        number: state.number[h],
                        resource: state.terrain[h].yields().map(|r| r as usize),
                        turns: *turns,
                    },
                )
            })
            .collect();
        // Longest first, and the hex index breaks a tie, so the answer is stable
        // rather than whatever the sort happened to do with equal turns.
        out.sort_by(|a, b| b.1.turns.cmp(&a.1.turns).then(a.0.cmp(&b.0)));
        self.spots = out.into_iter().map(|(_, spot)| spot).collect();
    }
}

/// How full each seat's hand got, turn by turn.
///
/// The ledger counts cards discarded, which is what a seven cost. This is the
/// exposure that made it possible: a seat sitting on eight cards is holding a bet
/// that the next seven belongs to somebody else, and the two facts read
/// completely differently. Discarding nothing all game is either careful play or
/// a quiet table, and only this tells them apart.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Hands {
    /// Turns each seat ended holding more than the discard limit (R-6.2).
    pub over: [u32; MAX_PLAYERS],
    /// Turns sampled.
    pub turns: usize,
    /// Cards thrown away to a seven, resource by resource.
    ///
    /// A discard is a decision: the rule takes half a hand and the player picks
    /// which half. What a seat threw away is what it had decided it did not need,
    /// and the ledger's single total cannot say that.
    pub thrown: [[u32; 5]; MAX_PLAYERS],
}

/// How often the board paid each seat anything, turn by turn.
///
/// The companion to [`Series`]: that follows how much a seat collected, this
/// follows how often they collected at all. A seat can be building all game and
/// still be paid on a quarter of the rolls, and the two lines say different
/// things about the same board.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cover {
    /// Per sample, per seat: the chance a roll pays them, robber and all.
    pub live: Vec<[f64; MAX_PLAYERS]>,
    /// The same with the robber ignored, which is what the buildings reach.
    /// The gap between the two is what the blockade cost, in rolls rather than
    /// in cards.
    pub open: Vec<[f64; MAX_PLAYERS]>,
    /// Per sample, per seat, per resource: the chance a roll pays them *that*
    /// card, robber and all.
    ///
    /// Coverage answers the trader's question, "does a roll pay me anything".
    /// This answers the builder's: a settlement wants a brick and a wood, and a
    /// seat covered on four numbers that all make wool is not covered for
    /// anything it wants to build.
    pub each: Vec<[[f64; 5]; MAX_PLAYERS]>,
}

impl Cover {
    /// How many turns it followed.
    pub fn turns(&self) -> usize {
        self.live.len()
    }

    /// A seat's mean coverage of one resource over the game.
    pub fn mean_of(&self, seat: usize, res: usize) -> f64 {
        if self.each.is_empty() {
            return 0.0;
        }
        self.each.iter().map(|row| row[seat][res]).sum::<f64>() / self.each.len() as f64
    }

    /// A seat's mean over the game, which is the figure to compare seats on: a
    /// coverage that was high for ten turns and low for a hundred was low.
    pub fn mean(&self, seat: usize, robber: bool) -> f64 {
        let rows = if robber { &self.live } else { &self.open };
        if rows.is_empty() {
            return 0.0;
        }
        rows.iter().map(|row| row[seat]).sum::<f64>() / rows.len() as f64
    }
}

/// One trade, as the two parties to it and what crossed between them.
///
/// Every trade in the game is one of these. Counts can be added up from them;
/// they cannot be got back out of counts, which is why the list is what is
/// kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Deal {
    /// The seat that gave `gave`.
    pub seat: usize,
    /// Who took it: a seat, or [`Trades::BANK`] or [`Trades::PORT`].
    pub with: usize,
    /// Which turn it landed in, counting the way the turns table counts.
    pub turn: u32,
    pub gave: [u8; 5],
    pub took: [u8; 5],
}

/// Every trade in the game, in the order they happened.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Trades {
    pub deals: Vec<Deal>,
}

impl Trades {
    /// The two counters, as party numbers past the last seat.
    pub const BANK: usize = MAX_PLAYERS;
    pub const PORT: usize = MAX_PLAYERS + 1;

    /// Trades this party was one end of.
    pub fn ends(&self, party: usize) -> usize {
        self.deals
            .iter()
            .filter(|d| d.seat == party || d.with == party)
            .count()
    }

    /// Trades between two seats, counted once.
    pub fn between(&self, a: usize, b: usize) -> u32 {
        self.deals
            .iter()
            .filter(|d| (d.seat == a && d.with == b) || (d.seat == b && d.with == a))
            .count() as u32
    }

    /// Trades this seat made against a given counter.
    pub fn against(&self, seat: usize, counter: usize) -> u32 {
        self.deals
            .iter()
            .filter(|d| d.seat == seat && d.with == counter)
            .count() as u32
    }

    /// Cards a seat handed over and cards it took back, across every trade it
    /// was a party to.
    ///
    /// A deal is recorded from one side, so the other side's cards are the same
    /// two hands read the other way round. Counting only the recorded side would
    /// make every counterparty look like it never traded.
    pub fn cards(&self, seat: usize) -> (u32, u32) {
        let (mut gave, mut took) = (0, 0);
        for d in &self.deals {
            let sum = |h: &[u8; 5]| h.iter().map(|n| u32::from(*n)).sum::<u32>();
            if d.seat == seat {
                gave += sum(&d.gave);
                took += sum(&d.took);
            } else if d.with == seat {
                gave += sum(&d.took);
                took += sum(&d.gave);
            }
        }
        (gave, took)
    }

    /// The same, restricted to trades between two particular seats.
    ///
    /// What one seat handed another across the game, which is the figure behind
    /// "who fed the winner": a table can lose to the seat it kept trading with.
    pub fn cards_between(&self, seat: usize, other: usize) -> (u32, u32) {
        let (mut gave, mut took) = (0, 0);
        for d in &self.deals {
            let sum = |h: &[u8; 5]| h.iter().map(|n| u32::from(*n)).sum::<u32>();
            if d.seat == seat && d.with == other {
                gave += sum(&d.gave);
                took += sum(&d.took);
            } else if d.seat == other && d.with == seat {
                gave += sum(&d.took);
                took += sum(&d.gave);
            }
        }
        (gave, took)
    }
}

/// Follow every trade to its counterparty.
///
/// A player trade names both sides: the accepter is in the action and the
/// proposer is on the offer it accepts, read before applying clears the offer
/// away.
///
/// A supply trade names only one, so the counter is worked out from the price.
/// Four cards for one is the bank; three or two is a port, and which port is a
/// question about the board rather than about the trade (R-7.9). Read off the
/// hand rather than off the ports the seat owns, since the hand is what
/// actually moved.
fn trades_of(saved: &Saved) -> Trades {
    use carranta_core::action::Action;
    use carranta_core::state::Phase;

    let mut state = crate::game::Session::opening(saved.seats, saved.seed, saved.mode);
    let seats = state.players as usize;
    let mut out = Trades::default();
    let mut turn = 0u32;
    let mut playing = false;
    for step in &saved.moves {
        let Step::Move(action) = *step else { continue };
        let proposer = match action {
            Action::AcceptTrade { offer, .. } => state
                .offers
                .get(offer as usize)
                .map(|o| o.from as usize)
                .filter(|p| *p < seats),
            _ => None,
        };
        let actor = state.to_act as usize;
        let before = state.hand;
        if state.apply(action).is_err() {
            break;
        }
        // What one seat gave and took, read off their hand either side.
        let moved = |p: usize| {
            let gave = core::array::from_fn(|r| before[p][r].saturating_sub(state.hand[p][r]));
            let took = core::array::from_fn(|r| state.hand[p][r].saturating_sub(before[p][r]));
            (gave, took)
        };
        match action {
            Action::AcceptTrade { by, .. } => {
                let by = by as usize;
                if let Some(from) = proposer
                    && by < seats
                    && by != from
                {
                    let (gave, took) = moved(from);
                    out.deals.push(Deal {
                        seat: from,
                        with: by,
                        turn: turn + 1,
                        gave,
                        took,
                    });
                }
            }
            Action::Trade { .. } if actor < seats => {
                let (gave, took) = moved(actor);
                let paid: u32 = gave.iter().map(|n| u32::from(*n)).sum();
                out.deals.push(Deal {
                    seat: actor,
                    // Four for one is the only rate the bank offers anybody.
                    with: if paid >= 4 {
                        Trades::BANK
                    } else {
                        Trades::PORT
                    },
                    turn: turn + 1,
                    gave,
                    took,
                });
            }
            Action::EndTurn if playing => turn += 1,
            _ => {}
        }
        playing |= matches!(state.phase, Phase::PreRoll);
    }
    out
}

/// What the board itself dealt, against what an average board deals.
///
/// The discs are a fixed set (`DISCS`) laid on a fixed set of hexes, so "an
/// average board" is not a simulation: it is the mean pips of a disc times the
/// hexes a resource has. Every disc lands somewhere, so the pips across the
/// five resources always add to the same total and the card is a pure
/// redistribution. The question it answers is which resource got the good
/// numbers this time.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Board {
    /// Hexes of each resource, which is fixed by the tile set.
    pub hexes: [u32; 5],
    /// Pips actually laid on them.
    pub pips: [u32; 5],
    /// Mean pips on a disc, over the whole set. The expectation per hex.
    pub mean: f64,
    /// Per port kind: index 0 is the generic three to one, the rest are the two
    /// to ones in resource order.
    pub ports: [PortLand; PORT_KINDS],
    /// Pairs of neighbouring hexes that make the same resource, and how many a
    /// random deal would be expected to produce.
    ///
    /// Clumping is what the pip totals cannot see: two boards can owe every
    /// resource the same pips and play completely differently if one of them has
    /// all its ore in a corner. The expectation is exact rather than simulated,
    /// which the shape of the problem allows: the adjacency graph is fixed, so it
    /// is the number of neighbouring pairs times the chance that two tiles drawn
    /// from the set without replacement match.
    pub same: u32,
    pub same_expected: f64,
    /// Whether a six sits next to an eight. Some rule sets forbid it outright;
    /// this one deals as it deals, and it is worth knowing which happened.
    pub reds_touch: bool,
    /// The best intersection on the board: pips on the hexes it touches, and the
    /// numbers themselves, best first.
    pub best: u32,
    pub best_numbers: Vec<u8>,
    /// Intersections whose pips are at least [`Board::RICH`].
    pub rich: u32,
    /// Mean pips over every intersection touching land, which is what "the best
    /// intersection" has to be read against.
    pub spot_mean: f64,
}

/// The land a port kind can be built on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PortLand {
    /// Intersections carrying this port.
    pub spots: u32,
    /// Producing hexes those intersections touch, counted once per touch: a
    /// hex two of them share is two, because either could take it.
    pub touching: u32,
    /// Pips on those hexes, counted the same way.
    pub pips: u32,
}

impl Board {
    /// Pips at an intersection worth calling rich.
    ///
    /// Ten is the round number a player uses: three hexes averaging better than
    /// three pips each, which is a placement worth planning a game around.
    pub const RICH: u32 = 10;

    /// What a random deal would have put on this resource's hexes.
    pub fn expected(&self, res: usize) -> f64 {
        f64::from(self.hexes[res]) * self.mean
    }

    /// And on the land a port kind sits on.
    pub fn port_expected(&self, kind: usize) -> f64 {
        f64::from(self.ports[kind].touching) * self.mean
    }
}

use carranta_core::state::PORT_KINDS;

/// Read the board off the opening position, before anything is built on it.
fn board_of(saved: &Saved) -> Board {
    use carranta_core::state::DISCS;
    use carranta_core::topology::{HEX_COUNT, hex_vertices};

    let state = crate::game::Session::opening(saved.seats, saved.seed, saved.mode);
    let mut b = Board {
        mean: DISCS.iter().map(|n| f64::from(ways(*n))).sum::<f64>() / DISCS.len() as f64,
        ..Board::default()
    };
    for h in 0..HEX_COUNT {
        let Some(res) = state.terrain[h].yields() else {
            continue; // the desert carries no disc and produces nothing
        };
        b.hexes[res as usize] += 1;
        b.pips[res as usize] += ways(state.number[h]);
    }
    // Clumping: neighbouring hexes making the same thing, against the number a
    // random deal would be expected to produce. Both halves are exact. The graph
    // is fixed, so the pairs can be counted; and for a shuffled set of tiles, the
    // chance any given pair matches is the chance two tiles drawn without
    // replacement are the same terrain, which is a sum over the set.
    let mut pairs = 0u32;
    for a in 0..HEX_COUNT as u8 {
        for c in a + 1..HEX_COUNT as u8 {
            // Two hexes are neighbours when they share an edge, which is two of
            // the six corners.
            if (hex_vertices(a) & hex_vertices(c)).count_ones() != 2 {
                continue;
            }
            pairs += 1;
            let (x, y) = (state.terrain[a as usize], state.terrain[c as usize]);
            if x == y && x.yields().is_some() {
                b.same += 1;
            }
        }
    }
    let mut kinds = [0u32; 5];
    for h in 0..HEX_COUNT {
        if let Some(res) = state.terrain[h].yields() {
            kinds[res as usize] += 1;
        }
    }
    let tiles = f64::from(HEX_COUNT as u32);
    let matching: f64 = kinds
        .iter()
        .map(|n| f64::from(*n) * f64::from(n.saturating_sub(1)))
        .sum();
    b.same_expected = f64::from(pairs) * matching / (tiles * (tiles - 1.0));
    b.reds_touch = carranta_core::state::red_numbers_touch(&state.number);

    // Every intersection, so the best one can be read against the ordinary one.
    let mut spots = 0u32;
    let mut total = 0u32;
    for v in 0..carranta_core::topology::VERTEX_COUNT as u8 {
        let bit = carranta_core::topology::vertex_bit(v);
        let mut pips = 0u32;
        let mut numbers = Vec::new();
        for h in 0..HEX_COUNT {
            if hex_vertices(h as u8) & bit == 0 || state.terrain[h].yields().is_none() {
                continue;
            }
            pips += ways(state.number[h]);
            numbers.push(state.number[h]);
        }
        if numbers.is_empty() {
            continue; // a corner of the sea and the desert only
        }
        spots += 1;
        total += pips;
        if pips >= Board::RICH {
            b.rich += 1;
        }
        if pips > b.best {
            b.best = pips;
            numbers.sort_unstable_by(|a, c| ways(*c).cmp(&ways(*a)).then(a.cmp(c)));
            b.best_numbers = numbers;
        }
    }
    b.spot_mean = if spots == 0 {
        0.0
    } else {
        f64::from(total) / f64::from(spots)
    };

    for (kind, land) in b.ports.iter_mut().enumerate() {
        let spots = state.ports[kind];
        land.spots = spots.count_ones();
        for h in 0..HEX_COUNT {
            if state.terrain[h].yields().is_none() {
                continue;
            }
            // Once per intersection that touches it: a hex two port spots share
            // is worth counting twice, since either could have taken it.
            let on = (spots & hex_vertices(h as u8)).count_ones();
            land.touching += on;
            land.pips += on * ways(state.number[h]);
        }
    }
    b
}

/// What an opening placement bought, before anybody had a turn.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Opening {
    /// Dots on every number the two settlements touch, by resource. The
    /// standard measure of how much production a placement buys, split by what
    /// it buys.
    pub pips: [u32; 5],
    /// Cards a turn at fair odds, by resource, which is the same figure in the
    /// unit somebody plays in. A pip is a thirty-sixth of a card.
    pub per_turn: [f64; 5],
    /// Every number the placement sits on, ascending, a hex at a time. Two
    /// settlements on the same number is that number twice.
    pub numbers: Vec<u8>,
    /// Ports it sits on. `None` is the generic three to one; `Some(r)` is the
    /// two to one for that resource.
    pub ports: Vec<Option<usize>>,
    /// The chance that a roll pays this placement anything at all.
    ///
    /// The distinct numbers it touches, weighted by how often each comes up.
    /// Pips say how much a placement collects and this says how often it
    /// collects: eight pips on one number and eight spread over three are the
    /// same production and a very different game, and only this tells them
    /// apart.
    pub coverage: f64,
}

/// Read every opening off the board the moment the setup ends.
fn openings_of(saved: &Saved) -> [Opening; MAX_PLAYERS] {
    use carranta_core::state::{PORT_KINDS, Phase};
    use carranta_core::topology::{HEX_COUNT, hex_vertices};

    let mut state = crate::game::Session::opening(saved.seats, saved.seed, saved.mode);
    for step in &saved.moves {
        let Step::Move(action) = *step else { continue };
        if state.apply(action).is_err() {
            break;
        }
        // The line every other reading of this game draws: setup ends the first
        // time play reaches a pre-roll phase.
        if matches!(state.phase, Phase::PreRoll) {
            break;
        }
    }

    let seats = state.players as usize;
    let owed = production::expectation(&state);
    let mut out: [Opening; MAX_PLAYERS] = Default::default();
    for (p, o) in out.iter_mut().enumerate().take(seats) {
        o.per_turn = owed[p];
        for h in 0..HEX_COUNT {
            let n = state.number[h];
            if !(2..=12).contains(&n) {
                continue; // the desert carries no disc
            }
            let Some(res) = state.terrain[h].yields() else {
                continue;
            };
            let on = (state.settlements[p] & hex_vertices(h as u8)).count_ones();
            if on == 0 {
                continue;
            }
            o.pips[res as usize] += ways(n) * on;
            for _ in 0..on {
                o.numbers.push(n);
            }
        }
        o.numbers.sort_unstable();
        for kind in 0..PORT_KINDS {
            let on = (state.settlements[p] & state.ports[kind]).count_ones();
            for _ in 0..on {
                // Index 0 is the generic three to one; the rest are one per
                // resource, in resource order.
                o.ports.push(kind.checked_sub(1));
            }
        }
        // Distinct, because a number that pays twice still only comes up as
        // often as it comes up.
        let mut seen = o.numbers.clone();
        seen.dedup();
        o.coverage = seen.iter().map(|n| f64::from(ways(*n)) / 36.0).sum();
    }
    out
}

/// How many ways two dice make `n`: the dots under a disc.
fn ways(n: u8) -> u32 {
    6u32.saturating_sub((i32::from(n) - 7).unsigned_abs())
}

/// One turn of the game./// One turn of the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Turn {
    /// Whose turn it was.
    pub seat: usize,
    /// Decisions taken while it was theirs, everybody's included: a discard, an
    /// accepted offer and a robbery all happen inside somebody's turn, and the
    /// length of a turn is how much happened in it.
    pub actions: u32,
    /// How long it took, in milliseconds, or nought on a game saved before the
    /// clock was written down.
    ///
    /// Wall-clock, so it is the turn holder's thinking time plus everybody
    /// else's answers to it. Time spent waiting on a decision inside somebody's
    /// turn is time that turn took, whoever was being waited on.
    pub millis: u32,
}

/// Cut a game into its turns.
///
/// A turn is what falls between two `EndTurn`s, and the setup placements are
/// not one: they are dealt before anybody has a turn to take, so counting them
/// would hand the first seat a turn several times the size of any other.
///
/// Read off the saved game rather than the record, because the record has no
/// clock: `carranta-record` is about what happened and this is also about when.
/// Replaying the moves from the seed rebuilds the same positions in the same
/// order, which is the property the whole format rests on, so the two agree
/// about everything they both know.
fn turns_of(saved: &Saved) -> Vec<Turn> {
    use carranta_core::action::Action;
    use carranta_core::state::Phase;

    let mut state = crate::game::Session::opening(saved.seats, saved.seed, saved.mode);
    // Either the clock is whole or there is none: a partial one would attribute
    // real seconds to the wrong turns.
    let timed = saved.times.len() == saved.moves.len();
    let mut out = Vec::new();
    let mut actions = 0u32;
    let mut millis = 0u32;
    let mut playing = false;
    let mut since = 0u32;
    for (i, step) in saved.moves.iter().enumerate() {
        let seat = state.to_act as usize;
        // What this step cost is the gap since the one before it: the time went
        // somewhere, and where it went is the turn it landed in.
        let spent = if timed {
            let at = saved.times[i];
            let spent = at.saturating_sub(since);
            since = at;
            spent
        } else {
            0
        };
        match *step {
            Step::Move(action) => {
                if state.apply(action).is_err() {
                    break;
                }
                if playing {
                    actions += 1;
                    millis += spent;
                    if action == Action::EndTurn {
                        out.push(Turn {
                            seat,
                            actions,
                            millis,
                        });
                        actions = 0;
                        millis = 0;
                    }
                }
                // Setup ends the first time play reaches a pre-roll phase, which
                // is the same line `game::analyse` draws.
                playing |= matches!(state.phase, Phase::PreRoll);
            }
            // A refusal moves nothing, so it is not one of the turn's actions.
            // It still took as long as it took, and the table waited for it.
            Step::Passed { .. } => {
                if playing {
                    millis += spent;
                }
            }
        }
    }
    out
}

/// Everything the analytics page says about one game.
pub struct Study {
    pub report: Report,
    /// Where each seat's points came from, off the final position.
    pub points: [Points; MAX_PLAYERS],
    /// The game turn by turn, in the order they were taken.
    pub turns: Vec<Turn>,
    /// Every card each seat gained or lost, by what moved it.
    pub ledger: [Ledger; MAX_PLAYERS],
    /// Production against expectation, turn by turn.
    pub series: Series,
    /// How often the board paid each seat, turn by turn.
    pub cover: Cover,
    /// Each seat's true score at the end of every turn, hidden victory point
    /// cards included, on the same clock as the series above. The last row is
    /// the result: a game ends the moment somebody reaches the target, so the
    /// chart's last point and the result table's points column are the same
    /// figures or one of them is wrong.
    pub score: Vec<[u32; MAX_PLAYERS]>,
    /// The same score as the rest of the table could see it, with the hidden
    /// victory point cards left out. What everybody was playing against.
    pub seen: Vec<[u32; MAX_PLAYERS]>,
    /// What one roll was worth to each seat at the end of every turn, in cards:
    /// the engine they had built by then, read off the board rather than off
    /// what it happened to pay.
    pub engine: Vec<[f64; MAX_PLAYERS]>,
    /// Where the robber sat and who it sat on.
    pub robber: Robber,
    /// How exposed each hand was to a seven.
    pub hands: Hands,
    /// What each seat built, what it cost, and what stopped it.
    pub built: Built,
    /// What was in the offers each seat made.
    pub asks: Asks,
    /// How long each development card waited in hand.
    pub waits: Waits,
    /// Where the clock went, by kind of decision, when there is a clock.
    pub spent: Option<Spent>,
    /// What happened and when, for the strip that anchors every other chart.
    pub events: Vec<Event>,
    /// Who traded with whom, and at what counter.
    pub trades: Trades,
    /// Development cards still in each hand at the end, by kind. With what was
    /// played, this is what was drawn: a card is drawn once and then either
    /// played or held, and a played card never goes back (R-8.10).
    pub dev_held: [[u32; 5]; MAX_PLAYERS],
    /// What each seat's opening placement bought.
    pub opening: [Opening; MAX_PLAYERS],
    /// The board itself, against an average one.
    pub board: Board,
    /// Whether this game was saved with a clock in it. Games written before
    /// there was one still read, and say nothing about time rather than saying
    /// nought.
    pub timed: bool,
    pub production: production::Report,
    pub dice: dice::GameDice,
    /// Where this game's dice sit against every other game recorded here, as a
    /// percentage from 0 to 100, or `None` until there are others to sit
    /// against. A percentile of one game is not a percentile.
    pub dice_percentile: Option<f64>,
    /// The same standing as a place out of the whole corpus, most deviant
    /// first, which is the honest form of it while the corpus is small.
    pub dice_rank: Option<(usize, usize)>,
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
    let end = log.replay().ok()?;
    let points = points_of(&end, report.players as usize);
    let dev_held =
        core::array::from_fn(|p| core::array::from_fn(|c| u32::from(end.dev_held[p][c])));
    let opening = openings_of(saved);
    let board = board_of(saved);
    let turns = turns_of(saved);
    let ledger = ledger_of(saved);
    let (series, cover, score, seen, engine, robber, hands, built) = series_of(saved);
    let trades = trades_of(saved);
    let timed = !saved.times.is_empty() && saved.times.len() == saved.moves.len();
    let production = production::analyse(&log).ok()?;
    let rolls = dice::rolls(&log);
    let this = dice::analyse_game(&rolls, SIMS, saved.seed);

    // Every other finished game, for the percentile and the seat balance.
    // Finished games only, which is a filter the corpus cannot apply for
    // itself: it counts what it is given and reports `finished` beside it,
    // rightly, since a half-played game is still evidence about the dice.
    //
    // It is not evidence about anything on this page. A game nobody won has no
    // finishing order, so it says nothing about whether going first is worth
    // anything and only enlarges the denominator, and its dice are a handful of
    // rolls whose deviation from fair is enormous by construction, so placing a
    // full game against it is not a comparison.
    let mut games = corpus::Corpus::new(corpus::Config::of(&log));
    let mut others = 0usize;
    for g in history {
        if g.id == saved.id || g.winner.is_none() {
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
        dice::Corpus::from_games(deviations.clone()).deviation_percentile(this.kl_fair) * 100.0
    });
    // Where this game stands as a place rather than as a share. A percentile of
    // six games moves twenty points when a seventh is played, and printing it to
    // the percent claims a resolution the corpus does not have; a rank claims
    // exactly what it knows.
    let dice_rank = (!deviations.is_empty()).then(|| {
        let above = deviations.iter().filter(|d| **d > this.kl_fair).count();
        (above + 1, deviations.len() + 1)
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
            let ranked: Vec<Option<usize>> = (0..seats).map(|s| rank_of(&pool, s)).collect();
            if !pool.record(&log) {
                break;
            }
            for s in 0..seats {
                movement[s] = Some(Movement {
                    before: before[s],
                    after: pool.rating(seat_player(s)),
                    games: games[s],
                    rank_before: ranked[s],
                    rank_after: rank_of(&pool, s),
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
        points,
        turns,
        ledger,
        series,
        cover,
        score,
        seen,
        engine,
        robber,
        hands,
        built,
        asks: asks_of(saved),
        waits: waits_of(saved),
        spent: spent_of(saved),
        events: events_of(saved),
        trades,
        dev_held,
        opening,
        board,
        timed,
        production,
        dice: this,
        dice_percentile,
        dice_rank,
        corpus_games: others,
        movement,
        seat_wins,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Session;
    use crate::store::{Setup, game_id};
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
            by: String::new(),
            dealt: seed,
            winner: s.winner(),
            setup: Setup::default(),
            moves: s.moves().to_vec(),
            times: s.times().to_vec(),
        }
    }

    #[test]
    fn growth_recovers_a_rate_that_really_is_compounding() {
        // A made-up engine that doubles every ten turns: the fit should say so,
        // and say it is a perfectly straight line in logs.
        let doubles: Vec<[f64; MAX_PLAYERS]> = (0..61)
            .map(|i| {
                let mut row = [0.0f64; MAX_PLAYERS];
                row[0] = 2.0f64.powf(f64::from(i) / 10.0);
                row
            })
            .collect();
        let g = growth_of(&doubles, 0).expect("sixty turns is enough to fit");
        assert!((g.doubling.expect("it grows") - 10.0).abs() < 0.2, "{g:?}");
        assert!(g.fit > 0.999, "a geometric rate is a straight line in logs");
        assert!(g.fit > g.fit_line, "and a curve against a straight one");
        assert_eq!(g.shape(), Shape::Compounding);
        assert!(g.believable());
        assert!(
            g.late > g.early * 15.0,
            "and it ends far above where it began"
        );

        // A flat engine grows at nothing and doubles never.
        let flat: Vec<[f64; MAX_PLAYERS]> = (0..41).map(|_| [2.0; MAX_PLAYERS]).collect();
        let g = growth_of(&flat, 0).expect("forty turns is enough");
        assert!(g.per_turn.abs() < 1e-9, "{g:?}");
        assert!(g.doubling.is_none(), "flat never doubles");
        assert_eq!(g.early, g.late);
        assert_eq!(g.shape(), Shape::Flat);
        assert!(!g.believable(), "and nothing about it is worth believing");

        // An engine climbing by the same amount every turn is steady, not
        // compounding, however well its log happens to fit a line.
        let ramp: Vec<[f64; MAX_PLAYERS]> = (0..61)
            .map(|i| {
                let mut row = [0.0f64; MAX_PLAYERS];
                row[0] = 0.5 + 0.02 * f64::from(i);
                row
            })
            .collect();
        let g = growth_of(&ramp, 0).expect("sixty turns is enough");
        assert_eq!(g.shape(), Shape::Steady, "{g:?}");
        assert!(g.fit_line > 0.999, "a ramp is a straight line");
        assert!(g.per_turn > 0.0, "and it is still growing");

        // A short game declines to answer rather than answering badly.
        assert!(growth_of(&flat[..8], 0).is_none());
    }

    #[test]
    fn the_opening_and_the_coverage_chart_agree_about_the_same_board() {
        let g = played(7);
        let s = study(&g, std::slice::from_ref(&g)).expect("it studies");
        let seats = s.report.players as usize;
        // The opening card's coverage and the chart's first unblocked sample are
        // the same board read twice, so they cannot disagree. The first sample is
        // the end of the first turn, and only the seat taking it can have built
        // by then, so that one seat is allowed to be above its opening and
        // nobody is allowed to be below.
        let mut equal = 0;
        for p in 0..seats {
            let opening = s.opening[p].coverage;
            let first = s.cover.open[0][p];
            assert!(
                first >= opening - 1e-12,
                "seat {p}: coverage fell from {opening} to {first} without a \
                 building coming down"
            );
            if (first - opening).abs() < 1e-12 {
                equal += 1;
            }
        }
        assert!(
            equal >= seats - 1,
            "only the seat that took the first turn can have moved"
        );
    }

    #[test]
    fn a_real_engine_grows_over_a_real_game() {
        let g = played(3);
        let s = study(&g, std::slice::from_ref(&g)).expect("it studies");
        for p in 0..s.report.players as usize {
            let growth = growth_of(&s.engine, p).expect("a full game fits");
            // The expectation ignores the robber, and buildings only ever add
            // production, so an engine can hold steady and cannot shrink. A
            // seat that never builds is flat, which is a rating and not a bug.
            // The expectation ignores the robber and buildings only ever add
            // production, so a seat that built nothing comes out flat rather
            // than falling. Which is a rating, not a bug.
            assert!(growth.per_turn >= 0.0, "seat {p}: {growth:?}");
            assert!(growth.late >= growth.early, "an engine cannot shrink");
            assert!((0.0..=1.0).contains(&growth.fit));
            assert_eq!(
                growth.doubling.is_some(),
                growth.per_turn > 0.0,
                "only a growing engine doubles"
            );
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
        assert!(
            refused > 0,
            "some game in the range had an offer turned down"
        );
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
            let best = deltas.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
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
    fn what_scored_adds_up_to_the_score() {
        // The whole claim the result table makes. If these ever disagree, the
        // page is showing a decomposition of something else.
        for seed in 0..10u64 {
            let g = played(seed);
            let s = study(&g, std::slice::from_ref(&g)).expect("it studies");
            for seat in 0..s.report.players as usize {
                assert_eq!(
                    s.points[seat].total(),
                    s.report.vp[seat],
                    "seed {seed}, seat {seat}: {:?}",
                    s.points[seat]
                );
            }
            // And exactly one seat holds each tile, or nobody does.
            let with = |f: fn(&Points) -> Scored| {
                (0..s.report.players as usize)
                    .filter(|&i| f(&s.points[i]).held > 0)
                    .count()
            };
            assert!(with(|p| p.longest_road) <= 1, "one longest road at most");
            assert!(
                with(|p| p.largest_militia) <= 1,
                "one largest militia at most"
            );
        }
    }

    #[test]
    fn a_settlement_upgraded_is_not_a_settlement_any_more() {
        // Why the breakdown is read off the final position: the built count
        // keeps a settlement that has been a city for eighty turns, and scoring
        // it would put the table's arithmetic out by one per upgrade.
        let mut found = false;
        for seed in 0..10u64 {
            let g = played(seed);
            let s = study(&g, std::slice::from_ref(&g)).expect("it studies");
            for seat in 0..s.report.players as usize {
                let built = s.report.builds[seat].settlements;
                let standing = s.points[seat].settlements.held;
                let cities = s.points[seat].cities.held;
                if cities > 0 {
                    found = true;
                    assert_eq!(
                        standing + cities,
                        built,
                        "seed {seed}, seat {seat}: every city was a settlement first"
                    );
                }
            }
        }
        assert!(found, "some game in the range built a city");
    }

    #[test]
    fn every_card_is_accounted_for() {
        // The whole claim the ledger makes, and the reason it is read off the
        // hands rather than off the rules: if any way of moving a card is
        // missing, what came in stops matching what went out plus what is left.
        for seed in 0..10u64 {
            let g = played(seed);
            let s = study(&g, std::slice::from_ref(&g)).expect("it studies");
            let end = crate::game::Session::replay(g.seats, g.seed, g.mode, &g.moves)
                .expect("it replays");
            for seat in 0..s.report.players as usize {
                let led = s.ledger[seat];
                assert!(
                    led.balances(),
                    "seed {seed}, seat {seat}: {led:?} does not balance"
                );
                // And what it says is left really is what is in the hand.
                assert_eq!(led.held, end.hand_size(seat), "seed {seed}, seat {seat}");
                // The categories are the rows, and they are the whole of it.
                let (mut up, mut down) = (0, 0);
                for (_, n, incoming) in led.rows() {
                    if incoming { up += n } else { down += n }
                }
                assert_eq!(up, led.came_in());
                assert_eq!(down, led.went_out());
            }
            // A game that was played moved cards by more than one route.
            let ways = |f: fn(&Ledger) -> u32| {
                (0..s.report.players as usize)
                    .map(|i| f(&s.ledger[i]))
                    .sum::<u32>()
            };
            assert!(ways(|l| l.production) > 0, "seed {seed}: the board paid");
            assert!(ways(|l| l.built) > 0, "seed {seed}: somebody built");
        }
    }

    #[test]
    fn the_curves_end_where_the_ledger_says_they_should() {
        for seed in 0..6u64 {
            let g = played(seed);
            let s = study(&g, std::slice::from_ref(&g)).expect("it studies");
            let seats = s.report.players as usize;
            // A point per turn, on the same boundaries the turns table uses,
            // and one more for the winning turn, which never ends.
            assert!(
                (s.turns.len()..=s.turns.len() + 1).contains(&s.series.turns()),
                "seed {seed}: {} points for {} turns",
                s.series.turns(),
                s.turns.len()
            );
            for seat in 0..seats {
                // Cumulative, so neither line ever falls.
                for pair in s.series.actual.windows(2) {
                    let (a, b) = (pair[0][seat], pair[1][seat]);
                    for res in 0..5 {
                        assert!(b[res] >= a[res], "seed {seed}: production went backwards");
                    }
                }
                for pair in s.series.expected.windows(2) {
                    let total = |row: &[[f64; 5]; MAX_PLAYERS]| row[seat].iter().sum::<f64>();
                    assert!(total(&pair[1]) >= total(&pair[0]) - 1e-9);
                }
                // And the last point is what the ledger calls production: the
                // chart and the table are drawing the same cards.
                let end: u32 = s.series.actual.last().expect("a game has turns")[seat]
                    .iter()
                    .sum();
                assert_eq!(
                    end, s.ledger[seat].production,
                    "seed {seed}, seat {seat}: the curve and the ledger disagree"
                );
                // The expectation is a real one, not a flat nothing.
                let owed: f64 = s.series.expected.last().unwrap()[seat].iter().sum();
                assert!(
                    owed > 0.0,
                    "seed {seed}, seat {seat}: nothing was ever owed"
                );
            }
        }
    }

    #[test]
    fn a_trade_is_counted_both_ways_round() {
        // Gross rather than net: two wheat for one ore is one card in and two
        // out, and a hand size alone would call it one card out.
        let mut moved = 0;
        for seed in 0..8u64 {
            let g = played(seed);
            let s = study(&g, std::slice::from_ref(&g)).expect("it studies");
            for seat in 0..s.report.players as usize {
                let led = s.ledger[seat];
                // Nobody trades one way only across a whole game: every trade
                // gives as well as takes.
                assert_eq!(
                    led.traded_in > 0,
                    led.traded_out > 0,
                    "seed {seed}, seat {seat}: {led:?}"
                );
                moved += led.traded_in;
            }
        }
        assert!(moved > 0, "some game in the range traded");
    }

    #[test]
    fn every_trade_has_two_ends() {
        for seed in 0..8u64 {
            let g = played(seed);
            let s = study(&g, std::slice::from_ref(&g)).expect("it studies");
            let seats = s.report.players as usize;
            let tr = &s.trades;
            for d in &tr.deals {
                // Every deal names a seat and somebody else, and moves cards
                // both ways: a trade that only took would not be one.
                assert!(d.seat < seats, "seed {seed}");
                assert_ne!(d.seat, d.with, "seed {seed}: nobody trades with themselves");
                assert!(d.gave.iter().any(|n| *n > 0), "seed {seed}");
                assert!(d.took.iter().any(|n| *n > 0), "seed {seed}");
                assert!(d.turn > 0, "seed {seed}");
            }
            for a in 0..seats {
                assert_eq!(tr.between(a, a), 0, "seed {seed}");
                for c in 0..seats {
                    assert_eq!(tr.between(a, c), tr.between(c, a), "seed {seed}");
                }
                // The counter is either the bank or a port and never both, and
                // together they are what the market table calls supply trades.
                assert_eq!(
                    tr.against(a, Trades::BANK) + tr.against(a, Trades::PORT),
                    s.report.supply_trades[a],
                    "seed {seed}, seat {a}: the counters do not add up"
                );
            }
            // A player trade is counted for both sides in the report, so the
            // pairs here are exactly half of that column.
            let pairs: u32 = (0..seats)
                .flat_map(|a| (0..seats).map(move |c| (a, c)))
                .map(|(a, c)| tr.between(a, c))
                .sum();
            let counted: u32 = s.report.trades_completed[..seats].iter().sum();
            assert_eq!(pairs, counted, "seed {seed}");
            // And the ends add to twice the deals, since each has two.
            let ends: usize = (0..seats)
                .chain([Trades::BANK, Trades::PORT])
                .map(|p| tr.ends(p))
                .sum();
            assert_eq!(ends, tr.deals.len() * 2, "seed {seed}");
        }
    }

    #[test]
    fn a_card_drawn_was_played_or_is_still_held() {
        // What the development table claims: bought is played plus held, per
        // kind, because a played card never goes back to the deck (R-8.10).
        for seed in 0..8u64 {
            let g = played(seed);
            let s = study(&g, std::slice::from_ref(&g)).expect("it studies");
            for seat in 0..s.report.players as usize {
                let drawn: u32 = (0..5)
                    .map(|c| s.report.dev_played[seat][c] + s.dev_held[seat][c])
                    .sum();
                assert_eq!(
                    drawn, s.report.dev_bought[seat],
                    "seed {seed}, seat {seat}: cards leaked"
                );
                // A victory point card is never played, so all of them are held.
                assert_eq!(s.report.dev_played[seat][1], 0, "seed {seed}");
            }
        }
    }

    #[test]
    fn an_opening_is_what_the_board_gave_it() {
        for seed in 0..6u64 {
            let g = played(seed);
            let s = study(&g, std::slice::from_ref(&g)).expect("it studies");
            for seat in 0..s.report.players as usize {
                let o = &s.opening[seat];
                // Pips split by resource are the pips the report counts whole.
                assert_eq!(
                    o.pips.iter().sum::<u32>(),
                    s.report.opening[seat].pips,
                    "seed {seed}, seat {seat}"
                );
                // A resource is touched exactly when the board owes it, which
                // is the figure the report calls diversity.
                let touched = o.pips.iter().filter(|n| **n > 0).count() as u32;
                assert_eq!(touched, s.report.opening[seat].diversity, "seed {seed}");
                for res in 0..5 {
                    assert_eq!(o.pips[res] > 0, o.per_turn[res] > 0.0, "seed {seed}");
                    // A pip is a thirty-sixth of a card, per settlement.
                    let owed = f64::from(o.pips[res]) / 36.0;
                    assert!((o.per_turn[res] - owed).abs() < 1e-9, "seed {seed}");
                }
                // Two settlements touch at most three hexes each, and the pips
                // on them are what the numbers say.
                assert!(!o.numbers.is_empty() && o.numbers.len() <= 6, "seed {seed}");
                assert!(o.numbers.windows(2).all(|w| w[0] <= w[1]), "in order");
                let from_numbers: u32 = o.numbers.iter().map(|n| ways(*n)).sum();
                assert_eq!(from_numbers, o.pips.iter().sum::<u32>(), "seed {seed}");
                // Coverage is a probability, and it is the distinct numbers
                // rather than all of them: a number twice is one number.
                assert!((0.0..=1.0).contains(&o.coverage), "seed {seed}");
                let mut distinct = o.numbers.clone();
                distinct.dedup();
                let want: f64 = distinct.iter().map(|n| f64::from(ways(*n)) / 36.0).sum();
                assert!((o.coverage - want).abs() < 1e-12, "seed {seed}");
                assert_eq!(
                    o.ports.len() as u32,
                    s.report.opening[seat].ports,
                    "seed {seed}"
                );
            }
        }
    }

    #[test]
    fn eight_pips_on_one_number_is_not_eight_pips_on_three() {
        // What coverage is for. Two openings can buy the same production and
        // collect it on a wholly different number of rolls, and pips alone
        // cannot tell them apart.
        let history: Vec<Saved> = (0..12u64).map(played).collect();
        let mut seen = false;
        for g in &history {
            let s = study(g, &history).expect("it studies");
            let seats = s.report.players as usize;
            for a in 0..seats {
                for c in 0..seats {
                    let (x, y) = (&s.opening[a], &s.opening[c]);
                    if x.pips.iter().sum::<u32>() == y.pips.iter().sum::<u32>()
                        && (x.coverage - y.coverage).abs() > 0.02
                    {
                        seen = true;
                    }
                }
            }
        }
        assert!(
            seen,
            "two openings of equal pips and unequal coverage exist"
        );
    }

    #[test]
    fn the_turns_are_the_game_after_the_setup() {
        for seed in 0..6u64 {
            let g = played(seed);
            let s = study(&g, std::slice::from_ref(&g)).expect("it studies");
            let seats = s.report.players as usize;
            // One turn per `EndTurn`, which is what the report counts as well.
            assert_eq!(s.turns.len() as u32, s.report.turns, "seed {seed}");
            let total: u32 = s.turns.iter().map(|t| t.actions).sum();
            // Every action in a turn is an action in the game, and the setup
            // placements are the ones left over.
            assert!(total > 0 && total < s.report.actions, "seed {seed}");
            for t in &s.turns {
                assert!(t.seat < seats, "seed {seed}: a turn belongs to a seat");
                // A turn contains at least its own end.
                assert!(t.actions > 0, "seed {seed}: no empty turns");
            }
            // And play goes round the table, which is what makes a bar of them
            // readable as a sequence rather than a heap.
            for pair in s.turns.windows(2) {
                assert_eq!(
                    pair[1].seat,
                    (pair[0].seat + 1) % seats,
                    "seed {seed}: turns go round in seat order"
                );
            }
        }
    }

    #[test]
    fn the_order_games_were_played_in_is_the_order_they_are_rated_in() {
        // A rating is a function of every game before it, so the history's
        // order is load-bearing rather than cosmetic. Read at the end of the
        // list, a player has every earlier game behind them.
        let history: Vec<Saved> = (30..34u64).map(played).collect();
        let first = study(&history[0], &history).expect("it studies");
        let last = study(&history[3], &history).expect("it studies");
        assert_eq!(
            first.movement[0].unwrap().games,
            0,
            "nothing before the first"
        );
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
        // The wildest game in the set is above all of them, which is exactly
        // what a share of one would have hidden. The calmest is not pinned to
        // nought any more: the deviation is bias-corrected and floored there, so
        // several games in a fair set genuinely tie at nought, and a tie is not
        // a bottom place.
        assert_eq!(seen.last().unwrap().1, 100.0);
        assert!(
            seen.first().unwrap().1 < 100.0,
            "not everything is the worst"
        );
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
        assert!(
            (0.0..=100.0).contains(&p),
            "a percentile is a percentile: {p}"
        );
    }
}
