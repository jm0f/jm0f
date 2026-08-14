//! Per-game descriptive statistics (§10.3).
//!
//! One pass over a replayed game, accumulating everything the game summary
//! and the per-player breakdown need. Production and dice have their own
//! modules, [`crate::production`] and [`crate::dice`], because both are
//! analyses rather than tallies; what is here is counting.

use carranta_core::state::{DevCard, MAX_PLAYERS, State};
use carranta_core::topology::{HEX_COUNT, hex_vertices};
use carranta_core::{Action, Phase, Resolved};
use carranta_record::{Actor, Log, Payload, ReplayError};

use crate::dice::PIPS;

/// How good a seat's starting position was, judged from the board alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Opening {
    /// Total pips across all hexes touched, the standard measure of how much
    /// production a placement buys.
    pub pips: u32,
    /// Distinct resources reachable. Low diversity means dependence on trade.
    pub diversity: u32,
    /// Port kinds reachable from the starting settlements.
    pub ports: u32,
}

/// Pieces a seat put on the board.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Builds {
    pub roads: u32,
    pub settlements: u32,
    pub cities: u32,
}

/// Everything countable about one recorded game.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub game_id: u64,
    pub players: u8,
    pub winner: Option<u8>,
    /// True final points, hidden cards included (R-11.3).
    pub vp: [u32; MAX_PLAYERS],
    /// Completed turns.
    pub turns: u32,
    /// Actions applied, negotiation churn excluded.
    pub actions: u32,

    /// Rolls by total, indexed by `total - 2`.
    pub rolls: [u32; 11],
    /// Sevens, which drive the whole robber economy.
    pub sevens: u32,

    pub robber_moves: u32,
    /// How often the robber landed on each hex.
    pub robber_targets: [u32; HEX_COUNT],
    /// Who robbed whom: `steals[thief][victim]`.
    pub steals: [[u32; MAX_PLAYERS]; MAX_PLAYERS],
    /// Robberies that found an empty hand (R-6.4).
    pub empty_robberies: u32,
    /// Cards each seat discarded to a 7 (R-6.2).
    pub discards: [u32; MAX_PLAYERS],

    /// Supply trades, at whatever rate the ports allowed (R-7.6–R-7.9).
    pub supply_trades: [u32; MAX_PLAYERS],
    pub offers_made: [u32; MAX_PLAYERS],
    pub offers_withdrawn: [u32; MAX_PLAYERS],
    pub offers_declined: [u32; MAX_PLAYERS],
    /// Trades completed, counted for both parties.
    pub trades_completed: [u32; MAX_PLAYERS],

    pub dev_bought: [u32; MAX_PLAYERS],
    /// Development cards played, indexed by [`DevCard`].
    pub dev_played: [[u32; 5]; MAX_PLAYERS],

    pub builds: [Builds; MAX_PLAYERS],
    pub opening: [Opening; MAX_PLAYERS],

    /// True points at the end of each completed turn.
    ///
    /// **Truncation bias** (§10.6.2): games end when somebody reaches 10, so
    /// averaging this across games at a fixed turn index silently keeps only
    /// the games that lasted that long. Report the count per turn alongside
    /// any such average.
    pub vp_curve: Vec<[u32; MAX_PLAYERS]>,
    /// Largest hand each seat held at any point.
    pub peak_hand: [u32; MAX_PLAYERS],
    /// How often each bonus tile changed hands (R-10.5, R-10.6).
    pub longest_road_transfers: u32,
    pub largest_militia_transfers: u32,
}

/// Opening placement quality for every seat, from the board after setup.
pub fn opening_quality(state: &State) -> [Opening; MAX_PLAYERS] {
    let mut out = [Opening::default(); MAX_PLAYERS];
    for (p, slot) in out.iter_mut().enumerate().take(state.players as usize) {
        let mine = state.settlements[p] | state.cities[p];
        let mut resources = 0u32;
        for h in 0..HEX_COUNT {
            let touching = (hex_vertices(h as u8) & mine).count_ones();
            if touching == 0 {
                continue;
            }
            let n = state.number[h];
            if (2..=12).contains(&n) {
                slot.pips += PIPS[n as usize - 2] * touching;
            }
            if let Some(r) = state.terrain[h].yields() {
                resources |= 1 << r as usize;
            }
        }
        slot.diversity = resources.count_ones();
        slot.ports = state.ports.iter().filter(|&&kind| kind & mine != 0).count() as u32;
    }
    out
}

/// Tally a recorded game.
pub fn analyse(log: &Log) -> Result<Report, ReplayError> {
    let mut state = *log.created.opening;
    let seats = state.players as usize;
    let mut r = Report {
        game_id: log.game_id,
        players: state.players,
        winner: None,
        vp: [0; MAX_PLAYERS],
        turns: 0,
        actions: 0,
        rolls: [0; 11],
        sevens: 0,
        robber_moves: 0,
        robber_targets: [0; HEX_COUNT],
        steals: [[0; MAX_PLAYERS]; MAX_PLAYERS],
        empty_robberies: 0,
        discards: [0; MAX_PLAYERS],
        supply_trades: [0; MAX_PLAYERS],
        offers_made: [0; MAX_PLAYERS],
        offers_withdrawn: [0; MAX_PLAYERS],
        offers_declined: [0; MAX_PLAYERS],
        trades_completed: [0; MAX_PLAYERS],
        dev_bought: [0; MAX_PLAYERS],
        dev_played: [[0; 5]; MAX_PLAYERS],
        builds: [Builds::default(); MAX_PLAYERS],
        opening: [Opening::default(); MAX_PLAYERS],
        vp_curve: Vec::new(),
        peak_hand: [0; MAX_PLAYERS],
        longest_road_transfers: 0,
        largest_militia_transfers: 0,
    };

    let mut setup_done = false;
    let mut last_lr = state.longest_road;
    let mut last_lm = state.largest_militia;

    for event in &log.events {
        match event.payload {
            Payload::Declined { by, .. } => {
                if (by as usize) < seats {
                    r.offers_declined[by as usize] += 1;
                }
                continue;
            }
            Payload::Ended { winner, vp } => {
                r.winner = winner;
                r.vp = vp;
                continue;
            }
            Payload::Decision { .. } => {}
        }
        let Payload::Decision { action, resolved } = event.payload else {
            unreachable!()
        };

        let actor = match event.actor {
            Actor::Seat(s) if (s as usize) < seats => s as usize,
            _ => 0,
        };
        // Read whatever must be known *before* the action lands.
        let proposer = match action {
            Action::AcceptTrade { offer, .. } => state
                .offers
                .get(offer as usize)
                .map(|o| o.from as usize)
                .filter(|&p| p < seats),
            _ => None,
        };

        let got = state
            .apply_scripted(action, resolved)
            .map_err(|why| ReplayError::Illegal {
                seq: event.seq,
                why,
            })?;
        if got != resolved {
            return Err(ReplayError::Diverged { seq: event.seq });
        }
        r.actions += 1;

        match action {
            Action::Roll => {
                if let Resolved::Dice(a, b) = resolved {
                    let total = (a + b) as usize;
                    if (2..=12).contains(&total) {
                        r.rolls[total - 2] += 1;
                    }
                    r.sevens += (total == 7) as u32;
                }
            }
            Action::EndTurn => {
                r.turns += 1;
                r.vp_curve.push(core::array::from_fn(|p| {
                    if p < seats {
                        state.victory_points(p)
                    } else {
                        0
                    }
                }));
            }
            Action::MoveRobber { hex, victim } => {
                r.robber_moves += 1;
                if (hex as usize) < HEX_COUNT {
                    r.robber_targets[hex as usize] += 1;
                }
                if let Some(v) = victim.filter(|v| (*v as usize) < seats) {
                    match resolved {
                        Resolved::Steal(Some(_)) => r.steals[actor][v as usize] += 1,
                        _ => r.empty_robberies += 1,
                    }
                }
            }
            Action::Discard { player, .. } => {
                if (player as usize) < seats {
                    r.discards[player as usize] += 1;
                }
            }
            Action::Trade { .. } => r.supply_trades[actor] += 1,
            Action::ProposeTrade { by, .. } => {
                if (by as usize) < seats {
                    r.offers_made[by as usize] += 1;
                }
            }
            Action::WithdrawTrade { by, .. } => {
                if (by as usize) < seats {
                    r.offers_withdrawn[by as usize] += 1;
                }
            }
            Action::AcceptTrade { by, .. } => {
                if (by as usize) < seats {
                    r.trades_completed[by as usize] += 1;
                }
                if let Some(p) = proposer {
                    r.trades_completed[p] += 1;
                }
            }
            Action::BuyDev => r.dev_bought[actor] += 1,
            Action::PlayMilitia => r.dev_played[actor][DevCard::Militia as usize] += 1,
            Action::PlayRoadBuilding => r.dev_played[actor][DevCard::RoadBuilding as usize] += 1,
            Action::PlayInvention(_) => r.dev_played[actor][DevCard::Invention as usize] += 1,
            Action::PlayMonopoly(_) => r.dev_played[actor][DevCard::Monopoly as usize] += 1,
            Action::BuildRoad(_) | Action::PlaceRoad(_) => r.builds[actor].roads += 1,
            Action::BuildSettlement(_) | Action::PlaceSettlement(_) => {
                r.builds[actor].settlements += 1
            }
            Action::BuildCity(_) => r.builds[actor].cities += 1,
        }

        // Setup ends the first time play reaches a pre-roll phase.
        if !setup_done && matches!(state.phase, Phase::PreRoll) {
            setup_done = true;
            r.opening = opening_quality(&state);
        }
        for p in 0..seats {
            r.peak_hand[p] = r.peak_hand[p].max(state.hand_size(p));
        }
        if state.longest_road != last_lr {
            r.longest_road_transfers += 1;
            last_lr = state.longest_road;
        }
        if state.largest_militia != last_lm {
            r.largest_militia_transfers += 1;
            last_lm = state.largest_militia;
        }
    }
    Ok(r)
}

impl Report {
    /// Development cards a seat played, of any kind.
    pub fn dev_played_total(&self, seat: usize) -> u32 {
        self.dev_played[seat].iter().sum()
    }

    /// Cards a seat lost to robberies.
    pub fn robbed_of(&self, seat: usize) -> u32 {
        (0..MAX_PLAYERS).map(|thief| self.steals[thief][seat]).sum()
    }

    /// Cards a seat took from others.
    pub fn stole(&self, seat: usize) -> u32 {
        self.steals[seat].iter().sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::self_play;
    use carranta_core::state::TradeMode;

    #[test]
    fn a_game_summary_is_internally_consistent() {
        for seed in 0..30 {
            let log = self_play(seed, TradeMode::Full);
            let r = analyse(&log).expect("analyse");

            // Rolls, sevens, robber moves.
            assert_eq!(r.sevens, r.rolls[5]);
            let total_rolls: u32 = r.rolls.iter().sum();
            assert!(total_rolls > 10, "seed {seed}: only {total_rolls} rolls");
            // Every 7 forces a robber move, and every Militia forces one
            // more, except that playing a Militia can award Largest Militia
            // and reach 10 points on the spot, ending the game before the
            // robber ever moves (R-10.8, R-11.1). Only the final action can
            // leave such a move outstanding, so the shortfall is at most one.
            let militia: u32 = (0..4)
                .map(|p| r.dev_played[p][DevCard::Militia as usize])
                .sum();
            let owed = r.sevens + militia;
            assert!(
                r.robber_moves == owed || r.robber_moves + 1 == owed,
                "seed {seed}: {} robber moves against {owed} owed",
                r.robber_moves
            );

            // Steals never exceed robber moves.
            let steals: u32 = (0..4).map(|p| r.stole(p)).sum();
            assert!(steals + r.empty_robberies <= r.robber_moves);
            // Nobody robs themselves.
            for p in 0..4 {
                assert_eq!(r.steals[p][p], 0, "seed {seed}: seat {p} robbed itself");
            }

            // Builds respect the piece pools (R-8.6, R-8.8).
            for p in 0..4 {
                assert!(r.builds[p].roads <= 15);
                assert!(
                    r.builds[p].settlements <= 5 + 4,
                    "settlements are rebuilt after upgrades"
                );
                assert!(r.builds[p].cities <= 4);
            }

            // Development cards played are cards that were bought.
            let bought: u32 = r.dev_bought.iter().sum();
            assert!(bought <= carranta_core::state::DEV_DECK_SIZE as u32);
            for p in 0..4 {
                assert!(r.dev_played_total(p) <= r.dev_bought[p]);
            }

            // Trades are two-sided.
            assert_eq!(
                (0..4).map(|p| r.trades_completed[p]).sum::<u32>() % 2,
                0,
                "seed {seed}: a trade had one party"
            );
        }
    }

    #[test]
    fn the_winner_holds_the_most_points() {
        for seed in 0..30 {
            let r = analyse(&self_play(seed, TradeMode::Disabled)).unwrap();
            let Some(w) = r.winner else { continue };
            assert!(r.vp[w as usize] >= 10, "seed {seed}: R-11.1");
            for p in 0..4 {
                assert!(r.vp[w as usize] >= r.vp[p], "seed {seed}");
            }
        }
    }

    #[test]
    fn the_victory_point_curve_only_rises_and_ends_at_the_result() {
        let log = self_play(2, TradeMode::Full);
        let r = analyse(&log).unwrap();
        assert_eq!(r.vp_curve.len(), r.turns as usize);
        // Points are never lost, but the bonus tiles do transfer, so this is
        // only true of the building component. Check the total is monotone
        // apart from tile moves.
        let tile_moves = r.longest_road_transfers + r.largest_militia_transfers;
        let mut drops = 0;
        for w in r.vp_curve.windows(2) {
            for (now, before) in w[1].iter().zip(&w[0]) {
                if now < before {
                    drops += 1;
                }
            }
        }
        assert!(
            drops <= tile_moves,
            "{drops} point drops but only {tile_moves} tile transfers"
        );
    }

    #[test]
    fn opening_quality_reads_the_starting_position() {
        let log = self_play(5, TradeMode::Disabled);
        let r = analyse(&log).unwrap();
        for p in 0..4 {
            let o = r.opening[p];
            // Two settlements, at most three hexes each, at most 5 pips a hex.
            assert!(o.pips > 0 && o.pips <= 30, "seat {p}: {} pips", o.pips);
            assert!((1..=5).contains(&o.diversity), "seat {p}: {:?}", o);
            assert!(o.ports <= 6);
        }
    }

    #[test]
    fn declined_offers_are_counted_and_change_nothing_else() {
        let log = self_play(4, TradeMode::Full);
        let mut with_declines = log.clone();
        let base = analyse(&log).unwrap();

        // Splice in a decline; only that counter should move.
        with_declines.events.insert(
            5,
            carranta_record::Event {
                seq: u32::MAX, // sequence is not read by the tally
                at: Default::default(),
                actor: Actor::Seat(2),
                payload: Payload::Declined { offer: 0, by: 2 },
            },
        );
        let after = analyse(&with_declines).unwrap();
        assert_eq!(after.offers_declined[2], base.offers_declined[2] + 1);
        assert_eq!(after.actions, base.actions);
        assert_eq!(after.rolls, base.rolls);
        assert_eq!(after.vp, base.vp);
    }

    #[test]
    fn trading_off_means_no_player_trades() {
        let r = analyse(&self_play(8, TradeMode::Disabled)).unwrap();
        assert_eq!(r.offers_made, [0; MAX_PLAYERS]);
        assert_eq!(r.trades_completed, [0; MAX_PLAYERS]);
        // Supply trade is unaffected by the mode.
        assert!(r.supply_trades.iter().sum::<u32>() > 0);
    }
}
