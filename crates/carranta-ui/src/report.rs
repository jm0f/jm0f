//! The analytics page for one game, as HTML.
//!
//! Rendered on the server rather than fetched and drawn. Everything on it is
//! settled the moment the game ends, so there is nothing for a script to do:
//! the page is a report on a finished thing, and a report is a document.
//!
//! What it says, and what it refuses to say, is §10 of the scoping doc. The one
//! rule worth repeating here because it shapes the writing: **small n makes
//! p-values invalid, large n makes them uninformative**, so every figure is
//! paired with an effect size and a per-game result is placed against recorded
//! games rather than presented as a significance claim.

use std::fmt::Write as _;

use carranta_core::state::MAX_PLAYERS;

use crate::analysis::{Study, seat_name};
use crate::store::Saved;

const RESOURCE_NAMES: [&str; 5] = ["brick", "wood", "wool", "wheat", "ore"];
const DEV_NAMES: [&str; 5] = [
    "militia",
    "victory point",
    "monopoly",
    "road building",
    "invention",
];

/// Escape text going into the document.
fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn n1(v: f64) -> String {
    format!("{v:.1}")
}

/// A signed figure, so a movement reads as one.
///
/// Zero is written `+0.00` rather than `+-0.00`: a negative zero is still a
/// zero, and `-0.0 >= 0.0` is true, so the sign was being written twice.
fn signed(v: f64) -> String {
    let v = if v == 0.0 { 0.0 } else { v };
    if v >= 0.0 {
        format!("+{v:.2}")
    } else {
        format!("{v:.2}")
    }
}

/// One row of a table.
fn row(cells: &[String], head: bool) -> String {
    let tag = if head { "th" } else { "td" };
    let mut out = String::from("<tr>");
    for c in cells {
        let _ = write!(out, "<{tag}>{c}</{tag}>");
    }
    out.push_str("</tr>");
    out
}

/// Everybody at the table, in seat order, named.
fn names(saved: &Saved, seats: usize) -> Vec<String> {
    (0..seats).map(|s| seat_name(s, &saved.name)).collect()
}

pub fn page(saved: &Saved, study: &Study) -> String {
    let r = &study.report;
    let seats = r.players as usize;
    let who = names(saved, seats);
    let mut b = String::new();

    let _ = write!(
        b,
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>Carranta, game {id}</title><style>{css}</style></head><body>",
        id = esc(&saved.id),
        css = CSS
    );

    // ---- the header: which game, and what happened in it --------------------
    let _ = write!(
        b,
        "<header><a class=\"mark\" href=\"/\">Carranta</a>\
         <nav><a href=\"/{id}/\">the board</a></nav></header>\
         <main><h1>Game {id}</h1>\
         <p class=\"lede\">{seats} players, {mode} market, seed {seed}. \
         {turns} turns, {actions} actions.</p>",
        id = esc(&saved.id),
        mode = format!("{:?}", saved.mode).to_lowercase(),
        seed = crate::game::seed_code(saved.seed),
        turns = r.turns,
        actions = r.actions,
    );

    // ---- the result ---------------------------------------------------------
    b.push_str("<section><h2>The result</h2><table><thead>");
    b.push_str(&row(
        &[
            "".into(),
            "points".into(),
            "settlements".into(),
            "cities".into(),
            "roads".into(),
        ],
        true,
    ));
    b.push_str("</thead><tbody>");
    for s in 0..seats {
        let win = r.winner == Some(s as u8);
        let name = if win {
            format!("<strong>{}</strong> <span class=\"tag\">won</span>", esc(&who[s]))
        } else {
            esc(&who[s])
        };
        b.push_str(&row(
            &[
                name,
                r.vp[s].to_string(),
                r.builds[s].settlements.to_string(),
                r.builds[s].cities.to_string(),
                r.builds[s].roads.to_string(),
            ],
            false,
        ));
    }
    b.push_str("</tbody></table>");
    b.push_str(
        "<p class=\"note\">Points are the true total, hidden cards included \
         (R-11.3), which is not what the table could see while it was playing.</p></section>",
    );

    // ---- ratings ------------------------------------------------------------
    b.push_str("<section><h2>What it did to the ratings</h2>");
    if study.movement.iter().take(seats).all(Option::is_none) {
        b.push_str("<p class=\"note\">Nothing: this game could not be rated.</p>");
    } else {
        b.push_str("<table><thead>");
        b.push_str(&row(
            &[
                "".into(),
                "before".into(),
                "after".into(),
                "change".into(),
                "games behind it".into(),
            ],
            true,
        ));
        b.push_str("</thead><tbody>");
        for s in 0..seats {
            let Some(m) = study.movement[s] else { continue };
            let d = m.delta();
            let cls = if d > 0.0 {
                "up"
            } else if d < 0.0 {
                "down"
            } else {
                ""
            };
            b.push_str(&row(
                &[
                    esc(&who[s]),
                    n1(m.before.conservative()),
                    n1(m.after.conservative()),
                    format!("<span class=\"{cls}\">{}</span>", signed(d)),
                    m.games.to_string(),
                ],
                false,
            ));
        }
        b.push_str("</tbody></table>");
        b.push_str(
            "<p class=\"note\">A Weng-Lin Plackett-Luce update over the whole \
             finishing order, not just the winner (A-1): the final points rank \
             everyone, so one game carries a complete ranking rather than one \
             bit. The figure shown is the conservative estimate, three standard \
             deviations below the mean, which is what a rating is worth \
             believing rather than what it is. Early games move it a long way \
             because the belief starts wide; the count on the right is how much \
             each player had behind them going in.</p>",
        );
    }
    b.push_str("</section>");

    // ---- the dice -----------------------------------------------------------
    let total: u32 = r.rolls.iter().sum();
    b.push_str("<section><h2>The dice</h2>");
    let _ = write!(
        b,
        "<p>{total} rolls, {sevens} of them sevens. The deviation from a fair \
         pair of dice is <strong>{kl} bits</strong>.</p>",
        sevens = r.sevens,
        kl = format!("{:.3}", study.dice.kl_bits),
    );
    b.push_str("<table class=\"rolls\"><thead>");
    let mut heads = vec!["".to_string()];
    heads.extend((2..=12).map(|n| n.to_string()));
    b.push_str(&row(&heads, true));
    b.push_str("</thead><tbody>");
    let mut actual = vec!["rolled".to_string()];
    actual.extend(r.rolls.iter().map(u32::to_string));
    b.push_str(&row(&actual, false));
    let mut expected = vec!["expected".to_string()];
    for n in 2..=12u32 {
        let ways = 6 - (n as i32 - 7).abs();
        expected.push(n1(total as f64 * ways as f64 / 36.0));
    }
    b.push_str(&row(&expected, false));
    b.push_str("</tbody></table>");
    match study.dice_percentile {
        Some(p) => {
            let _ = write!(
                b,
                "<p>These dice deviated more than <strong>{p}%</strong> of the \
                 {n} other games recorded here.</p>",
                p = format!("{p:.0}"),
                n = study.corpus_games,
            );
        }
        None => b.push_str(
            "<p class=\"note\">There are no other games here to place this one \
             against yet. A percentile of one game is not a percentile.</p>",
        ),
    }
    b.push_str(
        "<p class=\"note\">No p-value, deliberately (§10.1). Across enough \
         games one in twenty clears p&lt;0.05 by construction, and those are \
         precisely the games somebody screenshots as proof of rigging. The \
         percentile carries the same information with no significance claim \
         attached. Whether the generator itself is fair is a different question \
         asked of millions of pooled rolls, never of one game.</p></section>",
    );

    // ---- production ---------------------------------------------------------
    b.push_str("<section><h2>What the board paid</h2><table><thead>");
    b.push_str(&row(
        &[
            "".into(),
            "expected".into(),
            "the robber".into(),
            "the supply".into(),
            "the dice".into(),
            "arrived".into(),
            "luck".into(),
        ],
        true,
    ));
    b.push_str("</thead><tbody>");
    for s in 0..seats {
        let d = study.production.decompose(s);
        b.push_str(&row(
            &[
                esc(&who[s]),
                n1(d.e_raw),
                signed(-d.robber_cost),
                signed(-d.supply_denial),
                format!(
                    "<span class=\"{}\">{}</span>",
                    if d.dice_luck >= 0.0 { "up" } else { "down" },
                    signed(d.dice_luck)
                ),
                n1(d.actual),
                format!("{:+.2}&sigma;", d.luck_z),
            ],
            false,
        ));
    }
    b.push_str("</tbody></table>");
    b.push_str(
        "<p class=\"note\">Expected is what the buildings standing at each roll \
         should have paid at the dice's true odds, and the three columns after \
         it are why that is not what arrived. Only one of them is chance: the \
         robber sitting on your hexes is an opponent's choice, and a stack \
         running dry is the supply (R-5.6). The last column is the dice term in \
         standard deviations, which is the one to read, since a card is worth \
         more on a small board than a large one.</p></section>",
    );

    // ---- the robber ---------------------------------------------------------
    b.push_str("<section><h2>The robber</h2>");
    let _ = write!(
        b,
        "<p>Moved {moves} times. {empty} of those robberies found an empty hand \
         (R-6.4).</p>",
        moves = r.robber_moves,
        empty = r.empty_robberies,
    );
    b.push_str("<table><thead>");
    let mut heads = vec!["stole from".to_string()];
    heads.extend(who.iter().map(|n| esc(n)));
    heads.push("discarded".to_string());
    b.push_str(&row(&heads, true));
    b.push_str("</thead><tbody>");
    for thief in 0..seats {
        let mut cells = vec![esc(&who[thief])];
        for victim in 0..seats {
            cells.push(if thief == victim {
                "&middot;".to_string()
            } else {
                r.steals[thief][victim].to_string()
            });
        }
        cells.push(r.discards[thief].to_string());
        b.push_str(&row(&cells, false));
    }
    b.push_str("</tbody></table>");
    b.push_str("<p class=\"note\">Read along a row: what that seat took.</p></section>");

    // ---- the market ---------------------------------------------------------
    b.push_str("<section><h2>The market</h2><table><thead>");
    b.push_str(&row(
        &[
            "".into(),
            "offered".into(),
            "turned down".into(),
            "withdrew".into(),
            "traded".into(),
            "with the bank".into(),
        ],
        true,
    ));
    b.push_str("</thead><tbody>");
    for s in 0..seats {
        b.push_str(&row(
            &[
                esc(&who[s]),
                r.offers_made[s].to_string(),
                r.offers_declined[s].to_string(),
                r.offers_withdrawn[s].to_string(),
                r.trades_completed[s].to_string(),
                r.supply_trades[s].to_string(),
            ],
            false,
        ));
    }
    b.push_str("</tbody></table>");
    b.push_str(
        "<p class=\"note\">A completed trade is counted for both sides of it, so \
         that column sums to twice the number of trades.</p></section>",
    );

    // ---- development cards --------------------------------------------------
    b.push_str("<section><h2>Development cards</h2><table><thead>");
    let mut heads = vec!["".to_string(), "bought".to_string()];
    heads.extend(DEV_NAMES.iter().map(|n| n.to_string()));
    b.push_str(&row(&heads, true));
    b.push_str("</thead><tbody>");
    for s in 0..seats {
        let mut cells = vec![esc(&who[s]), r.dev_bought[s].to_string()];
        cells.extend(r.dev_played[s].iter().map(u32::to_string));
        b.push_str(&row(&cells, false));
    }
    b.push_str("</tbody></table>");
    b.push_str(
        "<p class=\"note\">Played, not held. A victory point card is never \
         played (R-9.11), so that column is always empty and is kept so the \
         five read in the order the cards do.</p></section>",
    );

    // ---- the opening --------------------------------------------------------
    b.push_str("<section><h2>The opening</h2><table><thead>");
    b.push_str(&row(
        &[
            "".into(),
            "pips".into(),
            "resources".into(),
            "ports".into(),
            "biggest hand".into(),
        ],
        true,
    ));
    b.push_str("</thead><tbody>");
    for s in 0..seats {
        b.push_str(&row(
            &[
                esc(&who[s]),
                r.opening[s].pips.to_string(),
                format!("{} of {}", r.opening[s].diversity, RESOURCE_NAMES.len()),
                r.opening[s].ports.to_string(),
                r.peak_hand[s].to_string(),
            ],
            false,
        ));
    }
    b.push_str("</tbody></table>");
    b.push_str(
        "<p class=\"note\">Pips are the dots on every number the starting \
         settlements touch, which is the standard measure of how much \
         production a placement buys.</p></section>",
    );

    // ---- the corpus ---------------------------------------------------------
    if let Some(wins) = study.seat_wins {
        b.push_str("<section><h2>Across every game here</h2>");
        let _ = write!(
            b,
            "<p>{n} other games, all at this table's settings.</p><table><thead>",
            n = study.corpus_games
        );
        b.push_str(&row(&["seat".into(), "win rate".into()], true));
        b.push_str("</thead><tbody>");
        for s in 0..seats.min(MAX_PLAYERS) {
            b.push_str(&row(
                &[
                    format!("seat {s}"),
                    format!("{:.0}%", wins[s] * 100.0),
                ],
                false,
            ));
        }
        b.push_str("</tbody></table>");
        b.push_str(
            "<p class=\"note\">Seat, not player, because this is asking whether \
             going first is worth anything. It needs hundreds of games before it \
             means much: at this count the spread is noise.</p></section>",
        );
    }

    b.push_str("</main></body></html>");
    b
}

/// The page's own styles.
///
/// The same ink, paper and faces as the board, written out rather than shared:
/// the board's stylesheet is a game's worth of rules about cards and hexes, and
/// a document needs almost none of it.
const CSS: &str = "
:root {
  --bg: #F3EDE1; --panel: #FBF7EF; --line: #E2D9C8;
  --ink: #33261B; --dim: #6B5B4C; --accent: #E8542F; --good: #1B5637;
}
@font-face { font-family: Figtree; src: url('/font/figtree.woff2') format('woff2');
             font-weight: 300 900; font-display: swap; }
@font-face { font-family: Fraunces; src: url('/font/fraunces.woff2') format('woff2');
             font-weight: 300 900; font-display: swap; }
@font-face { font-family: Audiowide; src: url('/font/audiowide.woff2') format('woff2');
             font-weight: 400; font-display: swap; }
* { box-sizing: border-box; }
body { margin: 0; background: var(--bg); color: var(--ink);
       font: 16px/1.55 Figtree, system-ui, sans-serif; }
header { display: flex; align-items: baseline; gap: 1.2em;
         padding: 1.2rem clamp(16px, 5vw, 64px); }
.mark { font: 400 22px Audiowide, system-ui, sans-serif; color: var(--accent);
        text-decoration: none; }
nav a { color: var(--dim); text-decoration: none; border-bottom: 1px solid var(--line); }
nav a:hover { color: var(--accent); border-color: var(--accent); }
main { max-width: 62rem; margin: 0 auto; padding: 0 clamp(16px, 5vw, 64px) 5rem; }
h1 { font: 600 clamp(28px, 4vw, 40px)/1.1 Fraunces, Georgia, serif; margin: .4em 0 .2em; }
h2 { font: 600 13px Figtree, system-ui, sans-serif; text-transform: uppercase;
     letter-spacing: .09em; color: var(--dim); margin: 0 0 .9rem; }
.lede { color: var(--dim); margin: 0 0 2.5rem; }
section { background: var(--panel); border: 1px solid var(--line);
          border-radius: 14px; padding: 1.4rem 1.5rem; margin: 0 0 1.1rem;
          box-shadow: 0 2px 5px rgba(51,38,27,.05), 0 12px 28px rgba(51,38,27,.06); }
section > p { margin: 0 0 1rem; }
table { width: 100%; border-collapse: collapse; font-variant-numeric: tabular-nums; }
/* A wide table scrolls inside itself rather than pushing the page sideways. */
section { overflow-x: auto; }
th, td { text-align: right; padding: .45em .6em; border-bottom: 1px solid var(--line); }
th:first-child, td:first-child { text-align: left; }
thead th { font-weight: 600; font-size: 13px; color: var(--dim);
           text-transform: uppercase; letter-spacing: .05em; }
tbody tr:last-child td { border-bottom: 0; }
.rolls th, .rolls td { padding: .45em .35em; }
.tag { display: inline-block; padding: 0 .5em; border-radius: 999px;
       background: var(--accent); color: #FBF7EF; font-size: 12px;
       font-weight: 600; letter-spacing: .03em; vertical-align: 1px; }
.up { color: var(--good); font-weight: 600; }
.down { color: var(--accent); font-weight: 600; }
.note { color: var(--dim); font-size: 14px; margin: 1rem 0 0; }
";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::study;
    use crate::game::Session;
    use crate::store::game_id;
    use carranta_core::state::TradeMode;

    fn played(seed: u64) -> Saved {
        let mut s = Session::new(4, seed, TradeMode::Full);
        for _ in 0..500 {
            let v = s.version();
            if s.choices().is_empty() || s.act(0, v).is_err() {
                break;
            }
        }
        let (seats, dealt, mode) = s.table();
        Saved {
            id: game_id(seed),
            seats,
            seed: dealt,
            mode,
            name: "Egon".to_string(),
            dealt: seed,
            winner: s.winner(),
            moves: s.moves().to_vec(),
        }
    }

    #[test]
    fn the_page_says_what_the_game_did() {
        let history: Vec<Saved> = (0..3u64).map(played).collect();
        let s = study(&history[1], &history).expect("it studies");
        let html = page(&history[1], &s);
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.ends_with("</html>"));
        // Everybody at the table is named, and the board is a click away.
        // Seat 0 is the person, so the bots are the names from seat 1 on, which
        // is what the board calls them too.
        for name in ["Egon", "Bram", "Ines", "Odd"] {
            assert!(html.contains(name), "{name} is on the page");
        }
        assert!(html.contains(&format!("/{}/", history[1].id)));
        // The sections that were asked for, by their headings.
        for heading in [
            "The result",
            "What it did to the ratings",
            "The dice",
            "What the board paid",
            "The robber",
            "The market",
            "Development cards",
            "The opening",
        ] {
            assert!(html.contains(heading), "{heading} is a section");
        }
        // And the thing §10.1 forbids is not on it.
        assert!(!html.contains("p-value ="), "no significance claim");
        assert!(html.contains("bits"), "an effect size instead");
    }

    #[test]
    fn a_name_with_markup_in_it_is_text_on_the_page() {
        let mut g = played(5);
        g.name = "<script>alert(1)</script>".to_string();
        let s = study(&g, std::slice::from_ref(&g)).expect("it studies");
        let html = page(&g, &s);
        assert!(!html.contains("<script>alert"), "the name is escaped");
        assert!(html.contains("&lt;script&gt;alert"));
    }

    #[test]
    fn a_zero_is_written_once() {
        assert_eq!(signed(0.0), "+0.00");
        assert_eq!(signed(-0.0), "+0.00");
        assert_eq!(signed(1.5), "+1.50");
        assert_eq!(signed(-1.5), "-1.50");
    }

    #[test]
    fn the_first_game_says_it_has_nothing_to_compare_with() {
        let only = played(9);
        let s = study(&only, std::slice::from_ref(&only)).expect("it studies");
        let html = page(&only, &s);
        assert!(html.contains("no other games here"));
        assert!(!html.contains("Across every game here"));
    }
}
