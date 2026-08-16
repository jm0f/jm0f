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
  --demo N       have at least N played games to look at (plays what is missing)

Binds 127.0.0.1 only: the game is local and stays local.";

fn main() {
    let mut port: u16 = 8181;
    let mut seats: u8 = 4;
    let mut seed: u64 = 1;
    let mut mode = TradeMode::Full;
    // Beside the binary by default, so a game played today is still there
    // tomorrow without anyone having to say where to put it.
    let mut games = std::path::PathBuf::from("games");
    let mut demo: u32 = 0;

    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let mut value = || args.next().unwrap_or_default();
        match flag.as_str() {
            "--port" => port = value().parse().unwrap_or(port),
            "--seats" => seats = value().parse().unwrap_or(seats),
            "--seed" => seed = value().parse().unwrap_or(seed),
            "--games" => games = std::path::PathBuf::from(value()),
            "--demo" => demo = value().parse().unwrap_or(demo),
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

    // Loopback only. This has no authentication and keeps its games in a
    // directory beside it; it is a local tool, and binding it wider would be a
    // mistake rather than a feature.
    let listener = match TcpListener::bind(("127.0.0.1", port)) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("cannot listen on 127.0.0.1:{port}: {e}");
            std::process::exit(1);
        }
    };
    println!(
        "Carranta {}, open http://127.0.0.1:{port}",
        env!("CARRANTA_BUILD")
    );
    println!("  {seats} seats, {mode:?} market, seed {seed}");
    let server = Server::new(seats, seed, mode, &games);
    println!("  games in {}", server.store().dir().display());
    // Played before the door opens, so the addresses are printed beside the
    // one you would open anyway.
    for id in server.demo(demo) {
        println!("  played http://127.0.0.1:{port}/{id}/analytics");
    }
    server.serve(listener);
}
