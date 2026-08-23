//! Saving and resuming a run.
//!
//! A multi-day run on a laptop will be interrupted, a lid closed, a reboot, a
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
use crate::neat::{Innovations, NeatGenome, Params, Species};
use crate::train::{Config, Trainer};
use crate::train_neat::{NeatConfig, NeatTrainer};

/// Bumped when the format changes in a way an older reader would misread.
pub const FORMAT: u32 = 1;

/// Phase two writes its own format: the genomes are multi-line, the species
/// carry state, and reading one as the other must fail loudly, not weirdly.
pub const NEAT_FORMAT: u32 = 7;

/// The NEAT format before the pinned field members existed (E-26): a
/// selection line and sampling weights, but every outsider still rode the
/// rolling hall, so it reads back with nothing pinned.
pub const NEAT_FORMAT_NO_PINS: u32 = 6;

/// The NEAT format before the selection line existed (E-20 to E-24): payoff
/// table, but classic evaluation only, so it reads back with every selection
/// flag off and resumes the run it was.
pub const NEAT_FORMAT_CLASSIC_SELECTION: u32 = 5;

/// The NEAT format before the ask allowance existed (E-15).
///
/// Still readable: a format 2 run trained in the uncapped market, and reading
/// it back with the allowance at the rules cap resumes exactly the run it
/// was, which is what a resume is for. Writing always uses the current
/// format.
pub const NEAT_FORMAT_UNCAPPED: u32 = 2;

/// The NEAT format before the payoff table existed (E-19): win bonus, no
/// `payoff` line.
pub const NEAT_FORMAT_FLAT_WIN: u32 = 4;

/// The NEAT format before the win bonus existed (E-17).
///
/// Same principle: formats 2 and 3 both selected on finishing position alone,
/// so they read back with the bonus at zero and resume the runs they were.
pub const NEAT_FORMAT_POSITION_ONLY: u32 = 3;

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

/// Render a phase-two run as text.
pub fn encode_neat(trainer: &NeatTrainer) -> String {
    let c = &trainer.config;
    let p = &c.params;
    let mut out = String::new();
    let _ = writeln!(out, "carranta-evolve {NEAT_FORMAT}");
    let _ = writeln!(out, "method neat");
    let _ = writeln!(out, "run_seed {}", trainer.run_seed);
    let _ = writeln!(out, "generation {}", trainer.generation);
    let _ = writeln!(out, "trials {}", trainer.trials);
    let _ = writeln!(out, "population {}", c.population);
    let _ = writeln!(out, "validation {}", c.validation);
    let _ = writeln!(out, "trials_min {}", c.trials_min);
    let _ = writeln!(out, "trials_max {}", c.trials_max);
    let _ = writeln!(out, "hall_seats {}", c.hall_seats);
    let _ = writeln!(out, "hall_size {}", c.hall_size);
    let _ = writeln!(out, "sample {}", c.sample);
    let _ = writeln!(
        out,
        "give_cap {}",
        c.give_cap.map_or("hand".to_string(), |n| n.to_string())
    );
    let _ = writeln!(out, "want_cap {}", c.want_cap);
    let _ = writeln!(out, "ask_cap {}", c.ask_cap);
    let _ = writeln!(out, "win_bonus {}", c.win_bonus);
    match c.payoff {
        Some([a, b, cc, d]) => {
            let _ = writeln!(out, "payoff {a} {b} {cc} {d}");
        }
        None => {
            let _ = writeln!(out, "payoff none");
        }
    }
    // The selection line: which evaluation refinements the run trains under,
    // as words, or `classic` for none. A resume must evaluate the way the
    // run always did, so these are state, not flags to re-pass.
    let mut selection = String::new();
    for (on, word) in [
        (c.margin, "margin"),
        (c.halving, "halving"),
        (c.pfsp, "pfsp"),
        (c.rotate, "rotate"),
        (c.held_out_anchor, "held-out-anchor"),
        (c.deep_eval, "deep-eval"),
    ] {
        if on {
            if !selection.is_empty() {
                selection.push(' ');
            }
            selection.push_str(word);
        }
    }
    if selection.is_empty() {
        selection.push_str("classic");
    }
    let _ = writeln!(out, "selection {selection}");
    let _ = writeln!(out, "cap {}", c.cap);
    let _ = writeln!(out, "mode {}", mode_name(c.mode));
    // Display for f64 emits the shortest string that parses back to the same
    // value, so every one of these survives the round trip exactly.
    let _ = writeln!(out, "delta {}", trainer.delta);
    let _ = writeln!(out, "champion {}", trainer.champion);
    let _ = writeln!(out, "next_innov {}", trainer.inn.next_innov);
    let _ = writeln!(out, "next_node {}", trainer.inn.next_node);
    for (k, v) in [
        ("weight_p", p.weight_p),
        ("perturb_p", p.perturb_p),
        ("power", p.power),
        ("fresh", p.fresh),
        ("add_conn_p", p.add_conn_p),
        ("add_node_p", p.add_node_p),
        ("c1", p.c1),
        ("c2", p.c2),
        ("c3", p.c3),
        ("delta_start", p.delta_start),
        ("delta_step", p.delta_step),
        ("delta_floor", p.delta_floor),
        ("keep_disabled_p", p.keep_disabled_p),
    ] {
        let _ = writeln!(out, "{k} {v}");
    }
    let _ = writeln!(out, "target_species {}", p.target_species);
    let _ = writeln!(out, "stagnation {}", p.stagnation);

    let _ = writeln!(out, "\n# population: one genome block each");
    let _ = writeln!(out, "genomes {}", trainer.population.len());
    for g in &trainer.population {
        let _ = writeln!(out, "genome");
        out.push_str(&g.show());
        let _ = writeln!(out, "end");
    }

    let _ = writeln!(out, "\n# species: representative, best-ever, staleness");
    let _ = writeln!(out, "species {}", trainer.species.len());
    for s in &trainer.species {
        let _ = writeln!(out, "rep {} {}", s.best, s.stale);
        out.push_str(&s.rep.show());
        let _ = writeln!(out, "end");
    }

    let _ = writeln!(out, "\n# hall of fame: ladder ids, oldest first");
    let _ = writeln!(out, "hall {}", trainer.hall.len());
    for id in &trainer.hall {
        let _ = writeln!(out, "{id}");
    }

    // The pinned field members (E-26): outsiders enrolled to stay, which the
    // hall's eviction never touches.
    let _ = writeln!(out, "\n# pinned field members: ladder ids");
    let _ = writeln!(out, "pinned {}", trainer.pinned.len());
    for id in &trainer.pinned {
        let _ = writeln!(out, "{id}");
    }

    // PFSP sampling weights (E-22), id then weight, sorted by id so the
    // encoding is a pure function of the state. Without them a resume would
    // sample the hall uniformly for one generation and diverge from the run
    // it claims to continue.
    let mut weights: Vec<(u64, f64)> = trainer.hall_weight.iter().map(|(&k, &v)| (k, v)).collect();
    weights.sort_unstable_by_key(|&(id, _)| id);
    let _ = writeln!(out, "\n# hall sampling weights: id, weight");
    let _ = writeln!(out, "weights {}", weights.len());
    for (id, w) in weights {
        let _ = writeln!(out, "{id} {w}");
    }

    let _ = writeln!(out, "\n# ladder: one version block each");
    let mut ids = trainer.ladder.ids();
    ids.sort_unstable();
    let _ = writeln!(out, "ladder {}", ids.len());
    for id in ids {
        let v = trainer.ladder.get(id).expect("id came from the ladder");
        let r = trainer.ladder.rating(id);
        let _ = writeln!(
            out,
            "version {} {} {} {} {} {} {}",
            v.id,
            v.generation,
            trainer.ladder.games_played(id),
            r.mu,
            r.sigma,
            trainer.ladder.anchored_games(id),
            v.label,
        );
        out.push_str(&v.genome.show());
        let _ = writeln!(out, "end");
    }
    out
}

/// Read a phase-two run back.
pub fn decode_neat(text: &str) -> Result<NeatTrainer, LoadError> {
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
    if !matches!(
        version,
        NEAT_FORMAT
            | NEAT_FORMAT_NO_PINS
            | NEAT_FORMAT_CLASSIC_SELECTION
            | NEAT_FORMAT_FLAT_WIN
            | NEAT_FORMAT_UNCAPPED
            | NEAT_FORMAT_POSITION_ONLY
    ) {
        return Err(LoadError::Version(version));
    }
    let (line, method) = lines.next().ok_or(LoadError::Missing("method"))?;
    if method != "method neat" {
        return Err(bad(line, "this format requires `method neat`"));
    }

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
    let population_n: usize = num!("population");
    let validation: u32 = num!("validation");
    let trials_min: u32 = num!("trials_min");
    let trials_max: u32 = num!("trials_max");
    let hall_seats: usize = num!("hall_seats");
    let hall_size: usize = num!("hall_size");
    let sample: u32 = num!("sample");
    let give_cap = {
        let (line, v) = scalar("give_cap")?;
        if v == "hand" {
            None
        } else {
            Some(v.parse().map_err(|_| bad(line, "bad give_cap"))?)
        }
    };
    let want_cap: u8 = num!("want_cap");
    // Format 2 predates the allowance and trained uncapped; reading it back
    // at the rules cap resumes exactly the run it was.
    let ask_cap: u8 = if version == NEAT_FORMAT_UNCAPPED {
        carranta_core::state::OFFERS_PER_TURN
    } else {
        num!("ask_cap")
    };
    // Formats 2 and 3 selected on finishing position alone, which is the
    // bonus at zero.
    let win_bonus: f64 = if version >= NEAT_FORMAT_FLAT_WIN {
        num!("win_bonus")
    } else {
        0.0
    };
    // Older formats had no table; the bonus above is their whole story.
    let payoff: Option<[f64; 4]> = if version >= NEAT_FORMAT_CLASSIC_SELECTION {
        let (line, v) = scalar("payoff")?;
        if v == "none" {
            None
        } else {
            let parts: Vec<f64> = v.split(' ').filter_map(|x| x.parse().ok()).collect();
            match parts[..] {
                [a, b, c, d] => Some([a, b, c, d]),
                _ => return Err(bad(line, "payoff wants `none` or four numbers")),
            }
        }
    } else {
        None
    };
    // Format 5 and earlier evaluated the classic way; the selection line
    // (E-20 to E-24) says which refinements a format 6 run trains under.
    let (margin, halving, pfsp, rotate, held_out_anchor, deep_eval) =
        if version >= NEAT_FORMAT_NO_PINS {
            let (line, v) = scalar("selection")?;
            let mut flags = (false, false, false, false, false, false);
            for word in v.split(' ') {
                match word {
                    "classic" => {}
                    "margin" => flags.0 = true,
                    "halving" => flags.1 = true,
                    "pfsp" => flags.2 = true,
                    "rotate" => flags.3 = true,
                    "held-out-anchor" => flags.4 = true,
                    "deep-eval" => flags.5 = true,
                    other => {
                        return Err(bad(line, &format!("unknown selection word `{other}`")));
                    }
                }
            }
            flags
        } else {
            (false, false, false, false, false, false)
        };
    let cap: usize = num!("cap");
    let mode = {
        let (line, v) = scalar("mode")?;
        mode_from(&v).ok_or_else(|| bad(line, &format!("unknown trade mode `{v}`")))?
    };
    let delta: f64 = num!("delta");
    let champion: u64 = num!("champion");
    let next_innov: u32 = num!("next_innov");
    let next_node: u32 = num!("next_node");
    let params = Params {
        weight_p: num!("weight_p"),
        perturb_p: num!("perturb_p"),
        power: num!("power"),
        fresh: num!("fresh"),
        add_conn_p: num!("add_conn_p"),
        add_node_p: num!("add_node_p"),
        c1: num!("c1"),
        c2: num!("c2"),
        c3: num!("c3"),
        delta_start: num!("delta_start"),
        delta_step: num!("delta_step"),
        delta_floor: num!("delta_floor"),
        keep_disabled_p: num!("keep_disabled_p"),
        target_species: num!("target_species"),
        stagnation: num!("stagnation"),
    };
    let config = NeatConfig {
        population: population_n,
        trials,
        validation,
        trials_min,
        trials_max,
        hall_seats,
        hall_size,
        sample,
        threads: NeatConfig::default().threads,
        give_cap,
        want_cap,
        ask_cap,
        win_bonus,
        payoff,
        cap,
        mode,
        params,
        margin,
        halving,
        pfsp,
        rotate,
        held_out_anchor,
        deep_eval,
    };

    // A genome block: `gene` lines up to `end`.
    let genome_block =
        |lines: &mut dyn Iterator<Item = (usize, &str)>| -> Result<NeatGenome, LoadError> {
            let mut genes = Vec::new();
            loop {
                let (line, text) = lines.next().ok_or(LoadError::Missing("end of genome"))?;
                if text == "end" {
                    break;
                }
                let rest = text
                    .strip_prefix("gene ")
                    .ok_or_else(|| bad(line, "expected a gene line"))?;
                genes.push(NeatGenome::parse_gene(rest).ok_or_else(|| bad(line, "bad gene"))?);
            }
            Ok(NeatGenome { genes })
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
        if text != "genome" {
            return Err(bad(line, "expected `genome`"));
        }
        population.push(genome_block(&mut lines)?);
    }

    let n = count(&mut lines, "species")?;
    let mut species = Vec::with_capacity(n);
    for _ in 0..n {
        let (line, text) = lines.next().ok_or(LoadError::Missing("species"))?;
        let rest = text
            .strip_prefix("rep ")
            .ok_or_else(|| bad(line, "expected `rep <best> <stale>`"))?;
        let mut p = rest.split_whitespace();
        let best: f64 = p
            .next()
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| bad(line, "bad species best"))?;
        let stale: u32 = p
            .next()
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| bad(line, "bad species staleness"))?;
        let rep = genome_block(&mut lines)?;
        species.push(Species {
            rep,
            members: Vec::new(),
            best,
            stale,
        });
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

    // The pinned field members (E-26), format 7 on; older files pinned
    // nothing, so nothing is what they read back with.
    let mut pinned = Vec::new();
    if version >= NEAT_FORMAT {
        let n = count(&mut lines, "pinned")?;
        for _ in 0..n {
            let (line, text) = lines.next().ok_or(LoadError::Missing("pinned entry"))?;
            pinned.push(
                text.parse::<u64>()
                    .map_err(|_| bad(line, "not a ladder id"))?,
            );
        }
    }

    // The sampling weights (E-22), format 6 on. Older files have none, and
    // resume with a uniform hall for one generation, which is what they did.
    let mut hall_weight = std::collections::HashMap::new();
    if version >= NEAT_FORMAT_NO_PINS {
        let n = count(&mut lines, "weights")?;
        for _ in 0..n {
            let (line, text) = lines.next().ok_or(LoadError::Missing("weight entry"))?;
            let mut f = text.split_whitespace();
            let id: u64 = f
                .next()
                .and_then(|v| v.parse().ok())
                .ok_or_else(|| bad(line, "bad weight id"))?;
            let w: f64 = f
                .next()
                .and_then(|v| v.parse().ok())
                .ok_or_else(|| bad(line, "bad weight"))?;
            hall_weight.insert(id, w);
        }
    }

    let n = count(&mut lines, "ladder")?;
    let mut ladder = Ladder::with_anchor(
        carranta_analytics::rating::Model::default(),
        NeatGenome::default(),
        "heuristic-v1",
    );
    for _ in 0..n {
        let (line, text) = lines.next().ok_or(LoadError::Missing("ladder entry"))?;
        let rest = text
            .strip_prefix("version ")
            .ok_or_else(|| bad(line, "expected a version line"))?;
        let f: Vec<&str> = rest.split_whitespace().collect();
        if f.len() < 7 {
            return Err(bad(line, "version line is too short"));
        }
        let genome = genome_block(&mut lines)?;
        let id: u64 = f[0].parse().map_err(|_| bad(line, "bad id"))?;
        ladder.restore(
            Versioned {
                id,
                generation: f[1].parse().map_err(|_| bad(line, "bad generation"))?,
                label: f[6].to_string(),
                genome,
            },
            Rating {
                mu: f[3].parse().map_err(|_| bad(line, "bad mu"))?,
                sigma: f[4].parse().map_err(|_| bad(line, "bad sigma"))?,
            },
            f[2].parse().map_err(|_| bad(line, "bad game count"))?,
            f[5].parse().map_err(|_| bad(line, "bad anchored count"))?,
        );
    }
    if ladder.get(ANCHOR).is_none() {
        return Err(LoadError::Missing("anchor"));
    }

    let mut trainer = NeatTrainer::restore(
        config,
        ladder,
        population,
        species,
        delta,
        Innovations::restore(next_innov, next_node),
        hall,
        generation,
        trials,
        run_seed,
        champion,
    );
    trainer.hall_weight = hall_weight;
    trainer.pinned = pinned;
    Ok(trainer)
}

/// Write a phase-two checkpoint atomically, like [`save`].
pub fn save_neat(trainer: &NeatTrainer, path: &Path) -> std::io::Result<()> {
    let temp = path.with_extension("tmp");
    std::fs::write(&temp, encode_neat(trainer))?;
    std::fs::rename(&temp, path)
}

/// Read a phase-two checkpoint from disk.
pub fn load_neat(path: &Path) -> std::io::Result<Result<NeatTrainer, LoadError>> {
    Ok(decode_neat(&std::fs::read_to_string(path)?))
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

    fn quick_neat() -> crate::train_neat::NeatConfig {
        crate::train_neat::NeatConfig {
            population: 6,
            trials: 4,
            validation: 4,
            trials_min: 4,
            trials_max: 16,
            hall_size: 4,
            sample: 0,
            threads: 2,
            cap: 400,
            mode: TradeMode::Disabled,
            ..crate::train_neat::NeatConfig::default()
        }
    }

    #[test]
    fn a_resumed_neat_run_continues_exactly_as_if_it_had_not_stopped() {
        // The same promise phase one makes, and harder to keep: a NEAT
        // checkpoint carries topologies, species state and the innovation
        // counters, and losing any of them shows up as a silent divergence
        // generations later rather than as an error.
        let mut straight = NeatTrainer::new(quick_neat(), 4_242);
        let mut interrupted = NeatTrainer::new(quick_neat(), 4_242);

        for _ in 0..3 {
            straight.step();
            interrupted.step();
        }
        let text = encode_neat(&interrupted);
        let Ok(mut resumed) = decode_neat(&text) else {
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
            assert_eq!(a.gap, b.gap);
            assert_eq!(a.species, b.species);
            assert_eq!(a.champion_genes, b.champion_genes);
        }
        assert_eq!(straight.population, resumed.population);
    }

    #[test]
    fn a_neat_checkpoint_round_trips_every_field() {
        let mut t = NeatTrainer::new(quick_neat(), 7);
        t.step();
        t.step();
        let Ok(restored) = decode_neat(&encode_neat(&t)) else {
            panic!("decode failed")
        };
        assert_eq!(restored.run_seed, t.run_seed);
        assert_eq!(restored.generation(), t.generation());
        assert_eq!(restored.population, t.population);
        assert_eq!(restored.hall, t.hall);
        assert_eq!(restored.delta, t.delta);
        assert_eq!(restored.champion, t.champion);
        assert_eq!(restored.inn.next_innov, t.inn.next_innov);
        assert_eq!(restored.inn.next_node, t.inn.next_node);
        assert_eq!(restored.species.len(), t.species.len());
        for (a, b) in restored.species.iter().zip(&t.species) {
            assert_eq!(a.rep, b.rep);
            assert_eq!(a.best, b.best);
            assert_eq!(a.stale, b.stale);
        }
        for id in t.ladder.ids() {
            assert_eq!(restored.ladder.rating(id), t.ladder.rating(id), "id {id}");
            assert_eq!(
                restored.ladder.get(id).map(|v| &v.genome),
                t.ladder.get(id).map(|v| &v.genome)
            );
        }
        // And the champion is exportable after a resume, which is the file a
        // deployment reads.
        assert!(restored.champion_genome().is_some());
    }

    #[test]
    fn a_pinned_outsider_survives_the_round_trip_and_the_hall() {
        // An enrolled exploiter is pinned (E-26): it survives the checkpoint
        // round trip, and it survives the hall's eviction, which is the whole
        // reason the pinned list exists.
        let config = crate::train_neat::NeatConfig {
            hall_size: 2,
            ..quick_neat()
        };
        let mut t = NeatTrainer::new(config, 21);
        let outsider = NeatGenome::default();
        let id = t.seed_baseline(outsider, 642);
        for _ in 0..4 {
            t.step();
        }
        assert!(t.pinned.contains(&id), "pinned through four evictions");
        assert!(!t.hall.contains(&id), "and never in the rolling hall");
        assert!(t.hall.len() <= 2, "which kept its own size");
        let restored = decode_neat(&encode_neat(&t)).expect("it reads back");
        assert_eq!(restored.pinned, t.pinned, "the pin survives the trip");

        // And a pin can be lifted by the generation it was enrolled under,
        // leaving the ladder's record intact.
        let mut t2 = restored;
        assert_eq!(t2.unpin(999), 0, "nothing of that generation");
        assert_eq!(t2.unpin(642), 1, "the exploiter leaves the field");
        assert!(t2.pinned.is_empty());
        assert!(t2.ladder.get(id).is_some(), "the ladder remembers it");

        // With the anchor held out, the freed featured seat belongs to the
        // pinned members: the outsider is met every generation, which shows
        // up as a sampling weight learned for it.
        let refined = crate::train_neat::NeatConfig {
            hall_size: 2,
            margin: true,
            halving: true,
            pfsp: true,
            rotate: true,
            held_out_anchor: true,
            ..quick_neat()
        };
        let mut t = NeatTrainer::new(refined, 22);
        let id = t.seed_baseline(NeatGenome::default(), 642);
        t.step();
        assert!(
            t.hall_weight.contains_key(&id),
            "the pinned outsider was met in the field"
        );
    }

    #[test]
    fn a_refined_run_round_trips_its_selection_and_resumes_exactly() {
        // The refined flags (E-20 to E-24) are how the run evaluates, so
        // losing one across a resume would silently change the objective.
        // Round trip first, then the exact-continuation promise under the
        // full bundle.
        let refined = crate::train_neat::NeatConfig {
            margin: true,
            halving: true,
            pfsp: true,
            rotate: true,
            held_out_anchor: true,
            deep_eval: true,
            ..quick_neat()
        };
        let mut straight = NeatTrainer::new(refined, 4_242);
        let mut interrupted = NeatTrainer::new(refined, 4_242);
        for _ in 0..3 {
            straight.step();
            interrupted.step();
        }
        let text = encode_neat(&interrupted);
        assert!(
            text.contains("\nselection margin halving pfsp rotate held-out-anchor deep-eval\n"),
            "the selection line names every refinement"
        );
        let Ok(mut resumed) = decode_neat(&text) else {
            panic!("decode failed")
        };
        let c = &resumed.config;
        assert!(
            c.margin && c.halving && c.pfsp && c.rotate && c.held_out_anchor && c.deep_eval,
            "all six flags survive the trip"
        );
        for _ in 0..2 {
            let a = straight.step();
            let b = resumed.step();
            assert_eq!(a.generation, b.generation);
            assert_eq!(
                a.best_fitness, b.best_fitness,
                "generation {}",
                a.generation
            );
            assert_eq!(a.games, b.games);
            assert_eq!(a.champion_genes, b.champion_genes);
        }
    }

    #[test]
    fn the_two_formats_refuse_each_other() {
        // A phase-one checkpoint read as phase two, or the reverse, must be a
        // version error, never a half-parsed trainer.
        let es = Trainer::new(quick(), 1);
        assert!(matches!(
            decode_neat(&encode(&es)),
            Err(LoadError::Version(1))
        ));
        let mut neat = NeatTrainer::new(quick_neat(), 1);
        neat.step();
        assert!(matches!(
            decode(&encode_neat(&neat)),
            Err(LoadError::Version(NEAT_FORMAT))
        ));
    }

    #[test]
    fn an_older_checkpoint_resumes_as_the_run_it_was() {
        // Format 2 predates the ask allowance (E-15) and format 3 the win
        // bonus (E-17). Those runs trained in the uncapped market and
        // selected on position alone, so reading one back must restore what
        // it had, and never today's training defaults: a resume that quietly
        // changed the market or the objective would be a different run
        // wearing the same directory.
        let mut t = NeatTrainer::new(quick_neat(), 9);
        t.step();
        let modern = encode_neat(&t);
        assert!(
            modern.contains("\nask_cap 3\n"),
            "the current format writes"
        );
        assert!(modern.contains("\nwin_bonus 1\n"), "both of them");
        assert!(
            modern.contains("\npayoff none\n"),
            "and the table's absence"
        );

        // Peel the format ladder one rung at a time, dropping the section
        // each rung added.
        let strip = |text: &str, key: &str| -> String {
            let mut out = String::new();
            let mut skip = 0usize;
            for l in text.lines() {
                let t = l.trim();
                if skip > 0 && !t.is_empty() && !t.starts_with('#') {
                    skip -= 1;
                    continue;
                }
                if let Some(n) = t.strip_prefix(key) {
                    skip = n.parse().expect("a section count");
                    continue;
                }
                out.push_str(l);
                out.push('\n');
            }
            out
        };

        // The same file as format 6 wrote it: version 6, no pinned section.
        let six = strip(&modern, "pinned ").replace("carranta-evolve 7", "carranta-evolve 6");
        let back = decode_neat(&six).expect("a format six file still reads");
        assert!(back.pinned.is_empty(), "nothing pinned, as it was");

        // The same file as format 5 wrote it: version 5, no selection line
        // and no weights section.
        let five = strip(&six, "weights ")
            .replace("carranta-evolve 6", "carranta-evolve 5")
            .replace("selection classic\n", "");
        let back = decode_neat(&five).expect("a format five file still reads");
        assert!(
            !back.config.margin && !back.config.halving && !back.config.rotate,
            "classic evaluation, as it was"
        );
        assert_eq!(back.config.payoff, None, "and the table it really had");

        // The same file as format 4 wrote it: version 4, no payoff line.
        let four = five
            .replace("carranta-evolve 5", "carranta-evolve 4")
            .replace("payoff none\n", "");
        let back = decode_neat(&four).expect("a format four file still reads");
        assert_eq!(back.config.payoff, None, "no table, as it was");
        assert_eq!(back.config.win_bonus, 1.0, "the bonus it really had");

        // The same file as format 3 wrote it: version 3, no win_bonus line.
        let three = four
            .replace("carranta-evolve 4", "carranta-evolve 3")
            .replace("win_bonus 1\n", "");
        let back = decode_neat(&three).expect("a format three file still reads");
        assert_eq!(back.config.win_bonus, 0.0, "position alone, as it was");
        assert_eq!(back.config.ask_cap, 3, "and the cap it really had");
        assert_eq!(back.generation(), t.generation(), "the same run");

        // And as format 2 wrote it: no ask_cap either.
        let two = three
            .replace("carranta-evolve 3", "carranta-evolve 2")
            .replace("ask_cap 3\n", "");
        let back = decode_neat(&two).expect("a format two file still reads");
        assert_eq!(
            back.config.ask_cap,
            carranta_core::state::OFFERS_PER_TURN,
            "an uncapped run resumes uncapped"
        );
        assert_eq!(back.config.win_bonus, 0.0);
        assert_eq!(back.generation(), t.generation());
    }

    #[test]
    fn a_resumed_run_continues_exactly_as_if_it_had_not_stopped() {
        // The property that makes a multi-day run worth starting. Not
        // "approximately the same". The same games, the same champions, the
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
