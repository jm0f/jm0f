//! Run an evolution strategy over the heuristic's weights.
//!
//! Built to be started and left alone. It checkpoints after every generation,
//! resumes exactly, and writes a history you can chart afterwards.
//!
//! ```text
//! cargo run --release -p carranta-evolve --example train -- --out runs/first
//! cargo run --release -p carranta-evolve --example train -- --out runs/first --resume
//! ```
//!
//! To stop it cleanly, create a file called `stop` in the output directory —
//! it finishes the generation in flight, checkpoints, and exits. Interrupting
//! it instead costs at most the generation in progress.
//!
//! Options: `--out DIR --resume --generations N --population N --survivors N
//! --trials N --validation N --sample N --threads N --mutation F --seed N
//! --mode disabled|restricted|full`

use std::path::PathBuf;

use carranta_core::state::TradeMode;
use carranta_evolve::checkpoint;
use carranta_evolve::genome::{Genome, NAMES};
use carranta_evolve::ladder::ANCHOR;
use carranta_evolve::{Config, Report, Trainer};

struct Args {
    out: PathBuf,
    resume: bool,
    generations: u32,
    seed: u64,
    config: Config,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        out: PathBuf::from("runs/latest"),
        resume: false,
        generations: 20,
        seed: 20_260_813,
        config: Config::default(),
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--resume" => args.resume = true,
            "--out" => args.out = PathBuf::from(value()?),
            "--generations" => {
                args.generations = value()?.parse().map_err(|_| "bad --generations")?
            }
            "--seed" => args.seed = value()?.parse().map_err(|_| "bad --seed")?,
            "--population" => {
                args.config.population = value()?.parse().map_err(|_| "bad --population")?
            }
            "--survivors" => {
                args.config.survivors = value()?.parse().map_err(|_| "bad --survivors")?
            }
            "--trials" => {
                args.config.trials = value()?.parse().map_err(|_| "bad --trials")?;
            }
            "--validation" => {
                args.config.validation = value()?.parse().map_err(|_| "bad --validation")?
            }
            "--sample" => args.config.sample = value()?.parse().map_err(|_| "bad --sample")?,
            "--threads" => args.config.threads = value()?.parse().map_err(|_| "bad --threads")?,
            "--mutation" => {
                args.config.mutation = value()?.parse().map_err(|_| "bad --mutation")?
            }
            "--mode" => {
                args.config.mode = match value()?.as_str() {
                    "disabled" => TradeMode::Disabled,
                    "restricted" => TradeMode::Restricted,
                    "full" => TradeMode::Full,
                    other => return Err(format!("unknown --mode `{other}`")),
                }
            }
            "--help" | "-h" => return Err("help".to_string()),
            other => return Err(format!("unknown option `{other}`")),
        }
    }
    if args.config.survivors >= args.config.population {
        return Err("--survivors must be below --population".to_string());
    }
    Ok(args)
}

const USAGE: &str = "\
carranta-evolve

  --out DIR            where checkpoints and history are written (runs/latest)
  --resume             continue the run in --out rather than starting one
  --generations N      generations to run this session (20)
  --seed N             run seed; ignored when resuming
  --population N       genomes per generation (48)
  --survivors N        genomes that breed the next generation (12)
  --trials N           starting games per genome; adapts as the run converges
  --validation N       held-out games the champion is rated on (96)
  --sample N           validation games recorded and analysed (8; 0 to skip)
  --threads N          workers (all cores)
  --mutation F         mutation step, in per-gene scale units (1.0)
  --mode MODE          disabled | restricted | full (restricted)

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
        b.maritime_trades,
        b.settlements_built,
        b.cities_built,
        b.roads_built,
        b.dev_bought,
        b.militia_played,
        b.production,
    )
}

const CSV_HEADER: &str = "generation,trials,games,best_fitness,median_fitness,noise,spread,\
above_anchor,champion_sigma,connectivity,seconds,sampled,turns,trades,offers,maritime,settlements,cities,roads,\
dev_bought,militia,production\n";

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
                "{} already exists — pass --resume to continue it, or choose another --out",
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
    for _ in 0..args.generations {
        if stop.exists() {
            println!("\nstop file found — finishing here");
            break;
        }
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
