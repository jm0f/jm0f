//! `carranta-play`, a local board in a browser.
//!
//! ```text
//! cargo run --release -p carranta-ui
//! cargo run --release -p carranta-ui -- --port 9000 --seats 3 --mode restricted
//! ```

use std::net::TcpListener;

use carranta_core::state::TradeMode;
use carranta_ui::Server;

const USAGE: &str = "\
carranta-play, play Carranta locally in a browser

  --port N       port to listen on (8181)
  --seats N      3 or 4 (4)
  --seed N       board seed for the first table (random unless given)
  --mode MODE    full | restricted | disabled (full)
  --games DIR    where games are kept (./games)
  --demo N       have at least N finished games to look at (plays what is missing)
  --bots DIR     load every .net in DIR as a champion a chair can be given
  --trained FILE the same for one file; repeat for several

Champions are offered, not seated: a chair plays the house bot until a lobby
asks for one, and every game file records which player sat where.

Binds 127.0.0.1 by default: on a laptop the game is local and stays local.
Set PORT (as every host does) to bind 0.0.0.0 on that port instead, or HOST to
say exactly which address.";

fn main() {
    let mut port: u16 = 8181;
    let mut seats: u8 = 4;
    let mut seed: Option<u64> = None;
    let mut mode = TradeMode::Full;
    // Beside the binary by default, so a game played today is still there
    // tomorrow without anyone having to say where to put it.
    let mut games = std::path::PathBuf::from("games");
    let mut demo: u32 = 0;
    let mut trained: Vec<std::path::PathBuf> = Vec::new();
    let mut bots: Option<std::path::PathBuf> = None;

    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let mut value = || args.next().unwrap_or_default();
        match flag.as_str() {
            "--port" => port = value().parse().unwrap_or(port),
            "--seats" => seats = value().parse().unwrap_or(seats),
            "--seed" => seed = value().parse().ok().or(seed),
            "--games" => games = std::path::PathBuf::from(value()),
            "--demo" => demo = value().parse().unwrap_or(demo),
            "--trained" => trained.push(std::path::PathBuf::from(value())),
            "--bots" => bots = Some(std::path::PathBuf::from(value())),
            "--mode" => {
                mode = match value().as_str() {
                    "disabled" => TradeMode::Disabled,
                    "restricted" => TradeMode::Restricted,
                    _ => TradeMode::Full,
                }
            }
            "--help" | "-h" => {
                println!("{USAGE}");
                return;
            }
            other => {
                eprintln!("unknown option `{other}`\n\n{USAGE}");
                std::process::exit(2);
            }
        }
    }

    // Loopback unless something asks otherwise, and the something is a platform
    // rather than a flag: `PORT` is how every host here says which port it has
    // routed to this process, and its presence is a reliable sign of being one
    // of them. On a laptop neither is set and this stays what it has always
    // been, a local tool bound to loopback.
    //
    // `HOST` overrides both, for the case neither of those covers.
    let hosted = std::env::var("PORT").ok().and_then(|v| v.parse().ok());
    if let Some(p) = hosted {
        port = p;
    }
    let host = std::env::var("HOST").unwrap_or_else(|_| {
        if hosted.is_some() {
            "0.0.0.0"
        } else {
            "127.0.0.1"
        }
        .to_string()
    });
    let listener = match TcpListener::bind((host.as_str(), port)) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("cannot listen on {host}:{port}: {e}");
            std::process::exit(1);
        }
    };
    println!(
        "Carranta {}, listening on {host}:{port}",
        carranta_ui::stamp::build()
    );
    // Random unless somebody asked for a particular one: a fixed default
    // meant the first table after every restart was the same board, which a
    // fresh deployment turned into the same opening game for everybody. The
    // clock is entropy enough for a board seed, mixed so consecutive restarts
    // differ in more than their low bits, and passing --seed still gives the
    // exact reproducibility it always did.
    let seed = seed.unwrap_or_else(|| {
        let mut x = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() as u64)
            ^ (std::process::id() as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        x ^= x >> 33;
        x = x.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
        x ^= x >> 33;
        x
    });
    println!("  {seats} seats, {mode:?} market, seed {seed}");
    let mut server = Server::new(seats, seed, mode, &games);
    // Every champion this server can offer: the committed directory, then any
    // named outright. A champion that cannot be read stops the server rather
    // than being skipped, because whoever passed the flag wanted that player,
    // and a lobby quietly missing one of its choices is worse than a refusal.
    let mut files = trained;
    // A directory that is not there is no champions, not a failure: an empty
    // catalogue is the state every build was in before the first one existed,
    // and a laptop running without one should still start.
    let bots = bots.filter(|d| d.exists());
    if let Some(dir) = &bots {
        match std::fs::read_dir(dir) {
            Ok(entries) => {
                let mut found: Vec<_> = entries
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| p.extension().is_some_and(|e| e == "net"))
                    .collect();
                // Read in a fixed order so the lobby offers the same list on
                // every restart, whatever order the filesystem hands them back.
                found.sort();
                files.extend(found);
            }
            Err(e) => {
                eprintln!("cannot read {}: {e}", dir.display());
                std::process::exit(1);
            }
        }
    }
    if !files.is_empty() {
        let mut champions: Vec<carranta_ui::server::Champion> = Vec::new();
        for path in &files {
            let text = match std::fs::read_to_string(path) {
                Ok(text) => text,
                Err(e) => {
                    eprintln!("cannot read {}: {e}", path.display());
                    std::process::exit(1);
                }
            };
            let Some((net, generation, run)) = carranta_bot::net::Net::parse_meta(&text) else {
                eprintln!("{} is not a champion network file", path.display());
                std::process::exit(1);
            };
            // Two files of one generation are one player named twice, and a
            // lobby offering it twice would present self-play as a matchup.
            if champions.iter().any(|c| c.generation == generation) {
                eprintln!(
                    "{} is trained@{generation}, which is already loaded: champions are told \
                     apart by their generation, so two of one are not two players",
                    path.display()
                );
                std::process::exit(1);
            }
            champions.push(carranta_ui::server::Champion {
                generation,
                net,
                run,
            });
        }
        server = server.with_champions(champions);
        // Which of them an empty chair plays. Declared in a file beside them
        // rather than worked out from the catalogue, because the catalogue
        // cannot answer it: the strongest champion is the answer to a
        // measurement somebody ran, and a higher generation number is not the
        // same claim. A named generation that is not loaded stops the server,
        // for the same reason an unreadable champion file does.
        if let Some(dir) = &bots {
            if let Some(generation) = flagship_declared(&dir.join("FLAGSHIP")) {
                let (s, known) = server.with_flagship(generation);
                server = s;
                if !known {
                    eprintln!(
                        "{} names trained@{generation} as the strongest champion, which is not \
                         loaded: a table dealt without a choice would quietly get the house bot \
                         instead",
                        dir.join("FLAGSHIP").display()
                    );
                    std::process::exit(1);
                }
                println!("  empty chairs play trained@{generation}");
            }
        }
        let named: Vec<String> = server.roster().into_iter().map(|(id, _)| id).collect();
        println!("  chairs can be played by {}", named.join(", "));
    }
    // Leaked on purpose: this server lives until the process ends, and the
    // connection threads borrow it for as long as they run. A leak with the
    // lifetime of the program is the honest way to say so.
    let server: &'static Server = Box::leak(Box::new(server));
    println!("  games in {}", server.store().dir().display());
    // Played before the door opens, so the addresses are printed beside the
    // one you would open anyway.
    for id in server.demo(demo) {
        println!("  played /{id}/analytics");
    }
    server.serve(listener);
}

/// The generation a `FLAGSHIP` file names, if the file is there and says one.
///
/// Blank lines and `#` comments are skipped, so the file can carry the
/// measurement that earned the place beside the number that took it. No file
/// is not an error: a catalogue nobody has ranked yet is the state every build
/// was in before the first champion was measured past the house bot.
fn flagship_declared(path: &std::path::Path) -> Option<u32> {
    let text = std::fs::read_to_string(path).ok()?;
    text.lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with('#'))
        .and_then(|l| l.split_whitespace().next())
        .and_then(|w| w.trim_start_matches("trained@").parse().ok())
}

#[cfg(test)]
mod tests {
    use super::flagship_declared;

    /// Write one throwaway file and read it back through the parser.
    fn declared(body: &str) -> Option<u32> {
        let path = std::env::temp_dir().join(format!(
            "carranta-flagship-{}-{}",
            std::process::id(),
            body.len()
        ));
        std::fs::write(&path, body).expect("a temporary file");
        let got = flagship_declared(&path);
        let _ = std::fs::remove_file(&path);
        got
    }

    #[test]
    fn a_declaration_is_a_generation_under_whatever_earned_it() {
        // The file is worth reading as well as parsing: the measurement that
        // won the chair belongs beside the number that took it.
        assert_eq!(declared("1369\n"), Some(1369));
        assert_eq!(
            declared("# vs 1526: -0.159, 55.5%\n\n1369\n"),
            Some(1369),
            "comments and blank lines carry the reasoning, not the answer"
        );
        assert_eq!(
            declared("trained@1369\n"),
            Some(1369),
            "the agent spelling is the same claim"
        );
        // Nothing to say is not the same as saying something wrong: no file
        // and an empty file both leave the house bot in the chair, while a
        // file that says something unreadable is a mistake worth surfacing as
        // absent rather than guessing a generation out of.
        assert_eq!(declared("# nothing but a note\n"), None);
        assert_eq!(declared(""), None);
        assert_eq!(declared("strongest\n"), None);
        assert_eq!(
            flagship_declared(&std::env::temp_dir().join("carranta-flagship-absent")),
            None
        );
    }
}
