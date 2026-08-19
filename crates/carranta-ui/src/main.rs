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
  --trained FILE seat a trained champion (a champion.net from carranta-evolve)
                 at every bot chair; games record it as trained@<generation>

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
    let mut trained: Option<std::path::PathBuf> = None;

    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let mut value = || args.next().unwrap_or_default();
        match flag.as_str() {
            "--port" => port = value().parse().unwrap_or(port),
            "--seats" => seats = value().parse().unwrap_or(seats),
            "--seed" => seed = value().parse().unwrap_or(seed),
            "--games" => games = std::path::PathBuf::from(value()),
            "--demo" => demo = value().parse().unwrap_or(demo),
            "--trained" => trained = Some(std::path::PathBuf::from(value())),
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
    // A champion that cannot be loaded stops the server rather than quietly
    // seating the house bot: whoever passed `--trained` wanted the champion,
    // and a wrong player that looks right is the worst of the outcomes.
    if let Some(path) = &trained {
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
        println!("  bot seats played by trained@{generation}");
        server = server.with_trained(net, generation);
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
