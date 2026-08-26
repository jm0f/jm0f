//! Where genomes are measured against each other.
//!
//! Three properties, each of which is a registered decision rather than an
//! implementation detail:
//!
//! **Trading is on** (E-9). A bot tuned in a game where nobody trades learns
//! strategies that will not transfer to human play, so training runs with
//! `Restricted`. One card for one card, so the generated action space stays
//! enumerable, but the negotiation is real. Costs roughly twice `Disabled`.
//!
//! **Common random numbers** (E-4). Every genome in a generation plays the
//! *same* board seeds against the *same* opponents, seat-rotated. That removes
//! board luck and seat effects by construction rather than averaging them
//! away. The feasibility measurement showed the paired difference between
//! identical agents is exactly zero, and it is what makes a generation
//! affordable.
//!
//! **Deterministic under parallelism.** A game's result depends only on its
//! seed and its four genomes, never on which worker ran it, so results do not
//! shift with core count or scheduling. A test asserts it.

use std::sync::atomic::{AtomicUsize, Ordering};

use carranta_bot::net::Net;
use carranta_bot::policy_net::{DeepNetPolicy, NetPolicy};
use carranta_bot::{Heuristic, Policy, settle_market};
use carranta_core::Action;
use carranta_core::state::{MAX_PLAYERS, OfferShapes, Phase, State, TradeMode};
use carranta_record::{Log, Recorder, SeatId};

use crate::genome::Genome;

/// One game to play: a board seed and the four seats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Job {
    pub seed: u64,
    pub seats: [Genome; MAX_PLAYERS],
}

/// How a game finished.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Outcome {
    /// Finishing position per seat, 1 = winner.
    pub position: [u32; MAX_PLAYERS],
    /// True victory points per seat (R-11.3).
    pub vp: [u32; MAX_PLAYERS],
    pub winner: Option<u8>,
    /// Actions applied, for cost accounting.
    pub actions: u32,
}

/// Plays games.
#[derive(Clone, Copy, Debug)]
pub struct Arena {
    pub mode: TradeMode,
    /// What proposals the market enumerates. Phase one trained under the
    /// single-type default; NEAT trains in the full mixed market, capped 2 a
    /// side unless a run says otherwise.
    pub shapes: OfferShapes,
    /// Proposals generated per seat per turn (E-15). The rules cap by
    /// default, which is the uncapped market every run before the allowance
    /// existed trained in.
    pub asks: u8,
    /// Actions before a game is abandoned. Reached only by a pathological
    /// genome, which then simply scores badly.
    pub cap: usize,
}

impl Default for Arena {
    fn default() -> Self {
        Arena {
            mode: TradeMode::Restricted,
            shapes: OfferShapes::SingleType,
            asks: carranta_core::state::OFFERS_PER_TURN,
            cap: 20_000,
        }
    }
}

/// Who plays a seat in a network game.
///
/// The anchor is not a network and never will be: it is the pinned heuristic,
/// the origin of the rating scale, so it appears in the roster as itself
/// rather than as some approximation of itself.
#[derive(Clone, Debug)]
pub enum Brain {
    Anchor,
    Net(Net),
    /// The same network one ply deeper (E-27): the evolved evaluation inside
    /// a beamed two-ply search, for measuring what depth buys. No setup
    /// rollout: this is the search training's inner loop can afford, a
    /// rollout per placement across tens of thousands of games is not.
    Deep(Net),
    /// The full deployment search (E-31): the two-ply beam plus the setup
    /// rollout, exactly what a lobby seat plays.
    DeepPlanned(Net),
    /// The ablation of the deep market answer: deep moves, one-ply accepts.
    DeepFlatMarket(Net),
}

/// One network game to play: a board seed and four roster indices.
///
/// Indices rather than networks so a job stays tiny and `Copy`, and a
/// thousand jobs sharing one hall-of-fame champion share its compiled network
/// rather than carrying a thousand clones of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NetJob {
    pub seed: u64,
    pub seats: [u32; MAX_PLAYERS],
}

impl Arena {
    /// Play one game.
    ///
    /// The seat's genome determines its play, and its generator is seeded from
    /// the seat *and* the board, so two identical genomes in one game still
    /// break ties independently rather than mirroring each other.
    pub fn play(&self, job: &Job) -> Outcome {
        let mut a = seat_bot(job, 0);
        let mut b = seat_bot(job, 1);
        let mut c = seat_bot(job, 2);
        let mut d = seat_bot(job, 3);
        let mut policies: Vec<&mut dyn Policy> = vec![&mut a, &mut b, &mut c, &mut d];
        self.drive(job.seed, &mut policies)
    }

    /// Play one network game.
    pub fn play_net(&self, roster: &[Brain], job: &NetJob) -> Outcome {
        let mut seats = net_seats(self, roster, job);
        let mut policies: Vec<&mut dyn Policy> = seats
            .iter_mut()
            .map(|p| p.as_mut() as &mut dyn Policy)
            .collect();
        self.drive(job.seed, &mut policies)
    }

    /// The game loop every kind of seat shares.
    fn drive(&self, seed: u64, policies: &mut Vec<&mut dyn Policy>) -> Outcome {
        let mut state = State::new(MAX_PLAYERS as u8, seed)
            .with_trade_mode(self.mode)
            .with_offer_shapes(self.shapes)
            .with_ask_allowance(self.asks);
        let mut buf = Vec::new();
        let mut actions = 0u32;

        while (actions as usize) < self.cap {
            if matches!(state.phase, Phase::GameOver { .. }) {
                break;
            }
            state.legal_into(&mut buf);
            if buf.is_empty() {
                break;
            }
            let seat = state.decider() as usize;
            let action = policies[seat].choose(&state, &buf);
            if state.apply(action).is_err() {
                break;
            }
            actions += 1;
            // Offers are worthless unless somebody is asked, and opponents
            // never reach `choose` during another seat's turn.
            settle_market(&mut state, policies);
        }

        let winner = match state.phase {
            Phase::GameOver { winner } => Some(winner),
            _ => None,
        };
        let vp: [u32; MAX_PLAYERS] = core::array::from_fn(|p| state.victory_points(p));
        Outcome {
            position: positions(winner, &vp),
            vp,
            winner,
            actions,
        }
    }

    /// Play many games across `threads` workers.
    ///
    /// Work is taken dynamically rather than split up front: game length varies
    /// several-fold, so a static split leaves workers idle at the end.
    pub fn play_all(&self, jobs: &[Job], threads: usize) -> Vec<Outcome> {
        play_many(jobs, threads, |j| self.play(j))
    }

    /// Play many network games across `threads` workers.
    pub fn play_net_all(&self, roster: &[Brain], jobs: &[NetJob], threads: usize) -> Vec<Outcome> {
        play_many(jobs, threads, |j| self.play_net(roster, j))
    }

    /// Play one game and keep a full record of it (§7).
    ///
    /// For sampling only. Recording costs nothing measurable per game, but the
    /// logs themselves would swamp a run that kept every one, so the trainer
    /// takes a small sample and this stays off the hot path.
    pub fn play_recorded(&self, job: &Job) -> (Outcome, Log) {
        let mut a = seat_bot(job, 0);
        let mut b = seat_bot(job, 1);
        let mut c = seat_bot(job, 2);
        let mut d = seat_bot(job, 3);
        let mut policies: Vec<&mut dyn Policy> = vec![&mut a, &mut b, &mut c, &mut d];
        self.drive_recorded(job.seed, &mut policies)
    }

    /// Play one network game and keep a full record of it.
    pub fn play_net_recorded(&self, roster: &[Brain], job: &NetJob) -> (Outcome, Log) {
        let mut seats = net_seats(self, roster, job);
        let mut policies: Vec<&mut dyn Policy> = seats
            .iter_mut()
            .map(|p| p.as_mut() as &mut dyn Policy)
            .collect();
        self.drive_recorded(job.seed, &mut policies)
    }

    fn drive_recorded(&self, seed: u64, policies: &mut Vec<&mut dyn Policy>) -> (Outcome, Log) {
        let mut rec = Recorder::new(
            seed,
            seed,
            State::new(MAX_PLAYERS as u8, seed)
                .with_trade_mode(self.mode)
                .with_offer_shapes(self.shapes)
                .with_ask_allowance(self.asks),
            (0..MAX_PLAYERS)
                .map(|s| SeatId::agent(s as u64, "evolve", 1))
                .collect(),
        );
        let mut buf = Vec::new();
        let mut actions = 0u32;

        while (actions as usize) < self.cap {
            if matches!(rec.state().phase, Phase::GameOver { .. }) {
                break;
            }
            rec.state().legal_into(&mut buf);
            if buf.is_empty() {
                break;
            }
            let seat = rec.state().decider() as usize;
            let action = policies[seat].choose(rec.state(), &buf);
            if rec.apply(action).is_err() {
                break;
            }
            actions += 1;
            settle_recorded(&mut rec, policies);
        }

        let winner = match rec.state().phase {
            Phase::GameOver { winner } => Some(winner),
            _ => None,
        };
        let vp: [u32; MAX_PLAYERS] = core::array::from_fn(|p| rec.state().victory_points(p));
        let outcome = Outcome {
            position: positions(winner, &vp),
            vp,
            winner,
            actions,
        };
        (outcome, rec.finish_into(winner))
    }
}

/// The dynamic worker pool every batch shares.
///
/// Work is taken dynamically rather than split up front: game length varies
/// several-fold, so a static split leaves workers idle at the end. A game's
/// result depends only on its job, never on which worker ran it, so results
/// cannot shift with core count or scheduling.
fn play_many<J: Sync>(
    jobs: &[J],
    threads: usize,
    play: impl Fn(&J) -> Outcome + Sync,
) -> Vec<Outcome> {
    if jobs.is_empty() {
        return Vec::new();
    }
    let threads = threads.max(1).min(jobs.len());
    if threads == 1 {
        return jobs.iter().map(&play).collect();
    }

    let next = AtomicUsize::new(0);
    let mut out = vec![Outcome::default(); jobs.len()];
    let play = &play;
    let collected: Vec<Vec<(usize, Outcome)>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..threads)
            .map(|_| {
                let next = &next;
                scope.spawn(move || {
                    let mut mine = Vec::new();
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        if i >= jobs.len() {
                            break;
                        }
                        mine.push((i, play(&jobs[i])));
                    }
                    mine
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });

    for batch in collected {
        for (i, outcome) in batch {
            out[i] = outcome;
        }
    }
    out
}

/// The four policies of one network game, boxed because two kinds of brain
/// sit at one table.
fn net_seats(arena: &Arena, roster: &[Brain], job: &NetJob) -> Vec<Box<dyn Policy>> {
    let _ = arena;
    (0..MAX_PLAYERS)
        .map(|s| {
            let seed = job.seed.wrapping_mul(31).wrapping_add(s as u64 + 1);
            match &roster[job.seats[s] as usize] {
                Brain::Anchor => Box::new(Heuristic::new(seed)) as Box<dyn Policy>,
                Brain::Net(n) => Box::new(NetPolicy::new(n.clone(), seed)) as Box<dyn Policy>,
                Brain::Deep(n) => {
                    Box::new(DeepNetPolicy::flat_setup(n.clone(), seed)) as Box<dyn Policy>
                }
                Brain::DeepPlanned(n) => {
                    Box::new(DeepNetPolicy::new(n.clone(), seed)) as Box<dyn Policy>
                }
                Brain::DeepFlatMarket(n) => {
                    Box::new(DeepNetPolicy::flat_market(n.clone(), seed)) as Box<dyn Policy>
                }
            }
        })
        .collect()
}

/// Settle the market through the recorder, so completed trades reach the log.
///
/// [`settle_market`] writes straight to a `State`, which would leave every
/// trade out of the record, a busy market with no trades in it.
fn settle_recorded(rec: &mut Recorder, policies: &mut [&mut dyn Policy]) {
    if rec.state().trade_mode == TradeMode::Disabled || rec.state().offer_count == 0 {
        return;
    }
    let mut declined = [[false; carranta_core::state::MAX_OFFERS]; MAX_PLAYERS];
    for _ in 0..16 {
        let mut acted = false;
        #[allow(clippy::needless_range_loop)] // `i` indexes both offers and `declined`
        'outer: for i in 0..rec.state().offer_count as usize {
            for seat in 0..policies.len() {
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
                if policies[seat].accepts(rec.state(), seat, i) {
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

fn seat_bot(job: &Job, seat: usize) -> Heuristic {
    let mut bot = Heuristic::new(job.seed.wrapping_mul(31).wrapping_add(seat as u64 + 1));
    bot.weights = job.seats[seat].weights();
    bot
}

/// Finishing positions from the result: 1 = winner, ties share a position.
///
/// The winner is placed first outright, only the active player can win
/// (R-11.1), so equal points at the top is not a tie in the game.
pub fn positions(winner: Option<u8>, vp: &[u32; MAX_PLAYERS]) -> [u32; MAX_PLAYERS] {
    core::array::from_fn(|i| {
        if Some(i as u8) == winner {
            return 1;
        }
        let ahead = (0..MAX_PLAYERS)
            .filter(|&j| j != i)
            .filter(|&j| Some(j as u8) == winner || vp[j] > vp[i])
            .count();
        ahead as u32 + 1
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(seed: u64, challenger: Genome, field: Genome) -> Job {
        Job {
            seed,
            seats: [challenger, field, field, field],
        }
    }

    #[test]
    fn a_game_finishes_and_ranks_everyone() {
        let arena = Arena::default();
        let base = Genome::default();
        for seed in 0..20 {
            let o = arena.play(&job(seed, base, base));
            assert!(o.winner.is_some(), "seed {seed} did not finish");
            assert!(o.actions > 50);
            // Positions are a permutation-with-ties over 1..=4.
            let w = o.winner.unwrap() as usize;
            assert_eq!(o.position[w], 1);
            assert!(o.vp[w] >= 10, "R-11.1");
            for p in 0..MAX_PLAYERS {
                assert!((1..=4).contains(&o.position[p]));
            }
        }
    }

    #[test]
    fn trading_actually_happens_in_the_arena() {
        // The whole point of E-9. If the market never settles, every strategy
        // learned here is learned in a game nobody plays.
        let arena = Arena::default();
        assert_eq!(arena.mode, TradeMode::Restricted);
        let base = Genome::default();
        let quiet = Arena {
            mode: TradeMode::Disabled,
            ..Arena::default()
        };
        let mut traded = 0;
        for seed in 0..20 {
            let with = arena.play(&job(seed, base, base));
            let without = quiet.play(&job(seed, base, base));
            if with != without {
                traded += 1;
            }
        }
        assert!(
            traded >= 18,
            "only {traded}/20 games differed with the market open"
        );
    }

    #[test]
    fn results_do_not_depend_on_the_worker_count() {
        // The property that lets a run be reproduced on a different machine.
        let arena = Arena::default();
        let base = Genome::default();
        let other = base.mutate(&mut carranta_core::rng::Rng::new(1), 3.0);
        let jobs: Vec<Job> = (0..60).map(|s| job(s, other, base)).collect();

        let one = arena.play_all(&jobs, 1);
        for threads in [2, 4, 8] {
            assert_eq!(arena.play_all(&jobs, threads), one, "{threads} workers");
        }
    }

    #[test]
    fn pairing_is_exact() {
        // Identical genomes on the same board play the identical game, so a
        // paired comparison has zero board and seat variance. This is what E-4
        // buys, and it is worth an assertion because losing it would quietly
        // multiply the cost of every generation.
        let arena = Arena::default();
        let base = Genome::default();
        for seed in 0..30 {
            let a = arena.play(&job(seed, base, base));
            let b = arena.play(&job(seed, base, base));
            assert_eq!(a, b, "seed {seed} is not reproducible");
        }
    }

    #[test]
    fn a_crippled_genome_loses() {
        // The sanity floor for the whole apparatus. All-zero weights score
        // every candidate identically, so the bot picks on its tie-break
        // stream alone, random play, which the bot work measured at a 0.24%
        // win rate. If the arena cannot see that, no amount of selection helps.
        let arena = Arena::default();
        let base = Genome::default();
        let crippled = Genome {
            genes: [0; crate::genome::GENES],
        };

        let mut crippled_wins = 0;
        let mut base_wins = 0;
        for seed in 0..60 {
            let o = arena.play(&Job {
                seed,
                seats: [crippled, base, crippled, base],
            });
            match o.winner {
                Some(0) | Some(2) => crippled_wins += 1,
                Some(1) | Some(3) => base_wins += 1,
                _ => {}
            }
        }
        assert!(
            base_wins > crippled_wins * 5,
            "the arena cannot tell a crippled genome apart: {base_wins} vs {crippled_wins}"
        );
    }

    #[test]
    fn the_points_weight_is_nearly_redundant() {
        // A finding, pinned so it is not rediscovered by accident: zeroing the
        // victory-point term barely changes how the bot plays.
        //
        // The reason is collinearity. `pips`, `road`, `militia` and `dev`
        // already reward the actions that *produce* points. A settlement is
        // worth building for its production whether or not the point it
        // carries is counted, so the points term is mostly re-describing what
        // the other terms say.
        //
        // It matters for training: evolution cannot gain much on a gene that
        // does not move the outcome, and a search that expects a smooth
        // response will spend its budget on a plateau. It is also an argument
        // for E-3 handing a network features rather than these weights
        // forever, since a learned combination could use the redundancy that
        // fixed weights cannot.
        let arena = Arena::default();
        let base = Genome::default();
        let mut no_points = base;
        no_points.genes[0] = 0;

        let mut differed = 0;
        let mut no_points_wins = 0;
        let mut base_wins = 0;
        for seed in 0..60 {
            let o = arena.play(&Job {
                seed,
                seats: [no_points, base, no_points, base],
            });
            if o != arena.play(&Job {
                seed,
                seats: [base; MAX_PLAYERS],
            }) {
                differed += 1;
            }
            match o.winner {
                Some(0) | Some(2) => no_points_wins += 1,
                Some(1) | Some(3) => base_wins += 1,
                _ => {}
            }
        }
        assert!(differed > 50, "the change did nothing at all to the games");
        assert!(
            no_points_wins * 2 > base_wins,
            "expected near-parity, got {no_points_wins} vs {base_wins}, if this now fails the feature set has stopped being collinear"
        );
    }

    #[test]
    fn positions_handle_ties_and_hidden_points() {
        // Two players level behind the winner share second; the position below
        // is consumed.
        assert_eq!(positions(Some(2), &[8, 8, 10, 4]), [2, 2, 1, 4]);
        // The winner can trail on visible points thanks to hidden cards.
        assert_eq!(positions(Some(1), &[9, 10, 9, 3]), [2, 1, 2, 4]);
        // An unfinished game still ranks by points.
        assert_eq!(positions(None, &[5, 7, 3, 3]), [2, 1, 3, 3]);
    }
}
