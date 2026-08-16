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

/// A card's header: what the card is, and one line on what it says.
///
/// Title then description, which is the borrowed design's shape for this and a
/// better one than the uppercase micro-label it replaces: a label tells you a
/// section exists, a description tells you whether to read it.
fn card_head(title: &str, desc: &str) -> String {
    card_head_why(title, desc, "")
}

/// The same, with the card's own explanation behind the description.
///
/// The long note that used to sit under a table is here instead. It was being
/// read through to reach the figures, or not read at all; on the description it
/// is one hover away from the reader who wants it and invisible to the one who
/// does not.
fn card_head_why(title: &str, desc: &str, why: &str) -> String {
    let desc = if why.is_empty() {
        format!("<p class=\"desc\">{desc}</p>")
    } else {
        format!("<p class=\"desc\" title=\"{}\">{desc}</p>", esc(why))
    };
    format!("<div class=\"card-head\"><h2>{title}</h2>{desc}</div>")
}

/// A totals row, in the table's foot.
///
/// Only where a column adds up to something true. A maximum, a rate and a
/// percentile do not, and a row that totalled them would be read as though they
/// did.
fn totals(cells: &[String]) -> String {
    let mut out = String::from("<tfoot><tr>");
    for c in cells {
        let _ = write!(out, "<td>{c}</td>");
    }
    out.push_str("</tr></tfoot>");
    out
}

/// A table lives in its own bordered box rather than bleeding into the card.
const T_OPEN: &str = "<div class=\"tw\"><table>";
const T_CLOSE: &str = "</table></div>";

/// A head row whose columns carry their own explanation.
///
/// The rules behind a column belong on the column rather than in a paragraph
/// under the table: a reader who wants to know what "cities" counts is looking
/// at the word "cities", and a reader who does not should not have to scroll
/// past the answer.
fn head_row(cells: &[(&str, &str)]) -> String {
    let mut out = String::from("<tr>");
    for (label, why) in cells {
        if why.is_empty() {
            let _ = write!(out, "<th>{label}</th>");
        } else {
            let _ = write!(out, "<th title=\"{}\">{label}</th>", esc(why));
        }
    }
    out.push_str("</tr>");
    out
}

/// One scoring column: how many, and what they were worth.
///
/// Nought is a dot, since a zero in a column of scores is nothing rather than a
/// number to read.
fn scored(s: crate::analysis::Scored) -> String {
    if s.held == 0 {
        "&middot;".to_string()
    } else {
        format!("{} <span class=\"worth\">({})</span>", s.held, s.points)
    }
}

/// Everybody at the table, in seat order, named.
fn names(saved: &Saved, seats: usize) -> Vec<String> {
    (0..seats).map(|s| seat_name(s, &saved.name)).collect()
}

/// The game as one bar: a segment per turn, sized by what happened in it and
/// coloured by whose it was.
///
/// The bar is always the full width, so it says nothing about how long the game
/// took and everything about how it was divided. A turn that reads twice as
/// wide as its neighbour had twice as much in it.
fn turn_bar(study: &Study, who: &[String], seats: usize) -> String {
    let mut b = String::from("<section>");
    b.push_str(&card_head_why(
        "The turns",
        "The whole game across one bar, a segment per turn, sized by what \
         happened in it and coloured by whose it was.",
        "The bar is always the full width, so it says nothing about how long \
         the game took and everything about how it was divided: a turn twice as \
         wide as its neighbour had twice as much in it. There are no gaps \
         because play goes round the table, so no two neighbours share a \
         colour. The setup placements are left out, since they come before \
         anybody has a turn to take.",
    ));
    let turns = &study.turns;
    if turns.is_empty() {
        b.push_str("<p class=\"note\">Nobody finished a turn in this game.</p></section>");
        return b;
    }

    let total: u32 = turns.iter().map(|t| t.actions).sum();
    let spent: u32 = turns.iter().map(|t| t.millis).sum();

    b.push_str("<div class=\"bar\">");
    for (i, t) in turns.iter().enumerate() {
        let when = if study.timed {
            format!(", {}", clock(t.millis))
        } else {
            String::new()
        };
        let _ = write!(
            b,
            "<span class=\"seg s{seat}\" style=\"flex-grow:{grow}\" \
             title=\"Turn {n}, {name}: {moves}{when}\"></span>",
            seat = t.seat.min(MAX_PLAYERS - 1),
            grow = t.actions,
            n = i + 1,
            name = esc(&who[t.seat]),
            moves = plural(t.actions, "move"),
        );
    }
    b.push_str("</div>");

    b.push_str(T_OPEN);
    b.push_str("<thead>");
    let mut heads = vec![
        ("", ""),
        ("turns", "Turns this seat took."),
        (
            "moves",
            "Decisions taken while it was their turn. Not all of them theirs: a \
             discard, a robbery and an accepted offer all land inside somebody \
             else's turn.",
        ),
        (
            "longest",
            "The most that happened in any one of their turns. A maximum, so it \
             has no total.",
        ),
    ];
    if study.timed {
        heads.push((
            "time",
            "Wall-clock time inside their turns, which is their own thinking \
             plus whatever the table made them wait for. A game the computer \
             played out to itself takes almost none.",
        ));
    }
    heads.push((
        "share of the bar",
        "How much of the bar's width is theirs, which is their share of the \
         game's moves rather than of its turns.",
    ));
    b.push_str(&head_row(&heads));
    b.push_str("</thead><tbody>");
    for s in 0..seats {
        let mine: Vec<&crate::analysis::Turn> = turns.iter().filter(|t| t.seat == s).collect();
        let moves: u32 = mine.iter().map(|t| t.actions).sum();
        let mut cells = vec![
            format!("<span class=\"dot s{s}\"></span>{}", esc(&who[s])),
            mine.len().to_string(),
            moves.to_string(),
            mine.iter()
                .map(|t| t.actions)
                .max()
                .unwrap_or(0)
                .to_string(),
        ];
        if study.timed {
            cells.push(clock(mine.iter().map(|t| t.millis).sum()));
        }
        cells.push(format!("{:.0}%", 100.0 * moves as f64 / total as f64));
        b.push_str(&row(&cells, false));
    }
    b.push_str("</tbody>");
    let mut foot = vec![
        "the game".to_string(),
        turns.len().to_string(),
        total.to_string(),
        // A maximum does not add up, and a column that adds where it cannot
        // would be read as though it did.
        "&middot;".to_string(),
    ];
    if study.timed {
        foot.push(clock(spent));
    }
    foot.push("100%".to_string());
    b.push_str(&totals(&foot));
    b.push_str(T_CLOSE);
    b.push_str("</section>");
    b
}

/// `1 move`, `4 moves`.
fn plural(n: u32, thing: &str) -> String {
    if n == 1 {
        format!("{n} {thing}")
    } else {
        format!("{n} {thing}s")
    }
}

/// A duration, at whatever precision it is worth reading at.
///
/// A game somebody played takes minutes a turn; a game the computer played out
/// to itself takes microseconds. One format cannot show both, so the scale
/// picks itself: below a second the milliseconds are the answer, and above an
/// hour the seconds are noise.
fn clock(millis: u32) -> String {
    let secs = millis / 1000;
    if millis < 1000 {
        format!("{millis}ms")
    } else if secs < 60 {
        format!("{:.1}s", millis as f64 / 1000.0)
    } else if secs < 3600 {
        format!("{}m {:02}s", secs / 60, secs % 60)
    } else {
        format!("{}h {:02}m", secs / 3600, (secs % 3600) / 60)
    }
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
    // The five things that score (R-11.3), each as how many were held and, in
    // brackets, what they were worth. The bracketed figures add across to the
    // total beside them.
    b.push_str("<section>");
    b.push_str(&card_head(
        "The result",
        "Where every point came from, read off the final position.",
    ));
    b.push_str(T_OPEN);
    b.push_str("<thead>");
    b.push_str(&head_row(&[
        ("", ""),
        (
            "points",
            "The true final score, hidden victory point cards included, which is \
             not what the table could see while it was playing (R-11.3).",
        ),
        (
            "settlements",
            "Settlements still standing at the end, one point each. A settlement \
             upgraded to a city stopped being a settlement, so this is fewer \
             than the number built.",
        ),
        ("cities", "Cities at the end, two points each."),
        (
            "victory points",
            "Victory point cards held. One point each, never played, and hidden \
             until the game ends (R-9.11). Not the score beside it: that is \
             every point from every source, this is the cards alone.",
        ),
        (
            "longest road",
            "Whether they held the longest road tile at the end. Two points, and \
             the roads themselves score nothing however many there are.",
        ),
        (
            "largest militia",
            "Whether they held the largest militia tile at the end. Two points, \
             and a militia played scores nothing on its own.",
        ),
    ]));
    b.push_str("</thead><tbody>");
    for s in 0..seats {
        let win = r.winner == Some(s as u8);
        let name = if win {
            format!(
                "<strong>{}</strong> <span class=\"tag\">won</span>",
                esc(&who[s])
            )
        } else {
            esc(&who[s])
        };
        let mut cells = vec![name, format!("<strong>{}</strong>", r.vp[s])];
        cells.extend(study.points[s].parts().iter().map(|p| scored(*p)));
        b.push_str(&row(&cells, false));
    }
    b.push_str("</tbody>");
    b.push_str(T_CLOSE);
    b.push_str("</section>");

    // ---- the turns ----------------------------------------------------------
    b.push_str(&turn_bar(study, &who, seats));

    // ---- ratings ------------------------------------------------------------
    b.push_str("<section>");
    b.push_str(&card_head_why(
        "What it did to the ratings",
        "The pool before this game and after it, which is the section this page \
         exists for.",
        "A Weng-Lin Plackett-Luce update over the whole finishing order, not \
         just the winner (A-1): the final points rank everyone, so one game \
         carries a complete ranking rather than one bit. Ratings are computed \
         by replaying every recorded game in order and reading the pool either \
         side of this one, since a rating is a function of everything before \
         it.",
    ));
    if study.movement.iter().take(seats).all(Option::is_none) {
        b.push_str("<p class=\"note\">Nothing: this game could not be rated.</p>");
    } else {
        b.push_str(T_OPEN);
        b.push_str("<thead>");
        b.push_str(&head_row(&[
            ("", ""),
            (
                "before",
                "The conservative estimate going in: three standard deviations \
                 below the mean, which is what a rating is worth believing \
                 rather than what it is.",
            ),
            ("after", "The same figure once this game was folded in."),
            (
                "change",
                "An update is a redistribution, so these very nearly cancel: \
                 somebody gains what somebody else loses.",
            ),
            (
                "games behind it",
                "How many games each player had behind them going in, which is \
                 how much the figure is worth believing. Early games move a \
                 rating a long way because the belief starts wide.",
            ),
        ]));
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
        b.push_str("</tbody>");
        // Before and after are positions rather than quantities, and adding
        // four ratings together produces a number of nothing.
        let moved: f64 = (0..seats)
            .filter_map(|s| study.movement[s])
            .map(|m| m.delta())
            .sum();
        b.push_str(&totals(&[
            "the table".to_string(),
            "&middot;".to_string(),
            "&middot;".to_string(),
            signed(moved),
            (0..seats)
                .filter_map(|s| study.movement[s])
                .map(|m| m.games)
                .sum::<u32>()
                .to_string(),
        ]));
        b.push_str(T_CLOSE);
    }
    b.push_str("</section>");

    // ---- the dice -----------------------------------------------------------
    let total: u32 = r.rolls.iter().sum();
    b.push_str("<section>");
    b.push_str(&card_head_why(
        "The dice",
        match study.dice_percentile {
            // "More than 100% of five games" is not a sentence. At either end
            // of the range the comparison is to all of them or none of them,
            // and saying so is both shorter and true.
            Some(p) => format!(
                "{total} rolls, {sevens} of them sevens, deviating from a fair \
                 pair by {kl} bits: {how} of the {n} other finished games \
                 recorded here.",
                sevens = r.sevens,
                kl = format!("{:.3}", study.dice.kl_bits),
                n = study.corpus_games,
                how = if p >= 99.5 {
                    "further than every one".to_string()
                } else if p < 0.5 {
                    "less than any".to_string()
                } else {
                    format!("further than {p:.0}%")
                },
            ),
            None => format!(
                "{total} rolls, {sevens} of them sevens, deviating from a fair \
                 pair by {kl} bits. There are no other finished games here to \
                 place this one against yet.",
                sevens = r.sevens,
                kl = format!("{:.3}", study.dice.kl_bits),
            ),
        }
        .as_str(),
        "No p-value, deliberately (§10.1). Across enough games one in twenty \
         clears p<0.05 by construction, and those are precisely the games \
         somebody screenshots as proof of rigging. The percentile carries the \
         same information with no significance claim attached, and until there \
         is a second game it is withheld, because a percentile of one game is \
         not a percentile. Whether the generator itself is fair is a different \
         question asked of millions of pooled rolls, never of one game.",
    ));
    b.push_str("<div class=\"tw\"><table class=\"rolls\"><thead>");
    let mut heads = vec![("", "")];
    let labels: Vec<String> = (2..=12).map(|n: u32| n.to_string()).collect();
    heads.extend(labels.iter().map(|s| (s.as_str(), "")));
    heads.push((
        "total",
        "Every roll in the game. The two rows have to add to the same number: \
         one is how the rolls fell and the other is how they should have.",
    ));
    b.push_str(&head_row(&heads));
    b.push_str("</thead><tbody>");
    let mut actual = vec!["rolled".to_string()];
    actual.extend(r.rolls.iter().map(u32::to_string));
    actual.push(format!("<strong>{total}</strong>"));
    b.push_str(&row(&actual, false));
    let mut expected = vec!["expected".to_string()];
    for n in 2..=12u32 {
        let ways = 6 - (n as i32 - 7).abs();
        expected.push(n1(total as f64 * ways as f64 / 36.0));
    }
    expected.push(format!("<strong>{}</strong>", n1(total as f64)));
    b.push_str(&row(&expected, false));
    b.push_str("</tbody>");
    b.push_str(T_CLOSE);
    b.push_str("</section>");

    // ---- production ---------------------------------------------------------
    b.push_str("<section>");
    b.push_str(&card_head_why(
        "What the board paid",
        "Production decomposed into what was chance and what was somebody's \
         doing (§10.2).",
        "Expected is what the buildings standing at each roll should have paid \
         at the dice's true odds, and the three columns after it are why that \
         is not what arrived. Only one of them is chance: the robber sitting on \
         your hexes is an opponent's choice, and a stack running dry is the \
         supply (R-5.6).",
    ));
    b.push_str(T_OPEN);
    b.push_str("<thead>");
    b.push_str(&head_row(&[
        ("", ""),
        (
            "expected",
            "What the buildings standing at each roll should have paid at the \
             dice's true odds.",
        ),
        (
            "the robber",
            "Cards the robber cost them by sitting on their hexes. Not chance: \
             somebody chose that hex.",
        ),
        (
            "the supply",
            "Cards a stack that had run dry could not pay (R-5.6). Not chance \
             either: the table emptied it.",
        ),
        (
            "the dice",
            "What is left once the other two are taken out, which is the only \
             genuinely random term of the three.",
        ),
        ("arrived", "Cards that actually reached their hand."),
        (
            "luck",
            "The dice term in standard deviations, which is the one to read: a \
             card is worth more on a small board than a large one. A spread, so \
             it has no total.",
        ),
    ]));
    b.push_str("</thead><tbody>");
    let mut sums = [0.0f64; 5];
    for s in 0..seats {
        let d = study.production.decompose(s);
        for (slot, v) in sums.iter_mut().zip([
            d.e_raw,
            -d.robber_cost,
            -d.supply_denial,
            d.dice_luck,
            d.actual,
        ]) {
            *slot += v;
        }
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
    b.push_str("</tbody>");
    b.push_str(&totals(&[
        "the board".to_string(),
        n1(sums[0]),
        signed(sums[1]),
        signed(sums[2]),
        signed(sums[3]),
        n1(sums[4]),
        // A z-score is a position on a spread. Four of them add to nothing.
        "&middot;".to_string(),
    ]));
    b.push_str(T_CLOSE);
    b.push_str("</section>");

    // ---- the robber ---------------------------------------------------------
    b.push_str("<section>");
    let _ = write!(
        b,
        "{}",
        card_head_why(
            "The robber",
            &format!(
                "Moved {moves} times, {empty} of those robberies finding an \
                 empty hand (R-6.4).",
                moves = r.robber_moves,
                empty = r.empty_robberies,
            ),
            "Read along a row: what that seat took. The diagonal is a dot \
             because nobody robs themselves.",
        )
    );
    b.push_str(T_OPEN);
    b.push_str("<thead>");
    let victims: Vec<String> = who.iter().map(|n| esc(n)).collect();
    let mut heads = vec![("stole from", "")];
    heads.extend(victims.iter().map(|n| (n.as_str(), "")));
    heads.push((
        "discarded",
        "Cards this seat threw away to sevens, which is the robber's other \
         cost and nobody's choice but the dice's (R-6.2).",
    ));
    b.push_str(&head_row(&heads));
    b.push_str("</thead><tbody>");
    let mut taken = vec![0u32; seats];
    let mut binned = 0u32;
    for thief in 0..seats {
        let mut cells = vec![esc(&who[thief])];
        for victim in 0..seats {
            cells.push(if thief == victim {
                "&middot;".to_string()
            } else {
                taken[victim] += r.steals[thief][victim];
                r.steals[thief][victim].to_string()
            });
        }
        binned += r.discards[thief];
        cells.push(r.discards[thief].to_string());
        b.push_str(&row(&cells, false));
    }
    b.push_str("</tbody>");
    let mut foot = vec!["lost".to_string()];
    foot.extend(taken.iter().map(u32::to_string));
    foot.push(binned.to_string());
    b.push_str(&totals(&foot));
    b.push_str(T_CLOSE);
    b.push_str("</section>");

    // ---- the market ---------------------------------------------------------
    b.push_str("<section>");
    b.push_str(&card_head(
        "The market",
        "Negotiation churn, which under an open market is most of the \
         interaction in a game (H-4).",
    ));
    b.push_str(T_OPEN);
    b.push_str("<thead>");
    b.push_str(&head_row(&[
        ("", ""),
        ("offered", "Offers this seat put on the table."),
        ("turned down", "Offers this seat said no to."),
        (
            "withdrew",
            "Offers this seat pulled back before anybody took them.",
        ),
        (
            "traded",
            "Trades this seat was a party to. Counted for both sides, so the \
             total is twice the number of trades that happened.",
        ),
        (
            "with the bank",
            "Trades against the supply rather than against a person.",
        ),
    ]));
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
    b.push_str("</tbody>");
    let sum = |v: &[u32; MAX_PLAYERS]| v[..seats].iter().sum::<u32>().to_string();
    b.push_str(&totals(&[
        "the table".to_string(),
        sum(&r.offers_made),
        sum(&r.offers_declined),
        sum(&r.offers_withdrawn),
        sum(&r.trades_completed),
        sum(&r.supply_trades),
    ]));
    b.push_str(T_CLOSE);
    b.push_str("</section>");

    // ---- development cards --------------------------------------------------
    b.push_str("<section>");
    b.push_str(&card_head_why(
        "Development cards",
        "What was bought, and what was played with it.",
        "Played, not held. A victory point card is never played (R-9.11), so \
         that column is always empty, and it is kept so the five read in the \
         order the cards do.",
    ));
    b.push_str(T_OPEN);
    b.push_str("<thead>");
    let mut heads = vec![
        ("", ""),
        ("bought", "Cards drawn from the deck, of any kind."),
    ];
    heads.extend(DEV_NAMES.iter().map(|n| (*n, "")));
    b.push_str(&head_row(&heads));
    b.push_str("</thead><tbody>");
    let mut played = [0u32; 5];
    for s in 0..seats {
        let mut cells = vec![esc(&who[s]), r.dev_bought[s].to_string()];
        cells.extend(r.dev_played[s].iter().map(u32::to_string));
        for (slot, n) in played.iter_mut().zip(r.dev_played[s]) {
            *slot += n;
        }
        b.push_str(&row(&cells, false));
    }
    b.push_str("</tbody>");
    let mut foot = vec![
        "the deck".to_string(),
        r.dev_bought[..seats].iter().sum::<u32>().to_string(),
    ];
    foot.extend(played.iter().map(u32::to_string));
    b.push_str(&totals(&foot));
    b.push_str(T_CLOSE);
    b.push_str("</section>");

    // ---- the opening --------------------------------------------------------
    b.push_str("<section>");
    b.push_str(&card_head(
        "The opening",
        "What the first two settlements bought, before anybody had a turn.",
    ));
    b.push_str(T_OPEN);
    b.push_str("<thead>");
    b.push_str(&head_row(&[
        ("", ""),
        (
            "pips",
            "The dots on every number the starting settlements touch, which is \
             the standard measure of how much production a placement buys.",
        ),
        (
            "resources",
            "How many of the five the opening touches at all. A placement can \
             be rich and still be missing something it will need.",
        ),
        ("ports", "Ports the starting settlements sit on."),
        (
            "biggest hand",
            "The most cards this seat ever held at once, anywhere in the game. \
             Not an opening figure, and here because it is the other half of \
             the same question: what the placement turned into.",
        ),
    ]));
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
    b.push_str("</tbody>");
    b.push_str(&totals(&[
        "the board".to_string(),
        r.opening[..seats]
            .iter()
            .map(|o| o.pips)
            .sum::<u32>()
            .to_string(),
        // Diversity is per placement and a peak is a maximum. Neither adds.
        "&middot;".to_string(),
        r.opening[..seats]
            .iter()
            .map(|o| o.ports)
            .sum::<u32>()
            .to_string(),
        "&middot;".to_string(),
    ]));
    b.push_str(T_CLOSE);
    b.push_str("</section>");

    // ---- the corpus ---------------------------------------------------------
    if let Some(wins) = study.seat_wins {
        b.push_str("<section>");
        let _ = write!(
            b,
            "{}",
            card_head_why(
                "Across every game here",
                &format!(
                    "Seat win rates across {n} other finished games, all at this \
                     table's settings.",
                    n = study.corpus_games
                ),
                "Seat, not player, because this is asking whether going first is \
                 worth anything. It needs hundreds of games before it means \
                 much: at this count the spread is noise. A game nobody won is \
                 left out, since it has no finishing order and can only enlarge \
                 the denominator.",
            )
        );
        b.push_str(T_OPEN);
        b.push_str("<thead>");
        b.push_str(&head_row(&[
            ("seat", ""),
            (
                "win rate",
                "Games won from this seat, over games played from it.",
            ),
        ]));
        b.push_str("</thead><tbody>");
        for s in 0..seats.min(MAX_PLAYERS) {
            b.push_str(&row(
                &[format!("seat {s}"), format!("{:.0}%", wins[s] * 100.0)],
                false,
            ));
        }
        b.push_str("</tbody>");
        b.push_str(&totals(&[
            "every seat".to_string(),
            format!(
                "{:.0}%",
                wins[..seats.min(MAX_PLAYERS)].iter().sum::<f64>() * 100.0
            ),
        ]));
        b.push_str(T_CLOSE);
        b.push_str("</section>");
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
/* ---- tokens ----
   shadcn's vocabulary, in Carranta's ink and paper. The names are theirs
   because the roles they name are the useful part: `--muted-foreground` says
   what a colour is *for*, where `--dim` only says what it looks like. The
   values are the board's, so a report still belongs to the game it reports on.

   The library itself is React, Tailwind and a bundler, none of which this
   server has or wants (H-1 in spirit: one binary, no build step). What was
   worth taking is the system underneath: the role names, the radius scale,
   the muted-foreground habit, and the proportions of a card and a table. */
:root {
  --background: #F3EDE1;
  --foreground: #33261B;
  --card: #FBF7EF;
  --muted: #EFE8DA;
  --muted-foreground: #6B5B4C;
  --border: #E2D9C8;
  --primary: #E8542F;
  --primary-foreground: #FBF7EF;
  --positive: #1B5637;
  /* One radius, and everything else derived from it, which is what keeps a
     badge, a table and a card looking like one family. */
  --radius: .625rem;
  --radius-sm: calc(var(--radius) - 4px);
  --radius-md: calc(var(--radius) - 2px);
  --radius-lg: var(--radius);
  --radius-xl: calc(var(--radius) + 4px);
  /* The seat colours the board plays in, so a bar segment is recognisably
     the same player who sat there. */
  --p0: #2CA7BA; --p1: #3C2EB8; --p2: #C065D2; --p3: #C1256B;
}
@font-face { font-family: Figtree; src: url('/font/figtree.woff2') format('woff2');
             font-weight: 300 900; font-display: swap; }
@font-face { font-family: Fraunces; src: url('/font/fraunces.woff2') format('woff2');
             font-weight: 300 900; font-display: swap; }
@font-face { font-family: Audiowide; src: url('/font/audiowide.woff2') format('woff2');
             font-weight: 400; font-display: swap; }
* { box-sizing: border-box; }
body { margin: 0; background: var(--background); color: var(--foreground);
       font: 16px/1.55 Figtree, system-ui, sans-serif;
       -webkit-font-smoothing: antialiased; }
header { display: flex; align-items: baseline; gap: 1.2em;
         padding: 1.2rem clamp(16px, 5vw, 64px); }
.mark { font: 400 22px Audiowide, system-ui, sans-serif; color: var(--primary);
        text-decoration: none; }
nav a { color: var(--muted-foreground); text-decoration: none;
        border-bottom: 1px solid var(--border); }
nav a:hover { color: var(--primary); border-color: var(--primary); }
main { max-width: 62rem; margin: 0 auto; padding: 0 clamp(16px, 5vw, 64px) 5rem;
       display: flex; flex-direction: column; gap: 1.5rem; }
/* Tight tracking on a big heading, which is the one typographic tic of the
   borrowed design worth keeping wholesale. */
h1 { font: 600 clamp(28px, 4vw, 40px)/1.1 Fraunces, Georgia, serif;
     letter-spacing: -.02em; margin: .4em 0 .2em; }
.lede { color: var(--muted-foreground); margin: 0 0 1rem; }

/* ---- card ----
   Border, generous padding, one shallow shadow. The two-layer lift the page
   had before reads as a modal floating over something; a report is not
   floating over anything. */
section { background: var(--card); border: 1px solid var(--border);
          border-radius: var(--radius-xl); padding: 1.5rem; margin: 0;
          box-shadow: 0 1px 2px rgba(51,38,27,.06); }
/* Title and one line on what the card says, before anything it says. */
.card-head { margin: 0 0 1.25rem; }
.card-head h2 { font: 600 17px/1.3 Figtree, system-ui, sans-serif;
                letter-spacing: -.01em; color: var(--foreground); margin: 0; }
.desc { color: var(--muted-foreground); font-size: 14px; margin: .25rem 0 0; }
section > p { margin: 0 0 1rem; }

/* ---- table ----
   In its own bordered box, headers muted and sentence-cased rather than
   shouted, rows that answer to the pointer. Numbers stay right-aligned: that
   is a column of figures, whatever the design language. */
.tw { border: 1px solid var(--border); border-radius: var(--radius-md);
      overflow-x: auto; }
table { width: 100%; border-collapse: collapse; font-size: 14px;
        font-variant-numeric: tabular-nums; }
th, td { text-align: right; padding: .7em .9em;
         border-bottom: 1px solid var(--border); }
th:first-child, td:first-child { text-align: left; }
thead th { font-weight: 500; color: var(--muted-foreground); white-space: nowrap; }
tbody tr { transition: background .12s ease; }
tbody tr:hover { background: var(--muted); }
tbody tr:last-child td { border-bottom: 0; }
.rolls th, .rolls td { padding: .7em .45em; }
/* Anything that explains itself on hover says so, quietly. */
thead th[title], .desc[title] { cursor: help;
                  text-decoration: underline dotted #BFAF9C;
                  text-underline-offset: 4px; }
/* The totals, ruled off above and set in the ink of a figure that matters. */
tfoot td { border-top: 1px solid var(--border); border-bottom: 0;
           font-weight: 600; color: var(--foreground); }
tbody tr:last-child td { border-bottom: 0; }
tfoot tr:hover { background: transparent; }
/* What a thing was worth, beside how many of it there were. */
.worth { color: var(--muted-foreground); }

/* ---- badge ---- */
.tag { display: inline-block; padding: .05em .45em; border-radius: var(--radius-sm);
       background: var(--primary); color: var(--primary-foreground);
       font-size: 12px; font-weight: 600; letter-spacing: .01em;
       vertical-align: 1px; }
.up { color: var(--positive); font-weight: 600; }
.down { color: var(--primary); font-weight: 600; }

/* ---- footnote ----
   The long explanation belongs after the thing it explains and below a rule,
   so the figures are not read through it. */
.note { color: var(--muted-foreground); font-size: 14px;
        margin: 1.25rem 0 0; padding-top: 1rem;
        border-top: 1px solid var(--border); }
/* A note that *is* the card's content has nothing above it to be ruled off. */
.card-head + .note { margin: 0; padding: 0; border-top: 0; }
/* Prose picks up again below a table rather than butting against it. */
.tw + p { margin: 1rem 0 0; }
section > p:last-child { margin-bottom: 0; }

/* ---- the turn bar ----
   Always the full width: every segment grows from a basis of nothing, so the
   widths are shares of the game and never of a scale the reader cannot see.
   No gaps between the segments: play goes round the table, so neighbours are
   never the same colour and a gap would only spend width on nothing. */
.bar { display: flex; width: 100%; height: 2.5rem; margin: 0 0 1.25rem;
       border-radius: var(--radius-md); overflow: hidden;
       background: var(--muted); }
.seg { flex: 1 1 0; min-width: 1px; background: var(--muted-foreground); }
.seg:hover { filter: brightness(1.15); }
.dot { display: inline-block; width: .55em; height: .55em; border-radius: 50%;
       margin-right: .55em; background: var(--muted-foreground);
       vertical-align: baseline; }
.s0 { background: var(--p0); } .s1 { background: var(--p1); }
.s2 { background: var(--p2); } .s3 { background: var(--p3); }
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
            times: s.times().to_vec(),
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
        // The result table is a decomposition of the score, so it names the
        // five things that score and not the things that do not.
        assert!(html.contains("victory points"));
        assert!(html.contains("largest militia"));
        assert!(!html.contains(">roads<"), "roads score nothing (R-11.3)");
        // The sections that were asked for, by their headings.
        for heading in [
            "The result",
            "The turns",
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
    fn a_scoring_column_says_how_many_and_what_they_were_worth() {
        let g = played(2);
        let s = study(&g, std::slice::from_ref(&g)).expect("it studies");
        let html = page(&g, &s);
        // Every column of the result table carries its own rule rather than a
        // paragraph under the table carrying all five.
        assert!(
            html.contains("<th title="),
            "the columns explain themselves"
        );
        assert!(!html.contains("Points, not pieces"), "and the note is gone");
        // A city held reads as the count with its two points in brackets.
        let cities = s.points[..s.report.players as usize]
            .iter()
            .map(|p| p.cities)
            .find(|c| c.held > 0)
            .expect("somebody built a city");
        assert_eq!(cities.points, cities.held * 2);
        assert!(
            html.contains(&format!(
                "{} <span class=\"worth\">({})</span>",
                cities.held, cities.points
            )),
            "the count, then what it was worth"
        );
        assert_eq!(scored(crate::analysis::Scored::default()), "&middot;");
    }

    #[test]
    fn the_bar_is_one_segment_per_turn() {
        let g = played(4);
        let s = study(&g, std::slice::from_ref(&g)).expect("it studies");
        let html = page(&g, &s);
        assert_eq!(
            html.matches("class=\"seg s").count(),
            s.turns.len(),
            "a segment for every turn and no more"
        );
        // Sized by what happened, not by an axis the reader cannot see.
        for t in &s.turns {
            assert!(html.contains(&format!("flex-grow:{}", t.actions)));
        }
        // A game played through a session carries its clock, so the column is
        // there and the row adds up to the whole of it.
        assert!(s.timed, "a game played here is a game that was timed");
        let spent: u32 = s.turns.iter().map(|t| t.millis).sum();
        assert!(
            html.contains(&clock(spent)),
            "the totals row carries the time"
        );
        // Explanations live on the columns now, not under the table.
        assert!(!html.contains("class=\"note\">Length is measured"));
        assert!(html.contains("Wall-clock time inside their turns"));
    }

    #[test]
    fn a_total_is_shown_only_where_a_column_has_one() {
        let g = played(6);
        let s = study(&g, std::slice::from_ref(&g)).expect("it studies");
        let html = page(&g, &s);
        // Every table that claims a total has one row of them and no more.
        assert_eq!(
            html.matches("<tfoot>").count(),
            html.matches("</tfoot>").count()
        );
        assert!(html.matches("<tfoot>").count() >= 5, "the summable tables");
        // The turns add across: the seats' moves are the game's moves.
        let moves: u32 = s.turns.iter().map(|t| t.actions).sum();
        assert!(html.contains(&format!("<td>{moves}</td>")));
        // A maximum is not totalled, and says so on the column.
        assert!(html.contains("A maximum, so it has no total."));
    }

    #[test]
    fn every_long_explanation_is_a_tooltip_now() {
        // Two games, so the corpus card is on the page to be checked too.
        let history: Vec<Saved> = (7..9u64).map(played).collect();
        let s = study(&history[1], &history).expect("it studies");
        let html = page(&history[1], &s);
        assert!(html.contains("Across every game here"), "the corpus card");
        // The paragraphs that used to sit under the tables are gone, and what
        // they said is on the card or the column it was about.
        for gone in [
            "Read along a row",
            "dots on every number",
            "Played, not held",
            "Counted for both sides",
            "Expected is what the buildings",
            "No p-value, deliberately",
            "Seat, not player",
        ] {
            assert!(!html.contains(&format!(">{gone}")), "{gone} is not prose");
            assert!(html.contains(gone), "{gone} is still said, in a tooltip");
        }
        // And no explanatory paragraph survives anywhere but the two empty
        // states, which are answers rather than explanations.
        let notes = html.matches("class=\"note\"").count();
        assert!(notes <= 1, "{notes} notes left on the page");
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
        assert!(html.contains("no other finished games here"));
        assert!(!html.contains("Across every game here"));
    }
}
