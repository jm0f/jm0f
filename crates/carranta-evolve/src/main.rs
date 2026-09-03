//! `carranta-train`, run the training loop: an evolution strategy over the
//! heuristic's weights (phase one) or NEAT over network topologies in the
//! mixed-offer market (phase two).
//!
//! Built to be started and left alone. It checkpoints after every generation,
//! resumes exactly, and writes a history you can chart afterwards. A NEAT run
//! also exports the current champion as `champion.net` each generation, which
//! is the file `carranta-play --trained` deploys.
//!
//! ```text
//! cargo run --release -p carranta-evolve -- --out runs/first
//! cargo run --release -p carranta-evolve -- --out runs/first --resume
//! cargo run --release -p carranta-evolve -- --method neat --out runs/neat-1
//! ```
//!
//! To stop it cleanly, create a file called `stop` in the output directory:
//! it finishes the generation in flight, checkpoints, and exits. Interrupting
//! it instead costs at most the generation in progress.
//!
//! See the `USAGE` string below for the full option list, and
//! `docs/training.md` for the laptop runbook.

use std::path::PathBuf;

use carranta_core::state::TradeMode;
use carranta_evolve::checkpoint;
use carranta_evolve::genome::{Genome, NAMES};
use carranta_evolve::ladder::ANCHOR;
use carranta_evolve::train_neat::{NeatConfig, NeatReport, NeatTrainer};
use carranta_evolve::{Config, Report, Trainer};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Method {
    /// Phase one: an evolution strategy over the heuristic's weights.
    Es,
    /// Phase two: NEAT proper, in the full mixed-offer market.
    Neat,
}

struct Args {
    out: PathBuf,
    resume: bool,
    generations: u32,
    seed: u64,
    method: Method,
    config: Config,
    neat: NeatConfig,
    /// Take a past champion out of the run's checkpoint instead of training:
    /// a label, a generation number, or `best`. `list` names what is there.
    export: Option<String>,
    /// Where an export is written. Defaults beside the checkpoint, named for
    /// the champion, so exporting twice does not overwrite the first answer
    /// and `champion.net` (the newest, rewritten every generation) is left
    /// exactly as the run left it.
    export_to: Option<PathBuf>,
    /// A network file to enrol as the opening incumbent of a fresh run
    /// (E-25): into the hall and the ladder, so the population trains against
    /// the best already shipped instead of rediscovering it.
    baseline: Option<PathBuf>,
    /// Whether --trials-min was said out loud. A resume takes its
    /// configuration from the checkpoint, so a flag only overrides what the
    /// person actually typed, never what a default happens to be.
    trials_min_given: bool,
    /// Whether --phased was said out loud, so a resume only starts a phase
    /// cycle when somebody asked for one.
    phased_given: bool,
    /// Whether --alps was said out loud, so a resume only starts layering
    /// when somebody asked for it.
    alps_given: bool,
    qd_given: bool,
    /// Fill the archive from the run's own champion catalogue before breeding.
    seed_archive: bool,
    /// The same for --stagnation.
    stagnation_given: bool,
    /// The same for --deep-eval.
    deep_eval_given: bool,
    /// The same for --trials-max.
    trials_max_given: bool,
    /// The same for --add-node and --add-conn.
    add_node_given: bool,
    add_conn_given: bool,
    /// Network files to enrol into the permanent opponent field (E-26), on a
    /// fresh run or a resume alike: an exploiter the population must answer,
    /// or a standard it must hold. Pinned, so the hall never evicts them.
    enrol: Vec<PathBuf>,
    /// Generations whose pinned members leave the field on this resume: a
    /// member dominated by another dilutes the seats of the one that still
    /// teaches.
    unpin: Vec<u32>,
}

fn parse_args() -> Result<Args, String> {
    parse_from(std::env::args().skip(1))
}

fn parse_from<I: Iterator<Item = String>>(mut it: I) -> Result<Args, String> {
    let mut args = Args {
        out: PathBuf::from("runs/latest"),
        resume: false,
        generations: 20,
        seed: 20_260_813,
        method: Method::Es,
        config: Config::default(),
        neat: NeatConfig::default(),
        export: None,
        export_to: None,
        baseline: None,
        trials_min_given: false,
        phased_given: false,
        alps_given: false,
        qd_given: false,
        seed_archive: false,
        stagnation_given: false,
        deep_eval_given: false,
        trials_max_given: false,
        add_node_given: false,
        add_conn_given: false,
        enrol: Vec::new(),
        unpin: Vec::new(),
    };
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--resume" => args.resume = true,
            "--out" => args.out = PathBuf::from(value()?),
            "--export" => args.export = Some(value()?),
            "--export-to" => args.export_to = Some(PathBuf::from(value()?)),
            "--generations" => {
                args.generations = value()?.parse().map_err(|_| "bad --generations")?
            }
            "--seed" => args.seed = value()?.parse().map_err(|_| "bad --seed")?,
            "--method" => {
                args.method = match value()?.as_str() {
                    "es" => Method::Es,
                    "neat" => Method::Neat,
                    other => return Err(format!("unknown --method `{other}`")),
                }
            }
            "--population" => {
                args.config.population = value()?.parse().map_err(|_| "bad --population")?;
                args.neat.population = args.config.population;
            }
            "--survivors" => {
                args.config.survivors = value()?.parse().map_err(|_| "bad --survivors")?
            }
            "--trials" => {
                args.config.trials = value()?.parse().map_err(|_| "bad --trials")?;
                args.neat.trials = args.config.trials;
            }
            "--validation" => {
                args.config.validation = value()?.parse().map_err(|_| "bad --validation")?;
                args.neat.validation = args.config.validation;
            }
            "--sample" => {
                args.config.sample = value()?.parse().map_err(|_| "bad --sample")?;
                args.neat.sample = args.config.sample;
            }
            "--threads" => {
                args.config.threads = value()?.parse().map_err(|_| "bad --threads")?;
                args.neat.threads = args.config.threads;
            }
            "--mutation" => {
                args.config.mutation = value()?.parse().map_err(|_| "bad --mutation")?
            }
            "--trials-min" => {
                let v: u32 = value()?.parse().map_err(|_| "bad --trials-min")?;
                if v == 0 {
                    return Err("--trials-min must be at least 1".to_string());
                }
                args.neat.trials_min = v;
                args.trials_min_given = true;
                if args.neat.trials < v {
                    args.neat.trials = v;
                }
            }
            // The ceiling on the adaptive budget. A field competitive enough
            // that the finalists genuinely do not separate doubles the
            // budget for ever; the ceiling is where more resolution stops
            // being worth more generations.
            "--trials-max" => {
                let v: u32 = value()?.parse().map_err(|_| "bad --trials-max")?;
                if v < args.neat.trials_min {
                    return Err("--trials-max must be at least --trials-min".to_string());
                }
                args.neat.trials_max = v;
                args.trials_max_given = true;
            }
            // How many generations a species may go without improving before
            // it stops breeding. The canonical fifteen was never sensitivity
            // tested, and a fresh topology can need longer than that to grow
            // into itself; raising this is the lever for structural patience.
            "--stagnation" => {
                let v: u32 = value()?.parse().map_err(|_| "bad --stagnation")?;
                if v == 0 {
                    return Err("--stagnation must be at least 1".to_string());
                }
                args.neat.params.stagnation = v;
                args.stagnation_given = true;
            }
            "--payoff" => {
                let v = value()?;
                let parts: Vec<f64> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                let [a, b, c, d] = parts[..] else {
                    return Err("--payoff wants four numbers, winner first: 10,4,2,1".to_string());
                };
                if !(a > b && b > c && c > d) {
                    return Err("--payoff must strictly fall from winner to fourth".to_string());
                }
                args.neat.payoff = Some([a, b, c, d]);
            }
            "--win-bonus" => {
                args.neat.win_bonus = value()?.parse().map_err(|_| "bad --win-bonus")?;
                if !args.neat.win_bonus.is_finite() || args.neat.win_bonus < 0.0 {
                    return Err("--win-bonus must be zero or more".to_string());
                }
            }
            "--margin" => args.neat.margin = true,
            "--halving" => args.neat.halving = true,
            "--pfsp" => args.neat.pfsp = true,
            "--rotate" => args.neat.rotate = true,
            "--held-out-anchor" => args.neat.held_out_anchor = true,
            // Deep evaluation of the finalists (E-28): the last halving
            // rounds play inside the beamed search, so selection breeds
            // evaluators for the condition the table deploys them under.
            "--phased" => {
                args.neat.phased = true;
                args.phased_given = true;
            }
            "--alps" => {
                args.neat.alps = true;
                args.alps_given = true;
            }
            // Quality diversity (E-37). It replaces speciation and the age
            // layers rather than joining them, so turning it on turns those
            // off: three selection schemes arguing over one population is
            // not a run anybody can read.
            "--qd" => {
                args.neat.qd = true;
                args.neat.alps = false;
                args.qd_given = true;
            }
            "--qd-games" => {
                let v: u32 = value()?.parse().map_err(|_| "bad --qd-games")?;
                args.neat.qd_games = v;
            }
            // Seed the archive from the champions the run already holds,
            // which is the diversity reservoir it owns and has been ignoring.
            "--seed-archive" => args.seed_archive = true,
            "--deep-eval" => {
                args.neat.deep_eval = true;
                args.deep_eval_given = true;
            }
            // The whole refined-selection bundle (E-20 to E-24) in one word.
            "--refined" => {
                args.neat.margin = true;
                args.neat.halving = true;
                args.neat.pfsp = true;
                args.neat.rotate = true;
                args.neat.held_out_anchor = true;
            }
            "--baseline" => args.baseline = Some(std::path::PathBuf::from(value()?)),
            "--enrol" => args.enrol.push(std::path::PathBuf::from(value()?)),
            "--unpin" => args
                .unpin
                .push(value()?.parse().map_err(|_| "bad --unpin")?),
            // The structural mutation odds, per offspring: how often a new
            // node or a new connection is tried. The levers for a run whose
            // topology has stopped moving.
            "--add-node" => {
                let v: f64 = value()?.parse().map_err(|_| "bad --add-node")?;
                if !(0.0..=1.0).contains(&v) {
                    return Err("--add-node is a probability".to_string());
                }
                args.neat.params.add_node_p = v;
                args.add_node_given = true;
            }
            "--add-conn" => {
                let v: f64 = value()?.parse().map_err(|_| "bad --add-conn")?;
                if !(0.0..=1.0).contains(&v) {
                    return Err("--add-conn is a probability".to_string());
                }
                args.neat.params.add_conn_p = v;
                args.add_conn_given = true;
            }
            "--give-cap" => {
                let v = value()?;
                args.neat.give_cap = if v == "hand" {
                    None
                } else {
                    Some(v.parse().map_err(|_| "bad --give-cap")?)
                };
            }
            "--want-cap" => args.neat.want_cap = value()?.parse().map_err(|_| "bad --want-cap")?,
            "--ask-cap" => args.neat.ask_cap = value()?.parse().map_err(|_| "bad --ask-cap")?,
            "--mode" => {
                let mode = match value()?.as_str() {
                    "disabled" => TradeMode::Disabled,
                    "restricted" => TradeMode::Restricted,
                    "full" => TradeMode::Full,
                    other => return Err(format!("unknown --mode `{other}`")),
                };
                args.config.mode = mode;
                args.neat.mode = mode;
            }
            "--help" | "-h" => return Err("help".to_string()),
            other => return Err(format!("unknown option `{other}`")),
        }
    }
    // Only phase two keeps a ladder of past champions, so an export is a NEAT
    // request whatever else was typed. Without this, `--export 42 --out
    // runs/neat-2` lands in the phase-one path and is answered by its guard
    // against starting a fresh run over an existing checkpoint, which reads
    // like a refusal to export.
    if args.export.is_some() {
        args.method = Method::Neat;
    }
    if args.method == Method::Es && args.config.survivors >= args.config.population {
        return Err("--survivors must be below --population".to_string());
    }
    if args.neat.want_cap == 0 {
        return Err("--want-cap must be at least 1".to_string());
    }
    Ok(args)
}

const USAGE: &str = "\
carranta-evolve

  --method M           es | neat (es). Phase one tunes the heuristic's
                       weights; neat evolves network topologies in the full
                       mixed-offer market and exports champion.net
  --out DIR            where checkpoints and history are written (runs/latest)
  --resume             continue the run in --out rather than starting one
  --generations N      generations to run this session; 0 runs until stopped (20)
  --seed N             run seed; ignored when resuming
  --population N       genomes per generation (48 es, 96 neat)
  --survivors N        es only: genomes that breed the next generation (12)
  --trials N           starting games per genome; adapts as the run converges
  --trials-min N       the floor the budget may fall to (default 16); raise it
                       to buy selection accuracy with games
  --alps               breed in age layers, refilling the youngest with fresh
                       genomes every generation, so a new lineage is measured
                       against its own age rather than against the champion
  --phased             alternate complexifying and simplifying phases: when
                       mean complexity passes a ceiling, additive mutation
                       stops, deletion starts and crossover is suspended
                       until shedding stops paying. Given on a resume, the
                       run begins by simplifying
  --payoff A,B,C,D     score positions by a table, winner first (e.g. 10,4,2,1),
                       instead of position less the win bonus; an unwon first
                       place pays the second place rate
  --validation N       held-out games the champion is rated on (96)
  --sample N           validation games recorded and analysed (8; 0 to skip)
  --threads N          workers (all cores)
  --mutation F         es only: mutation step, in per-gene scale units (1.0)
  --give-cap N|hand    neat only: cards an offer may give (2; hand = no cap)
  --want-cap N         neat only: cards an offer may ask (2)
  --ask-cap N          neat only: proposals generated per seat per turn (3).
                       Time is what asking costs at a real table; this is its
                       deterministic stand-in, and served tables share it
  --win-bonus F        neat only: what a win is worth beyond first place
                       (1.0). Subtracted from the finishing position, so a
                       won game scores 0 and first is two steps clear of
                       second. 0 selects on position alone, which favours
                       avoiding fourth over reaching first
  --export WHICH       write a past champion out of --out's checkpoint
                       instead of training; implies --method neat, the only
                       method that keeps past champions. WHICH is a label
                       (g042-0017), a generation number (42), `best` by
                       rating, or `list` to see what the run has
  --export-to FILE     where --export writes (default: beside the checkpoint,
                       named for the champion)
  --mode MODE          disabled | restricted | full (restricted es, full neat)

Create a file named `stop` in --out to finish the current generation and exit.";

/// One row of the run history.
fn csv_row(r: &Report, connectivity: f64) -> String {
    let b = &r.behaviour;
    format!(
        "{},{},{},{:.6},{:.6},{:.6},{:.4},{:.6},{:.4},{:.3},{:.4},{},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3}\n",
        r.generation,
        r.trials,
        r.games,
        r.best_fitness,
        r.median_fitness,
        r.noise,
        r.spread,
        r.above_anchor,
        r.champion_sigma,
        connectivity,
        r.seconds,
        b.games,
        b.turns,
        b.trades,
        b.offers_made,
        b.supply_trades,
        b.settlements_built,
        b.cities_built,
        b.roads_built,
        b.dev_bought,
        b.militia_played,
        b.production,
    )
}

const CSV_HEADER: &str = "generation,trials,games,best_fitness,median_fitness,noise,spread,\
above_anchor,champion_sigma,connectivity,seconds,sampled,turns,trades,offers,supply_trades,settlements,cities,roads,\
dev_bought,militia,production\n";

/// One row of a phase-two run's history.
fn neat_csv_row(r: &NeatReport, connectivity: f64) -> String {
    let b = &r.behaviour;
    format!(
        "{},{},{},{:.6},{:.6},{:.6},{},{},{},{},{:.2},{},{:.1},{},{:.6},{},{:.6},{:.4},{:.4},{:.4},{:.4},{:.3},{},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3}\n",
        r.generation,
        r.trials,
        r.games,
        r.best_fitness,
        r.median_fitness,
        r.noise,
        r.species,
        r.champion_nodes,
        r.champion_genes,
        r.champion_ears,
        r.mpc,
        if r.simplifying {
            "simplify"
        } else {
            "complexify"
        },
        r.mean_age,
        r.archive_filled,
        r.archive_mean,
        r.archive_found,
        r.gap,
        r.gap_ci,
        r.wins,
        r.wins_ci,
        connectivity,
        r.seconds,
        b.games,
        b.turns,
        b.trades,
        b.offers_made,
        b.supply_trades,
        b.settlements_built,
        b.cities_built,
        b.roads_built,
        b.dev_bought,
        b.militia_played,
        b.production,
    )
}

const NEAT_CSV_HEADER: &str = "generation,trials,games,best_fitness,median_fitness,noise,\
species,champion_nodes,champion_genes,champion_ears,mpc,phase,age,cells,archive_mean,found,gap,gap_ci,wins,wins_ci,connectivity,seconds,\
sampled,turns,trades,offers,supply_trades,settlements,cities,roads,dev_bought,militia,production\n";

/// Take one past champion out of a run and write it as a network file.
///
/// A run exports only its newest champion, overwriting `champion.net` every
/// generation, so by morning the good one from generation forty is not on
/// disk. It is not lost either: the checkpoint carries the whole ladder, every
/// champion with its genome and its rating, which is what this reads. Training
/// is not started and nothing in the run directory is disturbed, so this is
/// safe to run against a run that is still going.
fn export_champion(ckpt: &std::path::Path, which: &str, to: Option<&std::path::Path>) {
    let trainer = match checkpoint::load_neat(ckpt) {
        Ok(Ok(t)) => t,
        Ok(Err(e)) => {
            eprintln!("{}: {e}", ckpt.display());
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("cannot read {}: {e}", ckpt.display());
            std::process::exit(1);
        }
    };
    let roster = trainer.roster();
    if roster.is_empty() {
        eprintln!("{} has no champions yet", ckpt.display());
        std::process::exit(1);
    }
    // Listing is the other half of choosing: a label cannot be guessed, and a
    // generation number is only useful once you know which ones are there.
    if which == "list" {
        println!("  champion    generation    mu   sigma   games");
        for (label, generation, mu, sigma, games) in roster {
            println!("  {label:<12}{generation:>10}{mu:>7.2}{sigma:>8.2}{games:>8}");
        }
        println!("\n  export one with --export <champion|generation|best>");
        return;
    }
    // The run's name comes off the directory the checkpoint lives in:
    // `runs/neat-6/checkpoint.txt` is run `neat-6`.
    let run = ckpt.parent().map(|d| run_name(d)).unwrap_or_default();
    let Some((label, text)) = trainer.export(which, &run) else {
        eprintln!(
            "no champion `{which}` in {}; --export list names them",
            ckpt.display()
        );
        std::process::exit(1);
    };
    // Named for the champion rather than `champion.net`, so an export never
    // stands on the run's own file and two exports never stand on each other.
    let path = to.map(|p| p.to_path_buf()).unwrap_or_else(|| {
        ckpt.with_file_name(format!("champion-{label}.net"))
            .to_path_buf()
    });
    if let Err(e) = std::fs::write(&path, text) {
        eprintln!("cannot write {}: {e}", path.display());
        std::process::exit(1);
    }
    println!("wrote {} ({label})", path.display());
    println!(
        "  play it: cargo run --release -p carranta-ui -- --trained {}",
        path.display()
    );
}

/// Fill the quality-diversity archive from the champions a run already holds.
///
/// Every distinct body in the ladder is played a few games at seat 0 against
/// the anchor, which gives it both a fitness on the run's own scale and the
/// behavioural descriptors that decide its cell. Duplicates are skipped: a
/// ladder carries one entry per generation and a body that reigned for three
/// hundred generations is in there three hundred times, all identical.
///
/// Fitness is computed here rather than read off the ladder's rating, because
/// the archive compares within a cell against numbers the generation loop
/// produces, and a rating is on a different scale entirely.
fn seed_archive_from_catalogue(trainer: &mut NeatTrainer) {
    use carranta_core::state::{MAX_PLAYERS, OfferShapes};
    use carranta_evolve::arena::{Arena, Brain, NetJob};
    use carranta_evolve::behaviour::Sampler;
    use carranta_evolve::mapelites::{Descriptor, Placed};

    let cfg = trainer.config;
    let arena = Arena {
        mode: cfg.mode,
        shapes: OfferShapes::Mixed {
            give: cfg.give_cap,
            want: cfg.want_cap,
        },
        asks: cfg.ask_cap,
        cap: cfg.cap,
    };
    let games = cfg.qd_games.max(2);
    let mut ids = trainer.ladder.ids();
    ids.sort_unstable();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut bodies = 0usize;
    let mut placed = 0usize;
    for id in ids {
        if id == ANCHOR {
            continue;
        }
        let Some(v) = trainer.ladder.get(id) else {
            continue;
        };
        let genome = v.genome.clone();
        let generation = v.generation;
        if !seen.insert(genome.show()) {
            continue;
        }
        bodies += 1;
        // Seat 0 is the candidate, the other three the heuristic anchor: the
        // one opponent every champion in the catalogue was measured against,
        // whatever era it came from, so the fitnesses are commensurable.
        let roster = [Brain::Anchor, Brain::Net(genome.compile())];
        let jobs: Vec<NetJob> = (0..games as u64)
            .map(|t| NetJob {
                seed: 900_000_007 + t,
                seats: [1, 0, 0, 0],
            })
            .collect();
        let mut style = Sampler::default();
        let mut score = 0.0;
        for (outcome, log) in arena.play_net_all_recorded(&roster, &jobs, cfg.threads) {
            style.add_seat(&log, 0);
            let won = outcome.winner == Some(0);
            score += if cfg.margin {
                let own = outcome.vp[0] as f64;
                let others = (1..MAX_PLAYERS).map(|s| outcome.vp[s] as f64).sum::<f64>()
                    / (MAX_PLAYERS - 1) as f64;
                (others - own) - if won { cfg.win_bonus } else { 0.0 }
            } else {
                outcome.position[0] as f64 - if won { cfg.win_bonus } else { 0.0 }
            };
        }
        let fitness = score / games as f64;
        let descriptor = Descriptor::of(&style.finish());
        if trainer.seed_archive(genome, fitness, descriptor, generation) != Placed::Rejected {
            placed += 1;
        }
    }
    println!(
        "archive seeded from the catalogue: {bodies} distinct champions, {placed} took a cell, \
         {} of {} cells filled",
        trainer.archive().filled(),
        carranta_evolve::mapelites::CELLS * carranta_evolve::mapelites::CELLS
    );
}

/// A network file as an enrollable genome, with the generation it names.
///
/// The genome is the link list itself, re-numbered: an enrolled outsider
/// sits in the field and never breeds, so its innovation numbers only have
/// to be internally consistent.
fn genome_of_net(path: &std::path::Path) -> (carranta_evolve::neat::NeatGenome, u32) {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("cannot read {}: {e}", path.display());
            std::process::exit(1);
        }
    };
    let Some((net, generation, _)) = carranta_bot::net::Net::parse_meta(&text) else {
        eprintln!("{} is not a champion network file", path.display());
        std::process::exit(1);
    };
    // Node ids are positional: inputs, then bias, then output, then hidden.
    // A file written when the observation was narrower numbers its bias,
    // output and hidden nodes lower than this build does, so its ids are
    // shifted up into today's numbering. The network is unchanged: its links
    // still touch only the inputs it was trained on, and the senses added
    // since are inputs it never reads.
    let theirs = net.inputs();
    let ours = carranta_evolve::neat::INPUTS;
    if theirs > ours {
        eprintln!(
            "{} reads {theirs} inputs and this build observes {ours}: a network from a wider \
             observation cannot be narrowed",
            path.display()
        );
        std::process::exit(1);
    }
    let lift = (ours - theirs) as u32;
    let renumber = |id: u32| if id >= theirs as u32 { id + lift } else { id };
    let genes = text
        .lines()
        .filter_map(|l| {
            let mut p = l.split_whitespace();
            (p.next() == Some("link")).then(|| {
                Some(carranta_evolve::neat::Gene {
                    innov: 0,
                    from: renumber(p.next()?.parse().ok()?),
                    to: renumber(p.next()?.parse().ok()?),
                    weight: p.next()?.parse().ok()?,
                    enabled: true,
                })
            })?
        })
        .enumerate()
        .map(|(i, mut g)| {
            g.innov = i as u32;
            g
        })
        .collect();
    (carranta_evolve::neat::NeatGenome { genes }, generation)
}

/// The phase-two loop: the same rhythm as phase one, plus a champion export.
fn run_neat(args: Args) {
    let ckpt = args.out.join("checkpoint.txt");
    let history = args.out.join("history.csv");
    let champion_file = args.out.join("champion.net");
    let stop = args.out.join("stop");

    if let Some(which) = &args.export {
        export_champion(&ckpt, which, args.export_to.as_deref());
        return;
    }

    if args.resume && args.baseline.is_some() {
        eprintln!("--baseline seeds a fresh run; a resume already has its hall");
        std::process::exit(2);
    }
    let mut trainer = if args.resume {
        match checkpoint::load_neat(&ckpt) {
            Ok(Ok(mut t)) => {
                t.config.threads = args.neat.threads;
                // The budget floor and the species patience are training
                // knobs a plateau is allowed to turn mid-run: both change
                // how hard the run looks, not what it is looking at. Only
                // when said out loud, so a bare resume stays exact.
                if args.trials_min_given {
                    t.config.trials_min = args.neat.trials_min;
                    println!("trials floor raised to {}", t.config.trials_min);
                }
                // Switching the phased controller on mid-run starts by
                // shedding (E-35): the reason to reach for it is a genome
                // already too big, not a ceiling still ahead.
                if args.alps_given && !t.config.alps {
                    t.config.alps = true;
                    println!("age layers on, the youngest refilled every generation");
                }
                if args.qd_given && !t.config.qd {
                    t.config.qd = true;
                    t.config.alps = false;
                    t.config.qd_games = args.neat.qd_games;
                    println!(
                        "quality diversity on, breeding from a {}x{} archive of styles",
                        carranta_evolve::mapelites::CELLS,
                        carranta_evolve::mapelites::CELLS
                    );
                }
                if args.phased_given && !t.config.phased {
                    t.config.phased = true;
                    t.begin_simplifying();
                    println!("phased search on, simplifying from here");
                }
                if args.stagnation_given {
                    t.config.params.stagnation = args.neat.params.stagnation;
                    println!(
                        "species stagnation window now {}",
                        t.config.params.stagnation
                    );
                }
                if args.deep_eval_given {
                    t.config.deep_eval = true;
                    println!("finalists now evaluated a ply deep");
                }
                if args.trials_max_given {
                    t.config.trials_max = args.neat.trials_max;
                    println!("budget ceiling now {}", t.config.trials_max);
                }
                if args.add_node_given {
                    t.config.params.add_node_p = args.neat.params.add_node_p;
                    println!("add-node odds now {}", t.config.params.add_node_p);
                }
                if args.add_conn_given {
                    t.config.params.add_conn_p = args.neat.params.add_conn_p;
                    println!("add-conn odds now {}", t.config.params.add_conn_p);
                }
                println!(
                    "resumed {} at generation {}",
                    ckpt.display(),
                    t.generation()
                );
                t
            }
            Ok(Err(e)) => {
                eprintln!("{}: {e}", ckpt.display());
                std::process::exit(1);
            }
            Err(e) => {
                eprintln!("cannot read {}: {e}", ckpt.display());
                std::process::exit(1);
            }
        }
    } else {
        if ckpt.exists() {
            eprintln!(
                "{} already exists, pass --resume to continue it, or choose another --out",
                ckpt.display()
            );
            std::process::exit(1);
        }
        let _ = std::fs::write(&history, NEAT_CSV_HEADER);
        let mut fresh = NeatTrainer::new(args.neat, args.seed);
        // The opening incumbent (E-25): a shipped champion enrolled into the
        // hall before the first deal, so a fresh population is measured
        // against the best there is from generation one.
        if let Some(path) = &args.baseline {
            let (genome, generation) = genome_of_net(path);
            let id = fresh.seed_baseline(genome, generation);
            println!(
                "baseline {} enrolled as the opening incumbent (ladder id {id}, generation {generation})",
                path.display()
            );
        }
        fresh
    };

    for &generation in &args.unpin {
        let gone = trainer.unpin(generation);
        println!("unpinned {gone} member(s) of generation {generation} from the field");
    }
    // Outsiders pinned into the field (E-26), fresh run or resumed alike: an
    // exploiter the population has to answer stays in the field until it has
    // been answered, because the hall's eviction cannot reach it.
    for path in &args.enrol {
        let (genome, generation) = genome_of_net(path);
        let id = trainer.seed_baseline(genome, generation);
        println!(
            "{} pinned into the field (ladder id {id}, generation {generation})",
            path.display()
        );
    }
    // Seed the archive from the run's own champions (E-37).
    //
    // A run switched into quality diversity mid-flight starts with an empty
    // grid, and an empty grid breeds from whatever batch happens to be loaded.
    // On a converged run that batch is a hundred and fifty variations of one
    // player, so the archive would fill with copies of the thing that stopped
    // working. The catalogue is the way out: every distinct champion the run
    // ever crowned, hundreds of generations apart and genuinely unalike,
    // already sitting in the ladder. Seeding from it keeps the learning and
    // supplies the diversity in one step, which beats rewinding to an earlier
    // generation, because a rewind buys the diversity by throwing the
    // learning away.
    if args.seed_archive {
        seed_archive_from_catalogue(&mut trainer);
    }

    let c = &trainer.config;
    println!(
        "neat: population {}   market {:?} (give {}, want {}, asks {})   {}   workers {}   target species {}",
        c.population,
        c.mode,
        c.give_cap.map_or("hand".to_string(), |n| n.to_string()),
        c.want_cap,
        c.ask_cap,
        match c.payoff {
            Some([a, b, cc, d]) => format!("payoff {a}/{b}/{cc}/{d}"),
            None => format!("win bonus {}", c.win_bonus),
        },
        c.threads,
        c.params.target_species,
    );
    println!("writing to {}", args.out.display());
    {
        let words: Vec<&str> = [
            (c.margin, "margin"),
            (c.halving, "halving"),
            (c.pfsp, "pfsp"),
            (c.rotate, "rotate"),
            (c.held_out_anchor, "held-out-anchor"),
            (c.deep_eval, "deep-eval"),
        ]
        .into_iter()
        .filter_map(|(on, w)| on.then_some(w))
        .collect();
        if words.is_empty() {
            println!("  selection: classic");
        } else {
            println!("  selection: {}", words.join(" "));
        }
    }
    if c.margin {
        println!(
            "  fitness is the mean opposing victory points less your own, \
             less {} a win, lower is better",
            c.win_bonus
        );
    } else {
        match c.payoff {
            Some([a, b, cc, d]) => println!(
                "  fitness is minus the payoff of the place taken ({a}, {b}, {cc}, {d}; an \
                 unwon first place pays {b}), lower is better"
            ),
            None => println!(
                "  fitness is mean finishing position less {} a win, lower is better",
                c.win_bonus
            ),
        }
    }
    println!("  gap and wins are the champion's paired match against the anchor:");
    println!("  a negative gap and a win share above 50% are ahead, and either");
    println!("  one inside its interval has not been shown\n");
    println!(
        "  gen  trials    games    best  median   noise   sep  spp  nodes  genes  ears    mpc   age    cells        gap (E-16)        wins (E-17)   trades   secs"
    );

    let started = std::time::Instant::now();
    let mut total_games = 0u64;
    let mut done = 0u32;
    // The body of the last champion archived under `champions/`, so a reign
    // of a hundred generations is one file rather than a hundred copies. A
    // resume starts empty and re-archives the incumbent once, which is the
    // cheap way to be sure the archive holds it.
    let mut archived = String::new();
    while args.generations == 0 || done < args.generations {
        if stop.exists() {
            println!("\nstop file found, finishing here");
            break;
        }
        done += 1;
        let r = trainer.step();
        total_games += r.games as u64;
        let separated = (r.median_fitness - r.best_fitness) > 2.0 * r.noise;
        println!(
            "  {:>3}  {:>6}  {:>7}  {:.4}  {:.4}  {:.4}  {:>4}  {:>3}  {:>5}  {:>5}  {:>4}  {:>5.1}{}  {:>4.1}  {:>7}  {:>+7.3} +-{:>5.3}  {:>5.1}% +-{:>4.1}  {:>6.1}  {:>5.1}",
            r.generation,
            r.trials,
            r.games,
            r.best_fitness,
            r.median_fitness,
            r.noise,
            if separated { "yes" } else { "NO" },
            r.species,
            r.champion_nodes,
            r.champion_genes,
            r.champion_ears,
            r.mpc,
            // A simplifying generation is marked where it happens, so the
            // shedding and what it cost sit on the same line.
            if r.simplifying { "-" } else { " " },
            r.mean_age,
            // Coverage, and how many cells were reached for the first time.
            // Blank on a run that does not keep an archive, rather than a
            // column of zeroes pretending to mean something.
            if trainer.config.qd {
                format!("{}+{}", r.archive_filled, r.archive_found)
            } else {
                String::new()
            },
            r.gap,
            r.gap_ci,
            100.0 * r.wins,
            100.0 * r.wins_ci,
            r.behaviour.trades,
            r.seconds,
        );

        if let Err(e) = checkpoint::save_neat(&trainer, &ckpt) {
            eprintln!("warning: could not write {}: {e}", ckpt.display());
        }
        // The reigning champion, as a file a server can be handed. Written
        // beside the checkpoint every generation, so "deploy the latest" is a
        // copy of one small text file at any moment of a run.
        if let Some(g) = trainer.champion_genome() {
            let text = g.compile().show_from(r.generation, &run_name(&args.out));
            let temp = champion_file.with_extension("tmp");
            if std::fs::write(&temp, &text)
                .and_then(|_| std::fs::rename(&temp, &champion_file))
                .is_err()
            {
                eprintln!("warning: could not write {}", champion_file.display());
            }
            // And kept under `champions/` whenever the network itself is new.
            // A net is a few kilobytes, so a run's whole succession costs less
            // than a screenshot, and the archive is what makes a striking row
            // in the console retestable afterwards: `champion.net` remembers
            // one generation, this directory remembers them all. The body is
            // what is compared, because the generation header changes every
            // time and the network mostly does not.
            let body: String = text
                .lines()
                .filter(|l| !l.starts_with("generation"))
                .collect();
            if body != archived {
                let dir = args.out.join("champions");
                let name = dir.join(format!("gen-{:05}.net", r.generation));
                if std::fs::create_dir_all(&dir)
                    .and_then(|_| std::fs::write(&name, &text))
                    .is_ok()
                {
                    archived = body;
                } else {
                    eprintln!("warning: could not write {}", name.display());
                }
            }
        }
        let connectivity = trainer.ladder.connectivity(1);
        if let Ok(mut f) = std::fs::OpenOptions::new().append(true).open(&history) {
            use std::io::Write;
            let _ = f.write_all(neat_csv_row(&r, connectivity).as_bytes());
        }
    }

    let elapsed = started.elapsed().as_secs_f64();
    if total_games > 0 {
        println!(
            "\n  {total_games} games in {:.1} s   ({:.0} games/s)",
            elapsed,
            total_games as f64 / elapsed
        );
    }

    println!("\n== ladder ==");
    println!(
        "  connectivity {:.0}% of versions have played the anchor directly\n",
        trainer.ladder.connectivity(1) * 100.0
    );
    println!("  version        games      mu   sigma   shown   +anchor");
    let anchor_mu = trainer.ladder.rating(ANCHOR).mu;
    for (v, r, n) in trainer.ladder.standings(1).iter().take(10) {
        println!(
            "  {:<13} {:>5}  {:>6.2}  {:>5.2}  {:>6.2}  {:>+7.2}{}",
            v.label,
            n,
            r.mu,
            r.sigma,
            r.conservative(),
            r.mu - anchor_mu,
            if v.id == ANCHOR { "   <- pinned" } else { "" },
        );
    }

    println!("\n  checkpoint  {}", ckpt.display());
    println!("  champion    {}", champion_file.display());
    println!("  history     {}", history.display());
    println!(
        "  resume with --method neat --out {} --resume",
        args.out.display()
    );
}

fn main() {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            if e != "help" {
                eprintln!("error: {e}\n");
            }
            eprintln!("{USAGE}");
            std::process::exit(if e == "help" { 0 } else { 2 });
        }
    };

    if let Err(e) = std::fs::create_dir_all(&args.out) {
        eprintln!("cannot use {}: {e}", args.out.display());
        std::process::exit(1);
    }
    if args.method == Method::Neat {
        return run_neat(args);
    }
    let ckpt = args.out.join("checkpoint.txt");
    let history = args.out.join("history.csv");
    let stop = args.out.join("stop");

    let mut trainer = if args.resume {
        match checkpoint::load(&ckpt) {
            Ok(Ok(mut t)) => {
                // Cores belong to the machine, not to the run.
                t.config.threads = args.config.threads;
                println!(
                    "resumed {} at generation {}",
                    ckpt.display(),
                    t.generation()
                );
                t
            }
            Ok(Err(e)) => {
                eprintln!("{}: {e}", ckpt.display());
                std::process::exit(1);
            }
            Err(e) => {
                eprintln!("cannot read {}: {e}", ckpt.display());
                std::process::exit(1);
            }
        }
    } else {
        if ckpt.exists() {
            eprintln!(
                "{} already exists, pass --resume to continue it, or choose another --out",
                ckpt.display()
            );
            std::process::exit(1);
        }
        let _ = std::fs::write(&history, CSV_HEADER);
        Trainer::new(args.config, args.seed)
    };

    let c = &trainer.config;
    println!(
        "population {}   survivors {}   market {:?}   workers {}   sample {}",
        c.population, c.survivors, c.mode, c.threads, c.sample
    );
    println!("writing to {}", args.out.display());
    println!("  fitness is mean finishing position, lower is better (2.5 = average)");
    println!("  gap is the champion's paired match against the anchor, in positions:");
    println!("  negative is ahead, and a gap inside its interval has not been shown\n");
    println!(
        "  gen  trials    games    best  median   noise   sep  spread      +anchor   trades  cities   secs"
    );

    let started = std::time::Instant::now();
    let mut total_games = 0u64;
    let mut done = 0u32;
    // Zero means "until told otherwise". A multi-day run should not need a
    // generation count guessed in advance.
    while args.generations == 0 || done < args.generations {
        if stop.exists() {
            println!("\nstop file found, finishing here");
            break;
        }
        done += 1;
        let r = trainer.step();
        total_games += r.games as u64;
        let separated = (r.median_fitness - r.best_fitness) > 2.0 * r.noise;
        println!(
            "  {:>3}  {:>6}  {:>7}  {:.4}  {:.4}  {:.4}  {:>4}  {:>6.2}  {:>+6.2} +-{:>4.1}  {:>6.1}  {:>6.2}  {:>5.1}",
            r.generation,
            r.trials,
            r.games,
            r.best_fitness,
            r.median_fitness,
            r.noise,
            if separated { "yes" } else { "NO" },
            r.spread,
            r.above_anchor,
            r.champion_sigma,
            r.behaviour.trades,
            r.behaviour.cities_built,
            r.seconds,
        );

        // After every generation, so an interruption costs at most one.
        if let Err(e) = checkpoint::save(&trainer, &ckpt) {
            eprintln!("warning: could not write {}: {e}", ckpt.display());
        }
        let connectivity = trainer.ladder.connectivity(1);
        if let Ok(mut f) = std::fs::OpenOptions::new().append(true).open(&history) {
            use std::io::Write;
            let _ = f.write_all(csv_row(&r, connectivity).as_bytes());
        }
    }

    let elapsed = started.elapsed().as_secs_f64();
    if total_games > 0 {
        println!(
            "\n  {total_games} games in {:.1} s   ({:.0} games/s)",
            elapsed,
            total_games as f64 / elapsed
        );
    }

    println!("\n== ladder ==");
    println!(
        "  connectivity {:.0}% of versions have played the anchor directly\n",
        trainer.ladder.connectivity(1) * 100.0
    );
    println!("  version        games      mu   sigma   shown   +anchor");
    for (v, r, n) in trainer.ladder.standings(1).iter().take(10) {
        println!(
            "  {:<13} {:>5}  {:>6.2}  {:>5.2}  {:>6.2}  {:>+7.2}{}",
            v.label,
            n,
            r.mu,
            r.sigma,
            r.conservative(),
            r.mu - trainer.ladder.rating(ANCHOR).mu,
            if v.id == ANCHOR { "   <- pinned" } else { "" },
        );
    }

    println!("\n== what evolution changed ==");
    let start = Genome::default();
    let best = trainer.best();
    println!("  weight            hand-set   evolved   change");
    for ((name, &a), &b) in NAMES.iter().zip(&start.genes).zip(&best.genes) {
        if a == b {
            continue;
        }
        println!(
            "  {:<16} {:>8}  {:>8}   {:>+6.0}%",
            name,
            a,
            b,
            if a == 0 {
                f64::NAN
            } else {
                (b - a) as f64 / (a as f64).abs() * 100.0
            }
        );
    }

    println!("\n  checkpoint  {}", ckpt.display());
    println!("  history     {}", history.display());
    println!("  resume with --out {} --resume", args.out.display());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(line: &str) -> Result<Args, String> {
        parse_from(line.split_whitespace().map(str::to_string))
    }

    #[test]
    fn an_enrolled_narrow_champion_is_the_same_player_it_was() {
        // League members enrol as genomes, and genome node ids are
        // positional: a file written when the observation was 32 wide numbers
        // its bias, output and hidden nodes lower than this build does.
        // `genome_of_net` lifts those ids into today's numbering, and the
        // proof it lifted correctly is behavioural: the compiled genome must
        // value positions exactly as the file's own network does reading its
        // own slice.
        let narrow = 32usize;
        let out32 = carranta_bot::net::Net::output_id(narrow as u32 as usize);
        let bias32 = carranta_bot::net::Net::bias_id(narrow);
        let hidden = out32 + 1; // first hidden id in the narrow numbering
        let mut links: Vec<(u32, u32, f64)> = vec![
            (0, hidden, 0.4),
            (5, hidden, -0.3),
            (bias32, hidden, 0.2),
            (hidden, out32, 0.9),
        ];
        links.extend((1..5u32).map(|i| (i, out32, i as f64 / 10.0)));
        let old = carranta_bot::net::Net::assemble(narrow, &links).expect("acyclic");
        let path = std::env::temp_dir().join(format!("carranta-enrol-{}.net", std::process::id()));
        std::fs::write(&path, old.show_from(642, "neat-8")).expect("written");

        let (genome, generation) = genome_of_net(&path);
        let _ = std::fs::remove_file(&path);
        assert_eq!(generation, 642, "the file's generation survives");
        let wide = genome.compile();
        let state = carranta_core::state::State::new(4, 21);
        for me in 0..4 {
            let obs = carranta_bot::features::encode(
                &state,
                me,
                carranta_bot::features::Pending::default(),
            );
            assert_eq!(
                wide.eval(&obs),
                old.eval(&obs[..narrow]),
                "seat {me}: the lifted genome and the file disagree"
            );
        }
    }

    #[test]
    fn an_empty_command_line_is_a_complete_run() {
        let a = parse("").expect("no flags is valid");
        let d = Config::default();
        assert_eq!(a.out.to_str(), Some("runs/latest"));
        assert!(!a.resume);
        assert_eq!(a.config.population, d.population);
        assert_eq!(a.config.mode, d.mode);
    }

    #[test]
    fn flags_reach_the_configuration() {
        let a = parse(
            "--out runs/x --resume --generations 40 --seed 7 --population 16 \
             --survivors 4 --trials 200 --validation 64 --sample 3 --threads 6 \
             --mutation 0.5 --mode full",
        )
        .expect("every flag is known");
        assert_eq!(a.out.to_str(), Some("runs/x"));
        assert!(a.resume);
        assert_eq!(a.generations, 40);
        assert_eq!(a.seed, 7);
        assert_eq!(a.config.population, 16);
        assert_eq!(a.config.survivors, 4);
        assert_eq!(a.config.trials, 200);
        assert_eq!(a.config.validation, 64);
        assert_eq!(a.config.sample, 3);
        assert_eq!(a.config.threads, 6);
        assert_eq!(a.config.mutation, 0.5);
        assert_eq!(a.config.mode, TradeMode::Full);
    }

    #[test]
    fn the_refined_bundle_sets_every_selection_flag() {
        let a = parse("--method neat --refined --baseline bots/trained-462.net")
            .expect("the bundle is one word");
        assert!(a.neat.margin);
        assert!(a.neat.halving);
        assert!(a.neat.pfsp);
        assert!(a.neat.rotate);
        assert!(a.neat.held_out_anchor);
        assert_eq!(
            a.baseline.as_deref(),
            Some(std::path::Path::new("bots/trained-462.net"))
        );

        // And one at a time, for the ablation runs.
        let a = parse("--method neat --rotate --pfsp").expect("single flags parse");
        assert!(a.neat.rotate && a.neat.pfsp);
        assert!(!a.neat.margin && !a.neat.halving && !a.neat.held_out_anchor);
        assert!(a.baseline.is_none());
    }

    #[test]
    fn mid_run_knobs_remember_whether_they_were_said() {
        // A resume takes its configuration from the checkpoint, so these
        // flags only override when actually typed: the booleans are what
        // the resume branch reads.
        let a = parse("--method neat --trials-min 64 --stagnation 25 --deep-eval --trials-max 512")
            .expect("the flags parse");
        assert!(a.trials_min_given && a.stagnation_given && a.deep_eval_given);
        assert!(a.trials_max_given);
        assert_eq!(a.neat.trials_max, 512);
        assert!(a.neat.deep_eval);
        assert!(
            parse("--trials-min 64 --trials-max 32").is_err(),
            "ceiling under floor"
        );
        assert_eq!(a.neat.trials_min, 64);
        assert_eq!(a.neat.params.stagnation, 25);
        let a = parse("--method neat").expect("bare");
        assert!(!a.trials_min_given && !a.stagnation_given);
        assert!(parse("--stagnation 0").is_err());
    }

    #[test]
    fn the_field_levers_parse_and_remember_being_said() {
        let a = parse(
            "--method neat --enrol runs/x/champions/gen-00642.net \
             --add-node 0.08 --add-conn 0.15",
        )
        .expect("the levers parse");
        assert_eq!(a.enrol.len(), 1);
        assert!(a.add_node_given && a.add_conn_given);
        assert_eq!(a.neat.params.add_node_p, 0.08);
        assert_eq!(a.neat.params.add_conn_p, 0.15);
        let a = parse("--method neat").expect("bare");
        assert!(!a.add_node_given && !a.add_conn_given && a.enrol.is_empty());
        assert!(parse("--add-node 1.5").is_err(), "a probability");
    }

    #[test]
    fn a_mistyped_command_line_stops_the_run_rather_than_guessing() {
        // Silently falling back to a default would spend hours of machine time
        // on a configuration nobody asked for.
        assert!(parse("--population twelve").is_err());
        assert!(parse("--mode barter").is_err());
        assert!(parse("--populaton 12").is_err(), "typo in the flag itself");
        assert!(parse("--threads").is_err(), "flag with no value");
        assert!(
            parse("--survivors 48 --population 48").is_err(),
            "a population that all survives never selects"
        );
    }

    #[test]
    fn zero_generations_means_until_stopped() {
        // Not "do nothing": a run left alone for days should not need its
        // length guessed up front. The loop reads it as unbounded.
        let a = parse("--generations 0").expect("zero is allowed");
        assert_eq!(a.generations, 0);
    }

    #[test]
    fn neat_flags_reach_the_configuration() {
        let a = parse(
            "--method neat --give-cap hand --want-cap 3 --ask-cap 5 --population 32 --mode full",
        )
        .expect("every flag is known");
        assert_eq!(a.method, Method::Neat);
        assert_eq!(a.neat.ask_cap, 5);
        assert_eq!(
            parse("--method neat").expect("defaults").neat.ask_cap,
            3,
            "the training default is three asks a turn"
        );
        assert_eq!(a.neat.give_cap, None, "hand means bounded by the hand");
        assert_eq!(a.neat.want_cap, 3);
        assert_eq!(
            parse("--method neat").expect("defaults").neat.win_bonus,
            1.0,
            "a win is worth a place beyond first by default"
        );
        assert_eq!(
            parse("--method neat --win-bonus 0")
                .expect("known")
                .neat
                .win_bonus,
            0.0,
            "and zero is E-6's pure position fitness"
        );
        assert!(parse("--win-bonus -1").is_err(), "a bonus is not a penalty");
        assert!(parse("--win-bonus x").is_err());
        assert_eq!(a.neat.population, 32);
        assert_eq!(a.neat.mode, TradeMode::Full);
        let b = parse("--method neat --give-cap 2").expect("a number is a cap");
        assert_eq!(b.neat.give_cap, Some(2));
        assert!(parse("--method neither").is_err());
        assert!(parse("--give-cap x").is_err());
        assert!(
            parse("--want-cap 0").is_err(),
            "an offer that may ask for nothing is not an offer"
        );
    }

    #[test]
    fn an_export_is_asked_for_by_name_and_never_by_accident() {
        let a = parse("--method neat --out runs/x --export g042-0017").expect("known flags");
        assert_eq!(a.export.as_deref(), Some("g042-0017"));
        assert_eq!(a.export_to, None, "defaulted beside the checkpoint");
        let b = parse("--method neat --export best --export-to /tmp/champ.net").expect("known");
        assert_eq!(b.export.as_deref(), Some("best"));
        assert_eq!(
            b.export_to.as_deref(),
            Some(std::path::Path::new("/tmp/champ.net"))
        );
        // An ordinary run must never be read as an export: this decides
        // between training for hours and writing one file and exiting.
        assert_eq!(
            parse("--method neat --out runs/x").expect("known").export,
            None
        );
        assert!(parse("--export").is_err(), "a name is required");
        // The ladder is phase two's, so asking for a champion is asking for
        // NEAT. Typed without --method, an export used to fall into phase one
        // and be answered by its "checkpoint already exists" guard.
        let c = parse("--out runs/neat-2 --export 72").expect("no --method needed");
        assert_eq!(c.method, Method::Neat);
        assert_eq!(c.export.as_deref(), Some("72"));
        assert_eq!(
            parse("--out runs/neat-2").expect("known").method,
            Method::Es,
            "and nothing else changes the default method"
        );
    }

    #[test]
    fn a_neat_csv_row_matches_its_header() {
        let cfg = NeatConfig {
            population: 6,
            trials: 4,
            validation: 4,
            trials_min: 4,
            sample: 0,
            threads: 2,
            cap: 400,
            mode: TradeMode::Disabled,
            ..NeatConfig::default()
        };
        let report = NeatTrainer::new(cfg, 3).step();
        let row = neat_csv_row(&report, 1.0);
        assert_eq!(
            row.matches(',').count(),
            NEAT_CSV_HEADER.matches(',').count(),
            "history.csv would be unreadable if the row drifted from its header"
        );
        assert!(row.ends_with('\n'));
    }

    #[test]
    fn a_csv_row_matches_its_header() {
        let cfg = Config {
            population: 6,
            survivors: 2,
            trials: 8,
            trials_min: 4,
            validation: 8,
            sample: 0,
            threads: 2,
            ..Config::default()
        };
        let report = Trainer::new(cfg, 3).step();
        let row = csv_row(&report, 1.0);
        assert_eq!(
            row.matches(',').count(),
            CSV_HEADER.matches(',').count(),
            "history.csv would be unreadable if the row drifted from its header"
        );
        assert!(row.ends_with('\n'));
    }

    #[test]
    fn workers_buy_time_not_a_different_answer() {
        // The claim the help text makes, checked rather than asserted.
        let mut cfg = Config {
            population: 6,
            survivors: 2,
            trials: 8,
            trials_min: 4,
            validation: 8,
            sample: 0,
            threads: 1,
            ..Config::default()
        };
        let one = Trainer::new(cfg, 11).step();
        cfg.threads = 4;
        let many = Trainer::new(cfg, 11).step();
        assert_eq!(one.best_fitness, many.best_fitness);
        assert_eq!(one.above_anchor, many.above_anchor);
        assert_eq!(one.games, many.games);
    }
}

/// The run's name, read off its directory: `runs/neat-6` is run `neat-6`.
fn run_name(out: &std::path::Path) -> String {
    out.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}
