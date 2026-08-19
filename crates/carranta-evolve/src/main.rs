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
            "--give-cap" => {
                let v = value()?;
                args.neat.give_cap = if v == "hand" {
                    None
                } else {
                    Some(v.parse().map_err(|_| "bad --give-cap")?)
                };
            }
            "--want-cap" => args.neat.want_cap = value()?.parse().map_err(|_| "bad --want-cap")?,
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
  --validation N       held-out games the champion is rated on (96)
  --sample N           validation games recorded and analysed (8; 0 to skip)
  --threads N          workers (all cores)
  --mutation F         es only: mutation step, in per-gene scale units (1.0)
  --give-cap N|hand    neat only: cards an offer may give (2; hand = no cap)
  --want-cap N         neat only: cards an offer may ask (2)
  --export WHICH       neat only: write a past champion out of --out's
                       checkpoint instead of training. WHICH is a label
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
        "{},{},{},{:.6},{:.6},{:.6},{},{},{},{:.6},{:.4},{:.4},{:.3},{},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3}\n",
        r.generation,
        r.trials,
        r.games,
        r.best_fitness,
        r.median_fitness,
        r.noise,
        r.species,
        r.champion_nodes,
        r.champion_genes,
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

const NEAT_CSV_HEADER: &str = "generation,trials,games,best_fitness,median_fitness,noise,\
species,champion_nodes,champion_genes,above_anchor,champion_sigma,connectivity,seconds,\
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
    let Some((label, text)) = trainer.export(which) else {
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

    let mut trainer = if args.resume {
        match checkpoint::load_neat(&ckpt) {
            Ok(Ok(mut t)) => {
                t.config.threads = args.neat.threads;
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
        NeatTrainer::new(args.neat, args.seed)
    };

    let c = &trainer.config;
    println!(
        "neat: population {}   market {:?} (give {}, want {})   workers {}   target species {}",
        c.population,
        c.mode,
        c.give_cap.map_or("hand".to_string(), |n| n.to_string()),
        c.want_cap,
        c.threads,
        c.params.target_species,
    );
    println!("writing to {}", args.out.display());
    println!("  fitness is mean finishing position, lower is better (2.5 = average)");
    println!("  read +anchor against its sigma: a gap inside it is noise, not progress\n");
    println!(
        "  gen  trials    games    best  median   noise   sep  spp  nodes  genes      +anchor   trades   secs"
    );

    let started = std::time::Instant::now();
    let mut total_games = 0u64;
    let mut done = 0u32;
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
            "  {:>3}  {:>6}  {:>7}  {:.4}  {:.4}  {:.4}  {:>4}  {:>3}  {:>5}  {:>5}  {:>+6.2} +-{:>4.1}  {:>6.1}  {:>5.1}",
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
            r.above_anchor,
            r.champion_sigma,
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
            let text = g.compile().show(r.generation);
            let temp = champion_file.with_extension("tmp");
            if std::fs::write(&temp, text)
                .and_then(|_| std::fs::rename(&temp, &champion_file))
                .is_err()
            {
                eprintln!("warning: could not write {}", champion_file.display());
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
    println!("  read +anchor against its sigma: a gap inside it is noise, not progress\n");
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
        let a = parse("--method neat --give-cap hand --want-cap 3 --population 32 --mode full")
            .expect("every flag is known");
        assert_eq!(a.method, Method::Neat);
        assert_eq!(a.neat.give_cap, None, "hand means bounded by the hand");
        assert_eq!(a.neat.want_cap, 3);
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
