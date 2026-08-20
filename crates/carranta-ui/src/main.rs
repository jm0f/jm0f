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
  --seed N       board seed (1)
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
    let mut seed: u64 = 1;
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
            "--seed" => seed = value().parse().unwrap_or(seed),
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
    if let Some(dir) = bots.filter(|d| d.exists()) {
        match std::fs::read_dir(&dir) {
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
            let Some((net, generation)) = carranta_bot::net::Net::parse(&text) else {
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
            champions.push(carranta_ui::server::Champion { generation, net });
        }
        server = server.with_champions(champions);
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
