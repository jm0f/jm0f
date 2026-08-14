//! Replay fidelity and redaction, exercised through the public API only.

use carranta_bot::{Heuristic, Policy};
use carranta_core::action::Resolved;
use carranta_core::state::{DevCard, TradeMode};
use carranta_core::{Action, Phase, State};
use carranta_record::{
    Log, Payload, Recorder, ReplayError, SeatId, Viewer, fog,
    fog::{SeenResolved, SeenWhat, project},
};

/// Record a self-play game, stopping after at most `cap` actions.
fn record(seed: u64, cap: usize, mode: TradeMode) -> (Log, State) {
    let mut bots: Vec<Heuristic> = (0..4).map(|i| Heuristic::new(seed * 7 + i)).collect();
    let opening = State::new(4, seed).with_trade_mode(mode);
    let seats = (0..4)
        .map(|i| SeatId::agent(1000 + i as u64, "heuristic", 1))
        .collect();
    let mut rec = Recorder::new(seed, seed, opening, seats);

    let mut buf = Vec::new();
    for _ in 0..cap {
        if matches!(rec.state().phase, Phase::GameOver { .. }) {
            break;
        }
        rec.state().legal_into(&mut buf);
        if buf.is_empty() {
            break;
        }
        let seat = rec.state().decider() as usize;
        let action = bots[seat].choose(rec.state(), &buf);
        if rec.apply(action).is_err() {
            break;
        }
    }
    let final_state = *rec.state();
    let winner = match final_state.phase {
        Phase::GameOver { winner } => Some(winner),
        _ => None,
    };
    (rec.finish_into(winner), final_state)
}

#[test]
fn replay_reproduces_every_recorded_game() {
    for seed in 0..40 {
        let (log, want) = record(seed, 20_000, TradeMode::Full);
        let got = log.replay().expect("replay");
        assert!(
            got.same_game_as(&want),
            "seed {seed} replayed to a different position"
        );
    }
}

#[test]
fn verify_checks_every_snapshot() {
    for seed in 0..40 {
        let (log, _) = record(seed, 20_000, TradeMode::Full);
        assert!(!log.snapshots.is_empty(), "seed {seed} took no snapshots");
        log.verify()
            .unwrap_or_else(|e| panic!("seed {seed}: {e:?}"));
    }
}

#[test]
fn seeking_from_a_snapshot_agrees_with_folding_from_the_start() {
    let (log, _) = record(7, 20_000, TradeMode::Full);
    let last = log.events.last().unwrap().seq;
    // Deliberately includes points just before, at, and after a snapshot.
    for seq in (0..=last).step_by(13) {
        let sought = log.replay_to(seq).expect("seek");

        let mut folded = *log.created.opening;
        for e in log.events.iter().take_while(|e| e.seq <= seq) {
            if let Payload::Decision { action, resolved } = e.payload {
                folded.apply_scripted(action, resolved).expect("fold");
            }
        }
        assert!(sought.same_game_as(&folded), "seek to {seq} disagreed");
    }
}

#[test]
fn a_tampered_outcome_fails_loudly() {
    // The point of storing outcomes rather than a seed (H-1): a log that no
    // longer describes a real game must break, not replay into a different one.
    let (log, _) = record(3, 20_000, TradeMode::Disabled);
    let roll = log
        .events
        .iter()
        .position(|e| {
            matches!(
                e.payload,
                Payload::Decision {
                    resolved: Resolved::Dice(..),
                    ..
                }
            )
        })
        .expect("a roll");

    let mut tampered = log.clone();
    let Payload::Decision { resolved, .. } = &mut tampered.events[roll].payload else {
        unreachable!()
    };
    let Resolved::Dice(a, b) = *resolved else {
        unreachable!()
    };
    // A different total, so the game genuinely diverges.
    *resolved = Resolved::Dice(if a == 6 { 1 } else { a + 1 }, b);

    match tampered.verify() {
        Err(ReplayError::SnapshotMismatch { .. }) | Err(ReplayError::Illegal { .. }) => {}
        other => panic!("tampering went unnoticed: {other:?}"),
    }
}

#[test]
fn a_die_that_was_never_rolled_is_rejected() {
    let mut log = record(5, 20_000, TradeMode::Disabled).0;
    // Claim a steal where the engine will roll dice: the script does not fit,
    // so `apply_scripted` falls back to a live draw and the mismatch surfaces.
    let roll = log
        .events
        .iter()
        .position(|e| {
            matches!(
                e.payload,
                Payload::Decision {
                    resolved: Resolved::Dice(..),
                    ..
                }
            )
        })
        .unwrap();
    let Payload::Decision { resolved, .. } = &mut log.events[roll].payload else {
        unreachable!()
    };
    *resolved = Resolved::Steal(None);
    assert!(matches!(log.replay(), Err(ReplayError::Diverged { .. })));
}

// ---------------------------------------------------------------------------
// Redaction. §7.6: a leak surfaces only when someone exploits it, so these
// assert *indistinguishability*. That changing something hidden leaves the
// view byte-identical, rather than spot-checking fields.
// ---------------------------------------------------------------------------

/// A position part-way through a real game, with cards in hands and a deck
/// partly drawn.
fn mid_game(seed: u64) -> State {
    let (log, _) = record(seed, 20_000, TradeMode::Full);
    let mid = log.events.last().unwrap().seq / 2;
    let s = log.replay_to(mid).expect("replay");
    assert!(s.hand_size(0) + s.hand_size(1) + s.hand_size(2) > 0);
    s
}

#[test]
fn moving_a_card_between_two_hands_is_invisible_to_a_third_seat() {
    for seed in 0..25 {
        let base = mid_game(seed);
        // Find a resource seat 0 holds, and hand it to seat 1. Hand *sizes*
        // change, which is public (R-6.2), so keep them equal by swapping.
        let Some(mine) = (0..5).find(|&r| base.hand[0][r] > 0) else {
            continue;
        };
        let Some(theirs) = (0..5).find(|&r| base.hand[1][r] > 0 && r != mine) else {
            continue;
        };
        let mut swapped = base;
        swapped.hand[0][mine] -= 1;
        swapped.hand[0][theirs] += 1;
        swapped.hand[1][theirs] -= 1;
        swapped.hand[1][mine] += 1;

        for viewer in [Viewer::Spectator, Viewer::Seat(2), Viewer::Seat(3)] {
            assert_eq!(
                fog::fog(&base, viewer),
                fog::fog(&swapped, viewer),
                "seed {seed}: {viewer:?} could tell two hands apart"
            );
        }
        // The owners, of course, can tell.
        assert_ne!(
            fog::fog(&base, Viewer::Seat(0)),
            fog::fog(&swapped, Viewer::Seat(0))
        );
    }
}

#[test]
fn the_undrawn_deck_is_invisible_to_everyone() {
    for seed in 0..25 {
        let base = mid_game(seed);
        let drawn = base.dev_drawn as usize;
        if drawn + 2 > base.dev_deck.len() {
            continue;
        }
        let mut shuffled = base;
        shuffled.dev_deck[drawn..].reverse();

        for seat in 0..4 {
            assert_eq!(
                fog::fog(&base, Viewer::Seat(seat)),
                fog::fog(&shuffled, Viewer::Seat(seat)),
                "seed {seed}: seat {seat} could read the deck"
            );
        }
        assert_eq!(
            fog::fog(&base, Viewer::Spectator),
            fog::fog(&shuffled, Viewer::Spectator)
        );
    }
}

#[test]
fn a_held_victory_point_card_does_not_show_in_apparent_points() {
    let mut s = mid_game(11);
    let before = fog::fog(&s, Viewer::Spectator);
    s.dev_held[1][DevCard::VictoryPoint as usize] += 1;
    let after = fog::fog(&s, Viewer::Spectator);

    // The count of cards held is public; what they are is not (R-9.11).
    assert_eq!(after.dev_count[1], before.dev_count[1] + 1);
    assert_eq!(
        after.apparent_vp, before.apparent_vp,
        "hidden points leaked"
    );
    // Its owner sees the real total.
    let own = fog::fog(&s, Viewer::Seat(1)).own.unwrap();
    assert_eq!(own.victory_points, before.apparent_vp[1] + 1);
}

#[test]
fn a_spectator_holds_no_private_data_at_all() {
    let s = mid_game(4);
    assert!(fog::fog(&s, Viewer::Spectator).own.is_none());
    // A seat index outside the game is treated as a spectator, not as an
    // oracle, the safe direction for that to be wrong.
    assert!(fog::fog(&s, Viewer::Seat(9)).own.is_none());
}

#[test]
fn only_the_thief_learns_which_card_was_stolen() {
    let (log, _) = record(2, 20_000, TradeMode::Full);
    let steals: Vec<_> = log
        .decisions()
        .filter(|(_, _, r)| matches!(r, Resolved::Steal(Some(_))))
        .map(|(e, _, r)| (e.seq, r))
        .collect();
    assert!(!steals.is_empty(), "no robbery in this game");

    let seen = project(&log, Viewer::Spectator).expect("project");
    for (seq, _) in &steals {
        let ev = seen.iter().find(|s| s.seq == *seq).unwrap();
        // The visible form carries that a card moved, never which.
        assert!(matches!(
            ev.what,
            SeenWhat::Decision {
                resolved: SeenResolved::Stolen { took_a_card: true },
                ..
            }
        ));
    }

    // And the thief's own view does show the card, via their hand, which is
    // exactly how a person learns it at the table.
    let (seq, _) = steals[0];
    let ev = log.events.iter().find(|e| e.seq == seq).unwrap();
    let carranta_record::Actor::Seat(thief) = ev.actor else {
        panic!("a robbery has an actor")
    };
    let before = log.replay_to(seq - 1).unwrap();
    let after = log.replay_to(seq).unwrap();
    assert_eq!(
        after.hand_size(thief as usize),
        before.hand_size(thief as usize) + 1
    );
}

#[test]
fn projection_agrees_with_replay_at_every_step() {
    let (log, want) = record(9, 20_000, TradeMode::Full);
    let seen = project(&log, Viewer::Seat(0)).expect("project");
    assert_eq!(seen.len(), log.events.len());

    // The last public position must describe the same game the log ends in.
    // Points are the true totals there rather than the apparent ones: the game
    // is over, so held Victory Point cards have been revealed (R-9.11).
    let last = seen.last().unwrap();
    assert_eq!(last.after.apparent_vp[1], want.victory_points(1));
    assert_eq!(last.after.hand_size[2], want.hand_size(2) as u8);
    assert_eq!(last.after.own.unwrap().hand, want.hand[0]);

    // Mid-game the same figure is still the apparent one.
    let mid = &seen[seen.len() / 2];
    let position = log.replay_to(mid.seq).unwrap();
    assert_eq!(mid.after.apparent_vp[1], position.public_victory_points(1));
}

#[test]
fn declining_an_offer_is_recorded_and_changes_nothing() {
    let opening = State::new(4, 1).with_trade_mode(TradeMode::Full);
    let mut rec = Recorder::new(1, 1, opening, vec![SeatId::human(1); 4]);
    let before = *rec.state();
    rec.decline(0, 2);
    assert!(before.same_game_as(rec.state()));

    let log = rec.finish_into(None);
    assert!(matches!(
        log.events[0].payload,
        Payload::Declined { offer: 0, by: 2 }
    ));
    // Churn is data, but it is not history: replay steps over it (H-4, H-7).
    assert!(log.replay().unwrap().same_game_as(&before));
}

#[test]
fn a_finished_log_records_the_winner_and_true_points() {
    let (log, want) = record(6, 20_000, TradeMode::Disabled);
    let Phase::GameOver { winner } = want.phase else {
        panic!("seed 6 did not finish")
    };
    let Some(Payload::Ended { winner: w, vp }) = log.events.last().map(|e| e.payload.clone())
    else {
        panic!("no ending recorded")
    };
    assert_eq!(w, Some(winner));
    assert_eq!(vp[winner as usize], want.victory_points(winner as usize));
    assert!(vp[winner as usize] >= 10, "R-11.1");
}

#[test]
fn an_empty_prefix_replays_to_the_opening() {
    let (log, _) = record(12, 20_000, TradeMode::Disabled);
    let first = log.replay_to(0).unwrap();
    let mut want = *log.created.opening;
    if let Payload::Decision { action, resolved } = log.events[0].payload {
        want.apply_scripted(action, resolved).unwrap();
    }
    assert!(first.same_game_as(&want));
    assert_eq!(log.replay_to(u32::MAX - 1), Err(ReplayError::PastEnd));
}

#[test]
fn recording_does_not_change_how_a_game_goes() {
    // The recorder must be an observer. Same seed, same bots, with and
    // without a log: the same game.
    for seed in 0..20 {
        let (_, recorded) = record(seed, 20_000, TradeMode::Full);

        let mut bots: Vec<Heuristic> = (0..4).map(|i| Heuristic::new(seed * 7 + i)).collect();
        let mut plain = State::new(4, seed).with_trade_mode(TradeMode::Full);
        let mut buf = Vec::new();
        for _ in 0..20_000 {
            if matches!(plain.phase, Phase::GameOver { .. }) {
                break;
            }
            plain.legal_into(&mut buf);
            if buf.is_empty() {
                break;
            }
            let seat = plain.decider() as usize;
            let a = bots[seat].choose(&plain, &buf);
            if plain.apply(a).is_err() {
                break;
            }
        }
        assert!(plain.same_game_as(&recorded), "seed {seed} diverged");
    }
}

#[test]
fn an_action_the_rules_reject_is_reported_with_its_place() {
    let mut log = record(8, 20_000, TradeMode::Disabled).0;
    log.events[3].payload = Payload::Decision {
        action: Action::BuildCity(0),
        resolved: Resolved::None,
    };
    match log.replay() {
        Err(ReplayError::Illegal { seq: 3, .. }) => {}
        other => panic!("expected an illegal action at 3, got {other:?}"),
    }
}
