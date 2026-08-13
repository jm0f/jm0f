//! Saving and resuming a run.
//!
//! A multi-day run on a laptop will be interrupted — a lid closed, a reboot, a
//! terminal shut. Without this, every interruption costs the whole run.
//!
//! **Resume is exact, not approximate.** A generation's randomness is derived
//! from `(run_seed, generation)` rather than carried in an evolving generator,
//! so a checkpoint needs only numbers that can be written down. A run resumed
//! from generation 40 produces exactly the games it would have produced had it
//! never stopped, which is what makes a long run reproducible at all.
//!
//! The format is plain text on purpose. A run that dies overnight should leave
//! something a person can read, diff and salvage without the program that
//! wrote it.

use std::fmt::Write as _;
use std::path::Path;

use carranta_analytics::rating::Rating;
use carranta_core::state::TradeMode;

use crate::genome::Genome;
use crate::ladder::{ANCHOR, Ladder, Versioned};
use crate::train::{Config, Trainer};

/// Bumped when the format changes in a way an older reader would misread.
pub const FORMAT: u32 = 1;

/// Why a checkpoint could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadError {
    /// Written by a different format version.
    Version(u32),
    /// A line that should have parsed did not. Carries the line number, so a
    /// hand-edited checkpoint says where it went wrong.
    Malformed { line: usize, what: String },
    /// A section the format requires was absent.
    Missing(&'static str),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::Version(v) => write!(f, "checkpoint format {v}, expected {FORMAT}"),
            LoadError::Malformed { line, what } => write!(f, "line {line}: {what}"),
            LoadError::Missing(s) => write!(f, "missing section: {s}"),
        }
    }
}

fn mode_name(m: TradeMode) -> &'static str {
    match m {
        TradeMode::Disabled => "disabled",
        TradeMode::Restricted => "restricted",
        TradeMode::Full => "full",
    }
}

fn mode_from(s: &str) -> Option<TradeMode> {
    match s {
        "disabled" => Some(TradeMode::Disabled),
        "restricted" => Some(TradeMode::Restricted),
        "full" => Some(TradeMode::Full),
        _ => None,
    }
}

/// Render a whole run as text.
pub fn encode(trainer: &Trainer) -> String {
    let c = &trainer.config;
    let mut out = String::new();
    let _ = writeln!(out, "carranta-evolve {FORMAT}");
    let _ = writeln!(out, "run_seed {}", trainer.run_seed);
    let _ = writeln!(out, "generation {}", trainer.generation);
    let _ = writeln!(out, "trials {}", trainer.trials);
    let _ = writeln!(out, "population {}", c.population);
    let _ = writeln!(out, "survivors {}", c.survivors);
    let _ = writeln!(out, "validation {}", c.validation);
    let _ = writeln!(out, "trials_min {}", c.trials_min);
    let _ = writeln!(out, "trials_max {}", c.trials_max);
    let _ = writeln!(out, "mutation {}", c.mutation);
    let _ = writeln!(out, "hall_seats {}", c.hall_seats);
    let _ = writeln!(out, "hall_size {}", c.hall_size);
    let _ = writeln!(out, "sample {}", c.sample);
    let _ = writeln!(out, "mode {}", mode_name(c.mode));

    let _ = writeln!(out, "\n# population: one genome per line");
    let _ = writeln!(out, "genomes {}", trainer.population.len());
    for g in &trainer.population {
        let _ = writeln!(out, "{}", g.encode());
    }

    let _ = writeln!(out, "\n# hall of fame: ladder ids, oldest first");
    let _ = writeln!(out, "hall {}", trainer.hall.len());
    for id in &trainer.hall {
        let _ = writeln!(out, "{id}");
    }

    let _ = writeln!(
        out,
        "\n# ladder: id generation games mu sigma anchored label genes..."
    );
    let mut ids = trainer.ladder.ids();
    ids.sort_unstable();
    let _ = writeln!(out, "ladder {}", ids.len());
    for id in ids {
        let v = trainer.ladder.get(id).expect("id came from the ladder");
        let r = trainer.ladder.rating(id);
        let _ = writeln!(
            out,
            // Display for f64 emits the shortest string that parses back to
            // the same value, so a rating survives the round trip exactly. A
            // fixed number of decimals would not, and the drift would show up
            // generations later as a resumed run that diverged.
            "{} {} {} {} {} {} {} {}",
            v.id,
            v.generation,
            trainer.ladder.games_played(id),
            r.mu,
            r.sigma,
            trainer.ladder.anchored_games(id),
            v.label,
            v.genome.encode(),
        );
    }
    out
}

/// Read a run back.
pub fn decode(text: &str) -> Result<Trainer, LoadError> {
    let mut lines = text
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim()))
        .filter(|(_, l)| !l.is_empty() && !l.starts_with('#'));

    let bad = |line: usize, what: &str| LoadError::Malformed {
        line,
        what: what.to_string(),
    };

    let (line, header) = lines.next().ok_or(LoadError::Missing("header"))?;
    let version: u32 = header
        .strip_prefix("carranta-evolve ")
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| bad(line, "not a carranta-evolve checkpoint"))?;
    if version != FORMAT {
        return Err(LoadError::Version(version));
    }

    // Scalars, in the order `encode` writes them.
    let mut scalar = |key: &'static str| -> Result<(usize, String), LoadError> {
        let (line, text) = lines.next().ok_or(LoadError::Missing(key))?;
        let value = text
            .strip_prefix(key)
            .and_then(|v| v.strip_prefix(' '))
            .ok_or_else(|| bad(line, &format!("expected `{key} <value>`, got `{text}`")))?;
        Ok((line, value.to_string()))
    };
    macro_rules! num {
        ($key:literal) => {{
            let (line, v) = scalar($key)?;
            v.parse()
                .map_err(|_| bad(line, &format!("`{}` is not a number for {}", v, $key)))?
        }};
    }

    let run_seed: u64 = num!("run_seed");
    let generation: u32 = num!("generation");
    let trials: u32 = num!("trials");
    let config = Config {
        // The live budget is `trials` above; `Config::trials` is only the
        // value a fresh run starts from, so it is restored to match.
        trials,
        population: num!("population"),
        survivors: num!("survivors"),
        validation: num!("validation"),
        trials_min: num!("trials_min"),
        trials_max: num!("trials_max"),
        mutation: num!("mutation"),
        hall_seats: num!("hall_seats"),
        hall_size: num!("hall_size"),
        sample: num!("sample"),
        mode: {
            let (line, v) = scalar("mode")?;
            mode_from(&v).ok_or_else(|| bad(line, &format!("unknown trade mode `{v}`")))?
        },
        // Not saved: how many cores are available is a property of the machine
        // resuming the run, not of the run.
        threads: Config::default().threads,
    };

    let count = |lines: &mut dyn Iterator<Item = (usize, &str)>,
                 key: &'static str|
     -> Result<usize, LoadError> {
        let (line, text) = lines.next().ok_or(LoadError::Missing(key))?;
        text.strip_prefix(key)
            .and_then(|v| v.strip_prefix(' '))
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| bad(line, &format!("expected `{key} <count>`, got `{text}`")))
    };

    let n = count(&mut lines, "genomes")?;
    let mut population = Vec::with_capacity(n);
    for _ in 0..n {
        let (line, text) = lines.next().ok_or(LoadError::Missing("genome"))?;
        population.push(Genome::decode(text).ok_or_else(|| bad(line, "not a genome"))?);
    }

    let n = count(&mut lines, "hall")?;
    let mut hall = Vec::with_capacity(n);
    for _ in 0..n {
        let (line, text) = lines.next().ok_or(LoadError::Missing("hall entry"))?;
        hall.push(
            text.parse::<u64>()
                .map_err(|_| bad(line, "not a ladder id"))?,
        );
    }

    let n = count(&mut lines, "ladder")?;
    let mut ladder = Ladder::default();
    for _ in 0..n {
        let (line, text) = lines.next().ok_or(LoadError::Missing("ladder entry"))?;
        let f: Vec<&str> = text.split_whitespace().collect();
        if f.len() < 8 {
            return Err(bad(line, "ladder line is too short"));
        }
        let parse_num = |s: &str, what: &str| -> Result<f64, LoadError> {
            s.parse().map_err(|_| bad(line, what))
        };
        let id: u64 = f[0].parse().map_err(|_| bad(line, "bad id"))?;
        let genome =
            Genome::decode(&f[7..].join(" ")).ok_or_else(|| bad(line, "bad genome in ladder"))?;
        ladder.restore(
            Versioned {
                id,
                generation: f[1].parse().map_err(|_| bad(line, "bad generation"))?,
                label: f[6].to_string(),
                genome,
            },
            Rating {
                mu: parse_num(f[3], "bad mu")?,
                sigma: parse_num(f[4], "bad sigma")?,
            },
            f[2].parse().map_err(|_| bad(line, "bad game count"))?,
            f[5].parse().map_err(|_| bad(line, "bad anchored count"))?,
        );
    }
    if ladder.get(ANCHOR).is_none() {
        return Err(LoadError::Missing("anchor"));
    }

    Ok(Trainer::restore(
        config, ladder, population, hall, generation, trials, run_seed,
    ))
}

/// Write a checkpoint, replacing any previous one atomically.
///
/// Via a temporary file and a rename: a crash during the write leaves the
/// previous checkpoint intact rather than a half-written one, which is the
/// whole point of having a checkpoint.
pub fn save(trainer: &Trainer, path: &Path) -> std::io::Result<()> {
    let temp = path.with_extension("tmp");
    std::fs::write(&temp, encode(trainer))?;
    std::fs::rename(&temp, path)
}

/// Read a checkpoint from disk.
pub fn load(path: &Path) -> std::io::Result<Result<Trainer, LoadError>> {
    Ok(decode(&std::fs::read_to_string(path)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quick() -> Config {
        Config {
            population: 8,
            survivors: 3,
            trials: 8,
            validation: 8,
            trials_min: 4,
            trials_max: 32,
            hall_size: 4,
            threads: 2,
            ..Config::default()
        }
    }

    #[test]
    fn a_resumed_run_continues_exactly_as_if_it_had_not_stopped() {
        // The property that makes a multi-day run worth starting. Not
        // "approximately the same" — the same games, the same champions, the
        // same ratings.
        let mut straight = Trainer::new(quick(), 4_242);
        let mut interrupted = Trainer::new(quick(), 4_242);

        for _ in 0..3 {
            straight.step();
            interrupted.step();
        }
        // Interrupt: write it out, throw it away, read it back.
        let text = encode(&interrupted);
        let Ok(mut resumed) = decode(&text) else {
            panic!("decode failed")
        };

        for _ in 0..3 {
            let a = straight.step();
            let b = resumed.step();
            assert_eq!(a.generation, b.generation);
            assert_eq!(
                a.best_fitness, b.best_fitness,
                "generation {}",
                a.generation
            );
            assert_eq!(a.games, b.games);
            assert_eq!(a.above_anchor, b.above_anchor);
            assert_eq!(a.spread, b.spread);
        }
        assert_eq!(straight.best(), resumed.best());
    }

    #[test]
    fn a_checkpoint_round_trips_every_field() {
        let mut t = Trainer::new(quick(), 7);
        t.step();
        t.step();
        let Ok(restored) = decode(&encode(&t)) else {
            panic!("decode failed")
        };

        assert_eq!(restored.run_seed, t.run_seed);
        assert_eq!(restored.generation(), t.generation());
        assert_eq!(restored.population, t.population);
        assert_eq!(restored.hall, t.hall);
        assert_eq!(restored.config.population, t.config.population);
        assert_eq!(restored.config.mode, t.config.mode);
        assert_eq!(restored.ladder.len(), t.ladder.len());
        for id in t.ladder.ids() {
            assert_eq!(restored.ladder.rating(id), t.ladder.rating(id), "id {id}");
            assert_eq!(restored.ladder.games_played(id), t.ladder.games_played(id));
            assert_eq!(
                restored.ladder.get(id).map(|v| &v.genome),
                t.ladder.get(id).map(|v| &v.genome)
            );
        }
        // Connectivity is derived from anchored counts, so it must survive too.
        assert_eq!(restored.ladder.connectivity(1), t.ladder.connectivity(1));
    }

    #[test]
    fn the_anchor_stays_pinned_across_a_resume() {
        // A restored anchor that could drift would silently invalidate every
        // cross-generation comparison in the run.
        let mut t = Trainer::new(quick(), 11);
        t.step();
        let Ok(mut restored) = decode(&encode(&t)) else {
            panic!("decode failed")
        };
        let before = restored.ladder.rating(ANCHOR).mu;
        for _ in 0..3 {
            restored.step();
        }
        assert_eq!(restored.ladder.rating(ANCHOR).mu, before);
    }

    #[test]
    fn a_checkpoint_is_readable_text() {
        let t = Trainer::new(quick(), 1);
        let text = encode(&t);
        assert!(text.starts_with("carranta-evolve 1\n"));
        assert!(text.contains("mode restricted"));
        assert!(text.contains("# population: one genome per line"));
        // Comments and blank lines are decoration, not structure.
        let stripped: String = text
            .lines()
            .filter(|l| !l.trim_start().starts_with('#'))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(decode(&stripped).is_ok());
    }

    #[test]
    fn a_damaged_checkpoint_says_where() {
        let t = Trainer::new(quick(), 2);
        let text = encode(&t);

        assert!(decode("").is_err_and(|e| e == LoadError::Missing("header")));
        assert!(decode("carranta-evolve 99\n").is_err_and(|e| e == LoadError::Version(99)));
        // A mangled scalar names its line.
        let broken = text.replacen("trials ", "trials x", 1);
        match decode(&broken) {
            Err(LoadError::Malformed { line, .. }) => assert!(line > 0),
            Err(other) => panic!("expected a malformed error, got {other:?}"),
            Ok(_) => panic!("a mangled scalar was accepted"),
        }
        // A truncated file is missing, not malformed.
        let truncated: String = text.lines().take(6).collect::<Vec<_>>().join("\n");
        assert!(decode(&truncated).is_err_and(|e| matches!(e, LoadError::Missing(_))));
    }

    #[test]
    fn saving_is_atomic() {
        let dir = std::env::temp_dir().join("carranta-evolve-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("run.ckpt");
        let mut t = Trainer::new(quick(), 3);
        t.step();

        save(&t, &path).unwrap();
        assert!(path.exists());
        // The temporary is gone: a rename, not a copy.
        assert!(!path.with_extension("tmp").exists());

        let Ok(loaded) = load(&path).unwrap() else {
            panic!("decode failed")
        };
        assert_eq!(loaded.generation(), t.generation());
        std::fs::remove_file(&path).ok();
    }
}
