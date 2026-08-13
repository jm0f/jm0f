//! Run an evolution strategy over the heuristic's weights.
//!
//! `cargo run --release -p carranta-evolve --example train -- [generations]`

use carranta_evolve::genome::{Genome, NAMES};
use carranta_evolve::ladder::ANCHOR;
use carranta_evolve::{Config, Trainer};

fn main() {
    let generations: u32 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(12);

    let config = Config::default();
    println!(
        "evolution strategy over {} weights",
        carranta_evolve::genome::GENES
    );
    println!(
        "  population {}   survivors {}   market {:?}   workers {}",
        config.population, config.survivors, config.mode, config.threads
    );
    println!("  fitness is mean finishing position, lower is better (2.5 = average)\n");

    let mut trainer = Trainer::new(config, 20_260_813);
    println!("  gen  trials    games    best  median   noise   sep   spread   +anchor    secs");

    let started = std::time::Instant::now();
    let mut total_games = 0u64;
    for _ in 0..generations {
        let r = trainer.step();
        total_games += r.games as u64;
        let separated = (r.median_fitness - r.best_fitness) > 2.0 * r.noise;
        println!(
            "  {:>3}  {:>6}  {:>7}  {:.4}  {:.4}  {:.4}  {:>4}  {:>6.2}  {:>+7.2}  {:>6.1}",
            r.generation,
            r.trials,
            r.games,
            r.best_fitness,
            r.median_fitness,
            r.noise,
            if separated { "yes" } else { "NO" },
            r.spread,
            r.above_anchor,
            r.seconds,
        );
    }

    let elapsed = started.elapsed().as_secs_f64();
    println!(
        "\n  {total_games} games in {:.1} s   ({:.0} games/s)",
        elapsed,
        total_games as f64 / elapsed
    );

    // ---- The ladder: every champion, on one scale ----
    println!("\n== ladder ==");
    println!("  every version plays the anchor directly, so any two are two hops apart");
    println!(
        "  connectivity {:.0}% of versions anchored\n",
        trainer.ladder.connectivity(1) * 100.0
    );
    println!("  version        games      mu   sigma   shown   +anchor");
    for (v, r, n) in trainer.ladder.standings(1).iter().take(12) {
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

    // ---- What moved ----
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

    println!("\n== checkpoint (first lines) ==");
    for line in trainer.ladder.encode().lines().take(4) {
        println!("  {line}");
    }
}
