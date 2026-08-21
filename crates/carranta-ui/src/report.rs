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

use carranta_core::state::PORT_KINDS;

use crate::analysis::{Study, Trades, seat_names};
use crate::store::Saved;

const RESOURCE_NAMES: [&str; 5] = ["brick", "wood", "wool", "wheat", "ore"];
/// Where the victory point card sits in `DEV_NAMES` and in every per-card
/// array the analytics keep.
const VICTORY_POINT: usize = 1;

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

/// The header every page in this application wears.
///
/// One shape across the board, the report and the home page: the mark, what you
/// are looking at, and the ways out of it, pushed to the far end so the mark and
/// the name stay together as one thing. A header that changes between pages of
/// one application reads as three applications, and this one had: the board had
/// a mark, a table name and two links, the report had a mark and one bare `nav`
/// link in the body face, and the home page had none at all.
///
/// `context` is what this page is about beside the mark, or empty. `links` are
/// href and label pairs, in the order they should be read.
///
/// The board page carries the same markup by hand, in `assets/index.html`,
/// because that page is one file with its own stylesheet; the classes and the
/// rules are named the same on both sides so the two cannot drift far without
/// somebody noticing.
/// The tab icon, as the board page carries it.
///
/// A hex, inline, because there is no request to make for it: the board page has
/// had this for as long as it has existed and the server-rendered pages had
/// nothing, so every visit to the home page or a report asked for
/// `/favicon.ico`, got a 404, and put it in the console. The same markup on both
/// sides rather than a file to keep in step with itself.
pub(crate) const ICON: &str = "<link rel=\"icon\" href=\"data:image/svg+xml,\
     %3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 32 32'%3E\
     %3Cpolygon points='16,2 29,9 29,23 16,30 3,23 3,9' fill='%23E8542F'/%3E\
     %3C/svg%3E\">";

pub(crate) fn masthead(context: &str, links: &[(&str, &str)]) -> String {
    masthead_as(context, links, "")
}

/// The same, carrying the account strip, which belongs at the top right of
/// every page and not only the front one: signing in is about who you are
/// wherever you are, and a strip that vanished when you left home read as
/// being signed out.
pub(crate) fn masthead_as(context: &str, links: &[(&str, &str)], account: &str) -> String {
    mast(
        "<a class=\"mark\" href=\"/\">Carranta</a>",
        context,
        links,
        account,
    )
}

/// The same header for the page it would lead to.
///
/// Home is the one page whose title is the name of the thing, so there the mark
/// is the heading rather than a link: a link to the page you are already on is
/// an offer of nothing, and a page needs a heading more than it needs that.
/// Takes the account strip as well as its links, because the home page is the
/// one place accounts appear: the board and the report are about a game and have
/// no business asking anybody to sign in.
pub(crate) fn masthead_home(links: &[(&str, &str)], account: &str) -> String {
    mast("<h1 class=\"mark\">Carranta</h1>", "", links, account)
}

fn mast(mark: &str, context: &str, links: &[(&str, &str)], tail: &str) -> String {
    // Which build is serving this, beside the mark, dim and small. It was already
    // in every payload for exactly this reason and rendered nowhere, so the one
    // question a stale process makes somebody ask, "am I even looking at the new
    // code", could only be answered from a terminal. An afternoon went on that.
    let mut b = format!(
        "<header>{mark}<span class=\"build\">{}</span>",
        crate::stamp::build()
    );
    if !context.is_empty() {
        let _ = write!(b, "<span class=\"gameName\">{}</span>", esc(context));
    }
    if !links.is_empty() || !tail.is_empty() {
        b.push_str("<div class=\"headLinks\">");
        for (href, label) in links {
            let _ = write!(b, "<a class=\"headLink\" href=\"{href}\">{label}</a>");
        }
        b.push_str(tail);
        b.push_str("</div>");
    }
    b.push_str("</header>");
    b
}

/// A card's header: the title, and the card's rule behind it.
///
/// A card is a title and a table. The rule that used to sit in a paragraph
/// under the table hangs off the heading instead, under a dotted underline: a
/// note below the figures is read through to reach them, or not read at all,
/// while a tooltip is one hover from the reader who wants it and invisible to
/// the one who does not.
///
/// There is no subtitle. A sentence describing a card the reader is already
/// looking at is a sentence they read to learn nothing, and a sentence carrying
/// a figure is a figure that belongs in the table.
fn card_head(title: &str, why: &str) -> String {
    format!(
        "<div class=\"card-head\"><h2 data-tip=\"{}\">{title}</h2></div>",
        esc(why)
    )
}

/// A seat, named, behind the colour it played in and before where it finished.
///
/// The colour is the mark the board plays in, always immediately left of the
/// name, so a row can be found by colour rather than by reading down the names.
/// Every table names the same four people, and the thing a reader wants beside
/// a name is where it came. A badge on the winner alone answered that for one
/// of them and left the other three to be worked out from a column of points.
fn placed(seat: usize, name: &str, place: Option<usize>) -> String {
    let badge = match place {
        // First place is the win, so it keeps the colour the win had.
        Some(1) => " <span class=\"tag\">1st</span>".to_string(),
        Some(n) => format!(" <span class=\"tag quiet\">{}</span>", ordinal(n)),
        None => String::new(),
    };
    format!(
        "<span class=\"dot s{}\"></span>{}{badge}",
        seat.min(MAX_PLAYERS - 1),
        esc(name)
    )
}

/// Where each seat finished, best first.
///
/// The winner leads however the points fell, since they are the one who reached
/// ten and ended it. Everybody else is ordered on final points, and a tie shares
/// a place rather than being broken by seat number, which would be inventing an
/// order the game never played.
fn places(r: &carranta_analytics::game::Report, seats: usize) -> [Option<usize>; MAX_PLAYERS] {
    let won = |s: usize| r.winner == Some(s as u8);
    let mut order: Vec<usize> = (0..seats).collect();
    order.sort_by(|a, b| won(*b).cmp(&won(*a)).then(r.vp[*b].cmp(&r.vp[*a])));
    let mut out = [None; MAX_PLAYERS];
    let mut place = 1;
    for (i, &s) in order.iter().enumerate() {
        let previous = i.checked_sub(1).map(|j| order[j]);
        if previous.is_some_and(|q| won(q) || r.vp[q] != r.vp[s]) {
            place = i + 1;
        }
        out[s] = Some(place);
    }
    out
}

/// Nothing, written as nothing.
///
/// A column with no value in it is left blank rather than filled with a mark
/// standing in for the absence. The blank already says it.
const NONE: &str = "";

/// A subtotal, ruled off from the rows it adds up.
fn sub_row(label: &str, cells: &[String]) -> String {
    let mut out = format!("<tr class=\"sub\"><td>{label}</td>");
    for c in cells {
        let _ = write!(out, "<td>{c}</td>");
    }
    out.push_str("</tr>");
    out
}

/// A totals row, in the table's foot.
///
/// Only where a column adds up to something true. A maximum, a rate and a
/// percentile do not, and a row that totalled them would be read as though they
/// did, so those cells are left blank.
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
            let _ = write!(out, "<th data-tip=\"{}\">{label}</th>", esc(why));
        }
    }
    out.push_str("</tr>");
    out
}

/// An opening's pips, a row per resource and a hex per dot.
///
/// A row rather than a number, because "22 pips" says how much and says nothing
/// about what. Five rows always, so the rows line up down the column and a
/// resource nobody can produce reads as the gap it is.
fn pip_rows(pips: &[u32; 5]) -> String {
    let mut out = String::from("<span class=\"pips\">");
    for (res, n) in pips.iter().enumerate() {
        let _ = write!(
            out,
            "<span class=\"pip-row\" data-tip=\"{n} {name} pips\">",
            name = RESOURCE_NAMES[res],
        );
        for _ in 0..*n {
            let _ = write!(out, "{}", hex(&format!("on r{res}")));
        }
        out.push_str("</span>");
    }
    // The whole placement, under a rule, as the number rather than as more
    // hexes: twenty-two tiles in a row says less than the figure 22 does, and
    // the total is what one opening is compared with another on.
    let _ = write!(
        out,
        "<span class=\"pip-row sum\" data-tip=\"Every pip the two \
         settlements touch.\">{}</span>",
        pips.iter().sum::<u32>(),
    );
    out.push_str("</span>");
    out
}

/// The same pips as cards a turn, on the same five rows.
fn turn_rows(per_turn: &[f64; 5]) -> String {
    let mut out = String::from("<span class=\"pips rates\">");
    for (res, v) in per_turn.iter().enumerate() {
        let _ = write!(
            out,
            "<span class=\"pip-row\" data-tip=\"{name}\">{}</span>",
            if *v < 0.005 {
                NONE.to_string()
            } else {
                format!("{v:.2}")
            },
            name = RESOURCE_NAMES[res],
        );
    }
    // On the same line as the pips it is the same total of, so the two columns
    // still read across.
    let _ = write!(
        out,
        "<span class=\"pip-row sum\" data-tip=\"Cards a turn from the whole \
         placement, at fair odds.\">{:.2}</span>",
        per_turn.iter().sum::<f64>(),
    );
    out.push_str("</span>");
    out
}

/// The numbers a placement sits on, as the board draws them.
///
/// Six and eight in the board's own red, because on a board they are the two
/// everybody looks for and reading them in the same ink as the rest would lose
/// what the colour is for.
fn discs(numbers: &[u8]) -> String {
    if numbers.is_empty() {
        return NONE.to_string();
    }
    let mut out = String::from("<span class=\"discs\">");
    for n in numbers {
        let hot = *n == 6 || *n == 8;
        let _ = write!(
            out,
            "<span class=\"disc{}\" data-tip=\"{n} comes up on {ways} rolls in \
             36\">{n}</span>",
            if hot { " hot" } else { "" },
            ways = 6 - (i32::from(*n) - 7).abs(),
        );
    }
    out.push_str("</span>");
    out
}

/// Ports, at the rate they trade.
fn port_marks(ports: &[Option<usize>]) -> String {
    if ports.is_empty() {
        return NONE.to_string();
    }
    let mut out = String::from("<span class=\"discs\">");
    for p in ports {
        match p {
            Some(res) => {
                let _ = write!(
                    out,
                    "<span class=\"port r{res}\" data-tip=\"two to one, {}\">2:1</span>",
                    RESOURCE_NAMES[*res],
                );
            }
            None => out.push_str(
                "<span class=\"port any\" data-tip=\"three to one, anything\">3:1</span>",
            ),
        }
    }
    out.push_str("</span>");
    out
}

/// One board tile, at the size of a line of text.
///
/// It says nothing on hover of its own: a tile is one dot of a row, and the
/// row it belongs to is what carries the explanation.
fn hex(class: &str) -> String {
    // A flat-top hex, the shape the board is made of.
    const PATH: &str = "M9 0 L18 5.2 L18 15.6 L9 20.8 L0 15.6 L0 5.2 Z";
    format!(
        "<svg viewBox=\"-1 -1 20 22.8\" class=\"tile\">\
         <path class=\"{class}\" d=\"{PATH}\"/></svg>"
    )
}

/// A tooltip belonging to a shape in a drawing, laid over the drawing rather
/// than inside it.
///
/// Two things rule out putting it in the picture. SVG has no pseudo-elements
/// and no text that wraps, so the box would have to be a `foreignObject`; and
/// a `foreignObject` with any SVG geometry painted after it is dropped behind
/// the whole drawing by the browser, which is exactly the case here, since
/// every shape after this one is geometry. Over the top it is ordinary page
/// HTML at the page's own size, which is also how it comes out looking like
/// the tooltips in the tables rather than like a drawing of one.
///
/// The position is a percentage of the drawing's own box, so it holds wherever
/// the drawing is scaled to. Which way it opens is settled here rather than in
/// the stylesheet: near an edge it opens back towards the middle.
fn over_tip(i: usize, x: f64, y: f64, w: f64, h: f64, text: &str) -> String {
    let (left, up) = (x > w / 2.0, y > h / 2.0);
    format!(
        "<div class=\"tipat t{i}{}{}\" style=\"left:{lx:.2}%;top:{ly:.2}%\">\
         <span class=\"tipin\">{}</span></div>",
        if left { " to-left" } else { "" },
        if up { " up" } else { "" },
        esc(text),
        lx = 100.0 * x / w,
        ly = 100.0 * y / h,
    )
}

/// The rules that tie those tooltips to their shapes.
///
/// One a shape, which is the price of doing this without a script: CSS can ask
/// whether a drawing holds a hovered shape, but it cannot be told *which* one
/// without a selector naming it. They are written beside the drawing they
/// belong to rather than in the stylesheet, since only the drawing knows how
/// many shapes it ended up with.
fn tip_rules(scope: &str, n: usize) -> String {
    let mut out = String::from("<style>");
    for i in 0..n {
        let _ = write!(out, "{scope}:has(.k{i}:hover) .t{i}{{display:block}}");
    }
    out.push_str("</style>");
    out
}

/// A count with a second, smaller count in brackets after it.
///
/// The same shape the result table uses for points: two figures that belong
/// together, the one being read off the other.
fn bracketed(all: u32, some: u32) -> String {
    if all == 0 {
        NONE.to_string()
    } else {
        format!("{all} <span class=\"worth\">({some})</span>")
    }
}

/// One scoring column: how many, and what they were worth.
///
/// Nought is blank, since a zero in a column of scores is nothing rather than a
/// number to read.
fn scored(s: crate::analysis::Scored) -> String {
    if s.held == 0 {
        NONE.to_string()
    } else {
        format!("{} <span class=\"worth\">({})</span>", s.held, s.points)
    }
}

/// Everybody at the table, in seat order, named.
///
/// From the chairs, which is the only record of who was actually in them. It
/// used to be "seat nought is the person and the rest are bots", which the draw
/// and the second human both made untrue: a report would name the winner Ines
/// while a house bot answered to the reader's own name.
fn names(saved: &Saved, seats: usize) -> Vec<String> {
    let mut named = seat_names(saved);
    named.resize(seats, String::new());
    named
}

/// A name with where it finished, as plain text.
///
/// For tooltips and titles, which cannot hold markup. Anywhere the badge can
/// go, [`placed`] puts it there instead.
fn label(name: &str, place: Option<usize>) -> String {
    match place {
        Some(n) => format!("{name} {}", ordinal(n)),
        None => name.to_string(),
    }
}

/// A label on a drawing, carrying the same markup a table row would.
///
/// The page's own HTML, in the layer over the drawing rather than inside it.
/// Inside it, as a `foreignObject`, everything was scaled with the drawing:
/// a name came out a size the page never sets, and a place badge came out a
/// different pill from the one in every table, which is the one thing a badge
/// must not do. Over the top it is the same badge, because it is the same
/// markup at the same size.
///
/// `at` is where the label points from, and the box is hung off the anchor
/// accordingly, so a name to the left of a circle still ends at the rim.
fn over_label(x: f64, y: f64, w: f64, h: f64, at: &str, inner: &str) -> String {
    let align = match at {
        "end" => "to-end",
        "mid" => "to-mid",
        _ => "to-start",
    };
    format!(
        "<div class=\"name {align}\" style=\"left:{lx:.2}%;top:{ly:.2}%\">{inner}</div>",
        lx = 100.0 * x / w,
        ly = 100.0 * y / h,
    )
}

/// Who took from whom, drawn.
///
/// A sankey: thieves down the left, victims down the right, and a ribbon
/// between each pair as thick as the cards that moved along it. The grid below
/// says the same thing exactly; this says it at a glance, which is a different
/// and also useful thing for a table of numbers to be paired with.
///
/// Laid out here rather than by a script, because the page has none and this
/// needs none: every position is a fraction of a total that is known the moment
/// the game ends.
fn sankey(
    r: &carranta_analytics::game::Report,
    who: &[String],
    place: &[Option<usize>],
    seats: usize,
) -> String {
    // The drawing's own coordinates, scaled to whatever width it is given.
    const W: f64 = 720.0;
    const H: f64 = 232.0;
    const TOP: f64 = 14.0;
    const NODE: f64 = 12.0;
    const LEFT: f64 = 96.0;
    const RIGHT: f64 = W - 96.0;
    const GAP: f64 = 10.0;

    let took: Vec<u32> = (0..seats)
        .map(|s| (0..seats).map(|v| r.steals[s][v]).sum())
        .collect();
    let lost: Vec<u32> = (0..seats)
        .map(|v| (0..seats).map(|s| r.steals[s][v]).sum())
        .collect();
    let total: u32 = took.iter().sum();
    if total == 0 {
        return String::new();
    }

    // One scale for both sides, so a ribbon is the same thickness at each end.
    // Two scales would draw a card as larger where it landed than where it
    // left, which is a picture of something that did not happen.
    let widest = [&took, &lost]
        .iter()
        .map(|v: &&Vec<u32>| v.iter().filter(|n| **n > 0).count())
        .max()
        .unwrap_or(1);
    let scale = (H - GAP * widest.saturating_sub(1) as f64) / f64::from(total);

    // Where each seat's block sits on its side, centred on the drawing.
    let stack = |totals: &[u32]| -> Vec<(f64, f64)> {
        let shown = totals.iter().filter(|n| **n > 0).count();
        let height =
            f64::from(totals.iter().sum::<u32>()) * scale + GAP * shown.saturating_sub(1) as f64;
        let mut y = TOP + (H - height) / 2.0;
        totals
            .iter()
            .map(|n| {
                if *n == 0 {
                    return (y, y);
                }
                let block = (y, y + f64::from(*n) * scale);
                y = block.1 + GAP;
                block
            })
            .collect()
    };
    let from = stack(&took);
    let to = stack(&lost);

    // The drawing, and over it the layer its tooltips live in.
    let mut b = format!(
        "<div class=\"flow\"><div class=\"frame\"><svg viewBox=\"0 0 {W} {h}\" \
         role=\"img\" aria-label=\"Who took cards from whom\">",
        h = H + TOP * 2.0
    );
    let mut tips = String::from("<div class=\"over\">");
    let mut shape = 0;

    // The ribbons first, so the blocks and names sit over them.
    let mut out_at: Vec<f64> = from.iter().map(|(y, _)| *y).collect();
    let mut in_at: Vec<f64> = to.iter().map(|(y, _)| *y).collect();
    for thief in 0..seats {
        for victim in 0..seats {
            let n = r.steals[thief][victim];
            if n == 0 {
                continue;
            }
            let thick = f64::from(n) * scale;
            let (a, c) = (out_at[thief], in_at[victim]);
            let (bmid, cmid) = (LEFT + NODE + 60.0, RIGHT - NODE - 60.0);
            let (x0, x1) = (LEFT + NODE, RIGHT - NODE);
            let d = format!(
                "M{x0} {a} C{bmid} {a} {cmid} {c} {x1} {c} \
                 L{x1} {c2} C{cmid} {c2} {bmid} {a2} {x0} {a2} Z",
                a2 = a + thick,
                c2 = c + thick,
            );
            let _ = write!(b, "<path class=\"ribbon f{thief} k{shape}\" d=\"{d}\"/>");
            tips.push_str(&over_tip(
                shape,
                (x0 + x1) / 2.0,
                (a + c) / 2.0 + thick / 2.0,
                W,
                H + TOP * 2.0,
                &format!("{} took {n} from {}", who[thief], who[victim]),
            ));
            shape += 1;
            out_at[thief] += thick;
            in_at[victim] += thick;
        }
    }

    // The blocks, and a name beside each: the block in the drawing, the name in
    // the layer over it, at the page's own size.
    let tall = H + TOP * 2.0;
    for s in 0..seats {
        if took[s] > 0 {
            let _ = write!(
                b,
                "<rect class=\"node n{s}\" x=\"{x}\" y=\"{y}\" width=\"{NODE}\" \
                 height=\"{h}\" rx=\"3\"/>",
                x = LEFT,
                y = from[s].0,
                h = from[s].1 - from[s].0,
            );
            tips.push_str(&over_label(
                LEFT - 10.0,
                (from[s].0 + from[s].1) / 2.0,
                W,
                tall,
                "end",
                &placed(s, &who[s], place[s]),
            ));
        }
        if lost[s] > 0 {
            let _ = write!(
                b,
                "<rect class=\"node n{s}\" x=\"{x}\" y=\"{y}\" width=\"{NODE}\" \
                 height=\"{h}\" rx=\"3\"/>",
                x = RIGHT - NODE,
                y = to[s].0,
                h = to[s].1 - to[s].0,
            );
            tips.push_str(&over_label(
                RIGHT + 10.0,
                (to[s].0 + to[s].1) / 2.0,
                W,
                tall,
                "start",
                &placed(s, &who[s], place[s]),
            ));
        }
    }
    b.push_str("</svg>");
    tips.push_str("</div>");
    b.push_str(&tips);
    b.push_str(&tip_rules(".flow", shape));
    b.push_str("</div></div>");
    b
}

/// A hand of cards, named. `4 wheat`, `2 wood and 1 ore`.
fn hand_text(cards: &[u8; 5]) -> String {
    let parts: Vec<String> = cards
        .iter()
        .enumerate()
        .filter(|(_, n)| **n > 0)
        .map(|(r, n)| format!("{n} {}", RESOURCE_NAMES[r]))
        .collect();
    match parts.len() {
        0 => "nothing".to_string(),
        1 => parts[0].clone(),
        _ => format!(
            "{} and {}",
            parts[..parts.len() - 1].join(", "),
            parts[parts.len() - 1]
        ),
    }
}

/// Every trade in the game as a circle of who dealt with whom.
///
/// Nodes round the rim, each arc as long as that party's trades; one ribbon
/// across the middle **per trade**, not per pair, so a thick band is a run of
/// deals rather than a number you have to hover to read. The bank and the ports
/// are parties too, since a trade with the supply is still a trade and leaving
/// it out would draw a market smaller than the one played.
///
/// A chord rather than a sankey because trading is symmetric: there is no side
/// a trade goes from and no side it goes to, and drawing one would invent a
/// direction the game does not have.
fn chord(study: &Study, who: &[String], place: &[Option<usize>], seats: usize) -> String {
    // Wider than tall: the circle is round but the names beside it are not, so
    // a square box spends its height on nothing.
    const W: f64 = 560.0;
    const H: f64 = 400.0;
    const R: f64 = 150.0;
    const BAND: f64 = 12.0;
    const GAP: f64 = 0.05;

    let tr = &study.trades;
    if tr.deals.is_empty() {
        return String::new();
    }

    // The parties, in the order they go round: the seats, then the counters.
    let parties: Vec<usize> = (0..seats)
        .chain([Trades::BANK, Trades::PORT])
        .filter(|p| tr.ends(*p) > 0)
        .collect();
    let seat_of = |party: usize| parties.iter().position(|p| *p == party);
    // One unit of arc per end of one trade, and every trade has two.
    let units = (tr.deals.len() * 2) as f64;
    let span = std::f64::consts::TAU - GAP * parties.len() as f64;
    let unit = span / units;

    let mut arc = vec![(0.0, 0.0); parties.len()];
    let mut at = -std::f64::consts::FRAC_PI_2;
    for (i, party) in parties.iter().enumerate() {
        let width = unit * tr.ends(*party) as f64;
        arc[i] = (at, at + width);
        at += width + GAP;
    }

    let (mx, my) = (W / 2.0, H / 2.0);
    let point = |angle: f64, radius: f64| (mx + radius * angle.cos(), my + radius * angle.sin());
    let mut b = format!(
        "<div class=\"ring\"><div class=\"frame\"><svg viewBox=\"0 0 {W} {H}\" \
         role=\"img\" aria-label=\"Who traded with whom\">"
    );

    // Grouped by pair so the ribbons of one pair lie together rather than
    // crossing each other on their way to the same arc.
    let mut order: Vec<(usize, usize, usize)> = tr
        .deals
        .iter()
        .enumerate()
        .filter_map(|(i, d)| {
            let (a, c) = (seat_of(d.seat)?, seat_of(d.with)?);
            Some((a.min(c), a.max(c), i))
        })
        .collect();
    order.sort_unstable();

    let mut tips = String::from("<div class=\"over\">");
    let mut shape = 0;
    let mut cursor: Vec<f64> = arc.iter().map(|(a, _)| *a).collect();
    for (a, c, i) in order {
        let d = &tr.deals[i];
        let (a0, a1) = (cursor[a], cursor[a] + unit);
        let (c0, c1) = (cursor[c], cursor[c] + unit);
        cursor[a] = a1;
        cursor[c] = c1;
        let (x0, y0) = point(a0, R);
        let (x1, y1) = point(a1, R);
        let (u0, v0) = point(c0, R);
        let (u1, v1) = point(c1, R);
        let counter = match d.with {
            w if w == Trades::BANK => "with the bank".to_string(),
            w if w == Trades::PORT => "at a port".to_string(),
            w => format!("with {}", who[w]),
        };
        // Through the centre, which is what makes a chord read as one link
        // rather than two lines that happen to meet.
        let path = format!(
            "M{x0:.1} {y0:.1} A{R} {R} 0 0 1 {x1:.1} {y1:.1} \
             Q{mx} {my} {u1:.1} {v1:.1} A{R} {R} 0 0 1 {u0:.1} {v0:.1} \
             Q{mx} {my} {x0:.1} {y0:.1} Z"
        );
        let seat = d.seat.min(MAX_PLAYERS - 1);
        let _ = write!(b, "<path class=\"chord f{seat} k{shape}\" d=\"{path}\"/>");
        tips.push_str(&over_tip(
            shape,
            (x0 + u0) / 2.0,
            (y0 + v0) / 2.0,
            W,
            H,
            &format!(
                "Turn {}: {} gave {}, took {}, {counter}",
                d.turn,
                who[d.seat],
                hand_text(&d.gave),
                hand_text(&d.took),
            ),
        ));
        shape += 1;
    }

    // The rim, and a name outside each arc.
    for (i, party) in parties.iter().enumerate() {
        let (a0, a1) = arc[i];
        let (x0, y0) = point(a0, R);
        let (x1, y1) = point(a1, R);
        let (x2, y2) = point(a1, R + BAND);
        let (x3, y3) = point(a0, R + BAND);
        let class = if *party < seats {
            format!("n{party}")
        } else {
            "supply".to_string()
        };
        let band = format!(
            "M{x0:.1} {y0:.1} A{R} {R} 0 0 1 {x1:.1} {y1:.1} \
             L{x2:.1} {y2:.1} A{r2} {r2} 0 0 0 {x3:.1} {y3:.1} Z",
            r2 = R + BAND,
        );
        let _ = write!(b, "<path class=\"rim {class} k{shape}\" d=\"{band}\"/>");
        let centre = (a0 + a1) / 2.0;
        let (tx, ty) = point(centre, R + BAND + 14.0);
        tips.push_str(&over_tip(
            shape,
            tx,
            ty,
            W,
            H,
            &format!("{} trades", tr.ends(*party)),
        ));
        shape += 1;
        let name = match *party {
            w if w == Trades::BANK => "the bank".to_string(),
            w if w == Trades::PORT => "ports".to_string(),
            w => placed(w, &who[w], place[w]),
        };
        // A name runs outwards from the rim, so which way it is hung depends on
        // where round the circle it sits. At the top and bottom neither end is
        // outwards, and it is centred.
        let at = match centre.cos() {
            c if c < -0.3 => "end",
            c if c > 0.3 => "start",
            _ => "mid",
        };
        tips.push_str(&over_label(tx, ty, W, H, at, &name));
    }
    b.push_str("</svg>");
    tips.push_str("</div>");
    b.push_str(&tips);
    b.push_str(&tip_rules(".ring", shape));
    b.push_str("</div></div>");
    b
}

/// How the game was divided, seat by seat.
fn turn_bar(study: &Study, who: &[String], place: &[Option<usize>], seats: usize) -> String {
    let mut b = String::from("<section>");
    b.push_str(&card_head(
        "Turns",
        "A turn is what falls between two ends of turn, and it counts \
         everything that landed inside it, the turn holder's or not: a \
         discard, a robbery and an accepted offer all happen in somebody's \
         turn. The setup placements are left out, since they come before \
         anybody has a turn to take.",
    ));
    let turns = &study.turns;
    if turns.is_empty() {
        b.push_str("<p class=\"note\">Nobody finished a turn in this game.</p></section>");
        return b;
    }

    let spent: u32 = turns.iter().map(|t| t.millis).sum();

    b.push_str(T_OPEN);
    b.push_str("<thead>");
    let mut heads = vec![("", ""), ("turns", "Turns this seat took.")];
    if study.timed {
        heads.push((
            "time",
            "Wall-clock time inside their turns, which is their own thinking \
             plus whatever the table made them wait for. A turn counts \
             everything that landed in it, so this includes what the table made \
             them wait for as well. A game the computer played out to itself \
             takes almost none.",
        ));
        heads.push((
            "share",
            "Their share of the game's time, which is the figure worth reading \
             here: everybody gets about the same number of turns, and nobody \
             takes the same time over them.",
        ));
    }
    b.push_str(&head_row(&heads));
    b.push_str("</thead><tbody>");
    for s in 0..seats {
        let mine: Vec<&crate::analysis::Turn> = turns.iter().filter(|t| t.seat == s).collect();
        let mut cells = vec![placed(s, &who[s], place[s]), mine.len().to_string()];
        if study.timed {
            let millis: u32 = mine.iter().map(|t| t.millis).sum();
            cells.push(clock(millis));
            // A game that took no measurable time divides by nought otherwise,
            // which is every game the computer played to itself.
            cells.push(if spent == 0 {
                NONE.to_string()
            } else {
                format!("{:.0}%", 100.0 * millis as f64 / spent as f64)
            });
        }
        b.push_str(&row(&cells, false));
    }
    b.push_str("</tbody>");
    let mut foot = vec!["the game".to_string(), turns.len().to_string()];
    if study.timed {
        foot.push(clock(spent));
        foot.push(if spent == 0 {
            NONE.to_string()
        } else {
            "100%".to_string()
        });
    }
    b.push_str(&totals(&foot));
    b.push_str(T_CLOSE);
    b.push_str(&clock_split(study));
    b.push_str("</section>");
    b
}

/// One thing drawn: what arrived, and what was expected.
struct Curve {
    /// The colour class, shared by the solid line and the dotted one.
    colour: String,
    /// Plain, for the titles and the per-turn tooltip, which cannot hold
    /// markup.
    name: String,
    /// The same thing with its place badge, for the legend, which can.
    badge: String,
    actual: Vec<f64>,
    owed: Vec<f64>,
}

/// The gap between what the board owed a seat and what it paid them, split into
/// the three things that make it (§10.2).
///
/// The deviation column on the production card is one number standing for three
/// causes that mean completely different things:
///
/// ```text
/// arrived = expected - robber - supply + dice
/// ```
///
/// `dice` is chance. `robber` is other players choosing to sit on your hexes,
/// which is a social outcome and not a random one. `supply` is a rules artefact
/// (R-5.6): cards owed that the stack could not pay. Reported as one figure they
/// tell a player nothing about which of the three happened to them, and the
/// three have different answers: shrug, play differently, or nothing at all.
///
/// The engine has computed this from the first day and the page never showed it.
/// It is exact rather than estimated, including the standard deviation the dice
/// column is measured in: production on one roll has a known distribution over
/// eleven outcomes, and rolls are independent, so the variances add even as the
/// buildings change under them.
///
/// Rolls only, which is why `arrived` here is a card or two under the ledger's
/// production row: the opening settlements paid before anybody rolled, and a
/// payout with no dice in it belongs in neither the expectation nor the luck.
fn deviation_card(study: &Study, who: &[String], place: &[Option<usize>], seats: usize) -> String {
    let pr = &study.production;
    if pr.rolls == 0 || seats == 0 {
        return String::new();
    }
    let mut b = String::from("<section>");
    b.push_str(&card_head(
        "Deviation",
        "What the board owed each seat over every roll, and what became of it \
         on the way to their hand. The robber is somebody's decision, the \
         supply is a rule, and only the dice column is chance, so the three are \
         split apart rather than added into one number. Rolls only: the opening \
         settlements paid before anybody rolled.",
    ));
    b.push_str(T_OPEN);
    b.push_str("<thead>");
    b.push_str(&head_row(&[
        ("", ""),
        (
            "expected",
            "Cards the pips through their buildings owed them over every roll of \
             the game, at fair odds, with the robber ignored.",
        ),
        (
            "robber",
            "Cards that expectation lost to the robber sitting on their hexes \
             (R-5.8). Not chance: this is what the rest of the table chose to do \
             to them.",
        ),
        (
            "supply",
            "Cards a roll owed them that the stack could not pay (R-5.6). \
             Usually nothing, and never anybody's fault.",
        ),
        (
            "dice",
            "What the dice did, given where the robber actually sat: the one \
             genuinely random term. In brackets, the same figure in standard \
             deviations, which is exact rather than estimated. Beyond two is a \
             game worth remembering; beyond three is rare.",
        ),
        (
            "arrived",
            "Cards that reached their hand from a roll. The four columns to the \
             left add across to it exactly, which is what makes them a \
             decomposition rather than four numbers that happen to sit \
             together.",
        ),
    ]));
    b.push_str("</thead><tbody>");
    let mut foot = [0.0f64; 5];
    for p in 0..seats {
        let d = pr.decompose(p);
        for (slot, v) in foot.iter_mut().zip([
            d.e_raw,
            d.robber_cost,
            d.supply_denial,
            d.dice_luck,
            d.actual,
        ]) {
            *slot += v;
        }
        b.push_str(&row(
            &[
                placed(p, &who[p], place[p]),
                format!("{:.1}", d.e_raw),
                cost(d.robber_cost),
                cost(d.supply_denial),
                format!(
                    "<span class=\"{}\">{:+.1}</span> \
                     <span class=\"worth\">({:+.1} sd)</span>",
                    if d.dice_luck >= 0.0 { "up" } else { "down" },
                    d.dice_luck,
                    d.luck_z,
                ),
                format!("{:.0}", d.actual),
            ],
            false,
        ));
    }
    b.push_str("</tbody>");
    b.push_str(&totals(&[
        "the board".to_string(),
        format!("{:.1}", foot[0]),
        cost(foot[1]),
        cost(foot[2]),
        format!(
            "<span class=\"{}\">{:+.1}</span>",
            if foot[3] >= 0.0 { "up" } else { "down" },
            foot[3]
        ),
        format!("{:.0}", foot[4]),
    ]));
    b.push_str(T_CLOSE);
    b.push_str("</section>");
    b
}

/// A cost, written as the negative it is, and blank when there was none.
///
/// Nought robber cost happens on a board nobody blockaded, and a column of
/// `-0.0` reads as a figure rather than as the nothing it is.
fn cost(v: f64) -> String {
    if v < 0.05 {
        NONE.to_string()
    } else {
        format!("<span class=\"down\">-{v:.1}</span>")
    }
}

/// The game as a race: who led, for how long, and how long the last stretch took.
///
/// A final score says who won and by how much. It cannot say who was in front for
/// most of the game and lost it, or whether the winner was clear from turn forty,
/// or how many turns the table had to stop them once they were within two points.
/// The chart above shows all of that and asks to be read; this says it.
///
/// The true score throughout, hidden cards and all, because that is what actually
/// decided the game. The table could not see it, which is what the dotted lines
/// on the chart are for.
fn race(study: &Study, who: &[String], place: &[Option<usize>], seats: usize) -> String {
    let rows = &study.score;
    let turns = rows.len();
    if turns < 2 || seats == 0 {
        return String::new();
    }
    // Two points short of the target: the point at which a seat can win inside
    // one turn, and so the point from which the rest of the table is out of time.
    let near = carranta_core::state::WINNING_VP.saturating_sub(2);
    // The first turn a seat's score reached a mark, counting from one.
    let reached = |p: usize, mark: u32| {
        (0..turns)
            .find(|i| rows[*i][p] >= mark)
            .map(|i| i as u32 + 1)
    };
    // Turns this seat held the lead outright. A tie is nobody's lead: two seats
    // level on eight are both in front of the other two and neither is ahead.
    let led = |p: usize| {
        (0..turns)
            .filter(|i| {
                let mine = rows[*i][p];
                mine > 0 && (0..seats).all(|q| q == p || rows[*i][q] < mine)
            })
            .count() as u32
    };

    let mut b = String::from(T_OPEN);
    b.push_str("<thead>");
    b.push_str(&head_row(&[
        ("", ""),
        (
            "halfway",
            "The turn this seat first reached half the target. Early here is a \
             fast start and nothing more: the opening pays from the first roll and \
             the first few points are the cheap ones.",
        ),
        (
            "in reach",
            "The turn they first came within two points of the target, which is \
             the point from which they can win inside a single turn. Blank for a \
             seat that never got that close. The foot carries the first seat to \
             get there and how many turns the rest of the table then had to stop \
             them, which is the length of the endgame.",
        ),
        (
            "in front",
            "Turns they held the lead outright, with their share of the game in \
             brackets. A tie is nobody's lead: two seats level are both ahead of \
             the others and neither is ahead of the other.",
        ),
        (
            "last in front",
            "The last turn they led. For everybody but the winner this is the \
             turn the game got away from them.",
        ),
    ]));
    b.push_str("</thead><tbody>");
    for p in 0..seats {
        let front = led(p);
        let last = (0..turns).rev().find(|i| {
            let mine = rows[*i][p];
            mine > 0 && (0..seats).all(|q| q == p || rows[*i][q] < mine)
        });
        b.push_str(&row(
            &[
                placed(p, &who[p], place[p]),
                match reached(p, carranta_core::state::WINNING_VP / 2) {
                    Some(t) => t.to_string(),
                    None => NONE.to_string(),
                },
                match reached(p, near) {
                    Some(t) => t.to_string(),
                    None => NONE.to_string(),
                },
                if front == 0 {
                    NONE.to_string()
                } else {
                    format!(
                        "{front} <span class=\"worth\">({:.0}%)</span>",
                        100.0 * f64::from(front) / turns as f64
                    )
                },
                match last {
                    Some(i) => (i as u32 + 1).to_string(),
                    None => NONE.to_string(),
                },
            ],
            false,
        ));
    }
    b.push_str("</tbody>");
    // How long the endgame lasted: from the first seat coming within two points
    // to the win. A long last stretch is a table that saw it coming and could not
    // stop it; a short one is a game decided in a single turn.
    let first_near = (0..seats).filter_map(|p| reached(p, near)).min();
    let ties = (0..turns)
        .filter(|i| {
            let top = (0..seats).map(|q| rows[*i][q]).max().unwrap_or(0);
            top > 0 && (0..seats).filter(|q| rows[*i][*q] == top).count() > 1
        })
        .count() as u32;
    b.push_str(&totals(&[
        "the game".to_string(),
        NONE.to_string(),
        // How long the last stretch lasted, which is the figure the card exists
        // for: a long one is a table that saw it coming and could not stop it, a
        // short one is a game decided inside a single turn.
        match first_near {
            Some(t) => format!(
                "{t} <span class=\"worth\">({} turns left)</span>",
                turns as u32 - t
            ),
            None => NONE.to_string(),
        },
        if ties == 0 {
            NONE.to_string()
        } else {
            format!("<span class=\"worth\">{ties} level</span>")
        },
        turns.to_string(),
    ]));
    b.push_str(T_CLOSE);
    b
}

/// What each seat built, what it cost them, and what stopped them.
///
/// The ledger's built row is one number for four different decisions. A seat that
/// spent forty cards on roads and a seat that spent forty on cities were playing
/// different games, and until now the page could not tell them apart.
///
/// The last two columns are not spending. A road network's length is the thing
/// the longest road tile is contested on, and it is the only thing a seat builds
/// that no table on this page shows unless they win it. And a seat holding the
/// price of a settlement with nowhere legal to put it was not saving up, it was
/// stuck: a real way to lose a game and an invisible one, since nothing in a
/// result or a ledger leaves a mark when a player wanted to build and could not.
fn building(study: &Study, who: &[String], place: &[Option<usize>], seats: usize) -> String {
    let bt = &study.built;
    if seats == 0 || bt.turns == 0 {
        return String::new();
    }
    let mut b = String::from("<section>");
    b.push_str(&card_head(
        "Building",
        "Where each seat's cards went, and what stopped them spending more. The \
         ledger above says how many cards were spent on building; this says on \
         what. Prices are read off the hand rather than from the rules, so a road \
         from a Road Building card costs what it really cost, which is nothing.",
    ));
    b.push_str(T_OPEN);
    b.push_str("<thead>");
    b.push_str(&head_row(&[
        ("", ""),
        (
            "roads",
            "Roads bought, with the cards they cost in brackets. Free roads from a \
             Road Building card are counted and cost nothing, which is why the \
             brackets can fall short of two a road.",
        ),
        (
            "settlements",
            "Settlements bought. The opening's two are not here: they were placed \
             rather than paid for, and the opening card is where they belong. So a \
             blank means a seat that never built beyond its opening, which is a \
             strategy and not a gap.",
        ),
        (
            "cities",
            "Upgrades from a settlement to a city, and their cost.",
        ),
        (
            "cards",
            "Development cards bought, and what they cost. The same count as the \
             bought column on the cards table, arrived at from the spending side.",
        ),
        (
            "spent",
            "Every card spent on building, which is the ledger's built row \
             reached by another route. If the two ever disagree, one of them is \
             wrong.",
        ),
    ]));
    b.push_str("</thead><tbody>");
    let mut foot = [0u32; 4];
    let mut all_spent = 0u32;
    for p in 0..seats {
        let mut cells = vec![placed(p, &who[p], place[p])];
        for kind in 0..4 {
            foot[kind] += bt.pieces[p][kind];
            cells.push(bracketed(bt.pieces[p][kind], bt.spent[p][kind]));
        }
        all_spent += bt.spent_all(p);
        cells.push(bt.spent_all(p).to_string());
        b.push_str(&row(&cells, false));
    }
    b.push_str("</tbody>");
    let mut cells = vec!["the table".to_string()];
    for kind in 0..4 {
        cells.push(foot[kind].to_string());
    }
    cells.push(all_spent.to_string());
    b.push_str(&totals(&cells));
    b.push_str(T_CLOSE);
    b.push_str(&roads(study, who, place, seats));
    b.push_str(&walls(study, who, place, seats));
    b.push_str("</section>");
    b
}

/// What the roads actually did.
///
/// Roads are the one thing on the board with no score and no production, so a
/// count of them says nothing at all: two seats can build eight each and one of
/// them has opened four places to live while the other has built into a wall. A
/// road is worth the difference it made, so the difference is measured either
/// side of the move that built it.
fn roads(study: &Study, who: &[String], place: &[Option<usize>], seats: usize) -> String {
    let bt = &study.built;
    if bt.pieces[..seats].iter().map(|k| k[0]).sum::<u32>() == 0 {
        return String::new();
    }
    let mut b = String::from(T_OPEN);
    b.push_str("<thead>");
    b.push_str(&head_row(&[
        ("", ""),
        (
            "longest chain",
            "The longest continuous road this seat finished with (R-10.3). What \
             the road tile is contested on, and the one thing a seat builds that \
             nothing else here shows unless they won it. It can fall as well as \
             rise: a settlement built through the middle of a road cuts it in two.",
        ),
        (
            "opened a spot",
            "Roads that made at least one new intersection legal for this seat to \
             settle on, with the spots they opened in brackets. This is what a \
             road is for.",
        ),
        (
            "lengthened",
            "Roads that made the longest chain longer, which is what a road is \
             for if the tile is what you are after. A road can do both, so this \
             and the column beside it overlap.",
        ),
        (
            "neither",
            "Roads that opened nothing and lengthened nothing. Not necessarily \
             wasted, since a network can be grown towards a spot two roads away, \
             but a seat whose roads are mostly here was building without a plan \
             or building into a wall.",
        ),
    ]));
    b.push_str("</thead><tbody>");
    for p in 0..seats {
        b.push_str(&row(
            &[
                placed(p, &who[p], place[p]),
                if bt.chain[p] == 0 {
                    NONE.to_string()
                } else {
                    bt.chain[p].to_string()
                },
                bracketed(bt.opened[p], bt.spots[p]),
                if bt.stretched[p] == 0 {
                    NONE.to_string()
                } else {
                    bt.stretched[p].to_string()
                },
                if bt.idle[p] == 0 {
                    NONE.to_string()
                } else {
                    bt.idle[p].to_string()
                },
            ],
            false,
        ));
    }
    b.push_str("</tbody>");
    b.push_str(T_CLOSE);
    b
}

/// Turns a seat could pay for something and could not build it.
///
/// Three walls, and they are different walls. A settlement with nowhere legal
/// means the board is full or the roads have not reached; a city with nothing to
/// upgrade means every settlement is already a city or was never built; a road
/// with nowhere to go means the network is boxed in or the pieces have run out.
/// None of the three leaves a mark anywhere else on this page, and a seat sitting
/// behind one is holding cards it cannot spend while sevens go on being rolled.
fn walls(study: &Study, who: &[String], place: &[Option<usize>], seats: usize) -> String {
    let bt = &study.built;
    if bt.turns == 0
        || (0..seats)
            .flat_map(|p| bt.stuck[p].iter().copied())
            .sum::<u32>()
            == 0
    {
        return String::new();
    }
    let mut b = String::from(T_OPEN);
    b.push_str("<thead>");
    let mut heads = vec![(
        "",
        "Turns that ended with the seat holding the price of the thing in the \
         column and unable to build it, with the share of the game in brackets. \
         Three different walls: a settlement with nowhere legal to stand, a city \
         with no settlement of their own left to upgrade, a road with nowhere to \
         go or none left in the box. Able to pay is half of it, since a board with \
         nowhere to build costs nothing to a seat that could not have paid anyway.",
    )];
    // The columns are named for the thing that could not be built, and the row
    // header carries what the count means, so they need no repeated preamble.
    heads.extend(crate::analysis::Built::STUCK.iter().map(|kind| (*kind, "")));
    b.push_str(&head_row(&heads));
    b.push_str("</thead><tbody>");
    for p in 0..seats {
        let mut cells = vec![placed(p, &who[p], place[p])];
        for kind in 0..3 {
            let turns = bt.stuck[p][kind];
            cells.push(if turns == 0 {
                NONE.to_string()
            } else {
                format!(
                    "{turns} <span class=\"worth\">({:.0}%)</span>",
                    100.0 * f64::from(turns) / bt.turns as f64
                )
            });
        }
        b.push_str(&row(&cells, false));
    }
    b.push_str("</tbody>");
    b.push_str(T_CLOSE);
    b
}

/// What happened and when: a lane a seat, a mark a thing.
///
/// Every chart on this page provokes the same question and none of them can
/// answer it: a line steps up around turn ninety and nothing says why. This is
/// the answer, on the same turn axis as the chart above it, so the two read
/// together: settlements, cities, cards bought, and the two tiles arriving.
///
/// A strip rather than another line chart, because these are events and not
/// quantities. Nothing is being measured up the page, so nothing pretends to be.
fn timeline(study: &Study, who: &[String], seats: usize) -> String {
    const W: f64 = 720.0;
    const PAD: f64 = 36.0;
    const LANE: f64 = 26.0;
    const FOOT: f64 = 22.0;

    let turns = study.score.len();
    if study.events.is_empty() || turns < 2 || seats == 0 {
        return String::new();
    }
    let h = LANE * seats as f64 + FOOT + 10.0;
    let x = |turn: u32| {
        let i = (turn.max(1) - 1) as f64;
        PAD + (W - PAD * 2.0) * i / (turns - 1).max(1) as f64
    };
    let y = |seat: usize| 8.0 + LANE * seat as f64 + LANE / 2.0;

    let mut b = format!(
        "<div class=\"strip\"><div class=\"frame\"><svg viewBox=\"0 0 {W} {h:.0}\" \
         role=\"img\" aria-label=\"What each seat did, turn by turn\">"
    );
    // A rule a seat to hang the marks on, so an empty lane reads as a seat that
    // built nothing rather than as a missing row.
    for p in 0..seats {
        let _ = write!(
            b,
            "<line class=\"lane\" x1=\"{PAD}\" x2=\"{r}\" y1=\"{ly:.1}\" \
             y2=\"{ly:.1}\"/>",
            r = W - PAD,
            ly = y(p),
        );
    }
    let base = LANE * seats as f64 + 8.0;
    let step = nice_step(turns);
    let mut ticks: Vec<usize> = (0..turns).step_by(step).collect();
    if ticks.last() != Some(&(turns - 1)) {
        if ticks.last().is_some_and(|last| turns - 1 - last < step / 2) {
            ticks.pop();
        }
        ticks.push(turns - 1);
    }
    for i in ticks {
        let _ = write!(
            b,
            "<line class=\"tick\" x1=\"{tx:.1}\" x2=\"{tx:.1}\" y1=\"{base:.1}\" \
             y2=\"{end:.1}\"/><text class=\"axis mid\" x=\"{tx:.1}\" \
             y=\"{ly:.1}\">{n}</text>",
            tx = x(i as u32 + 1),
            end = base + 5.0,
            ly = base + 18.0,
            n = i + 1,
        );
    }

    // The marks, and a tooltip a mark, in the layer over the drawing.
    let mut tips = String::from("<div class=\"over\">");
    let mut shape = 0;
    for e in &study.events {
        if e.seat >= seats {
            continue;
        }
        let (class, said) = e.what.mark();
        // A tile rides just above the lane rather than on it. It is not a
        // building and it lands on the same turn as one often enough that on the
        // line the two marks sat on top of each other.
        let lift = if matches!(e.what, crate::analysis::Happened::Tile) {
            8.0
        } else {
            0.0
        };
        let (cx, cy) = (x(e.turn), y(e.seat) - lift);
        // Four shapes rather than four colours, since the colour is already
        // saying which seat: a filled square is a building, the bigger one a
        // city, a ring is a card bought, and a diamond is a tile changing hands.
        let side = size(e.what);
        let turned = matches!(e.what, crate::analysis::Happened::Tile);
        let _ = write!(
            b,
            "<rect class=\"beat {class} f{p} k{shape}\" x=\"{:.1}\" y=\"{:.1}\" \
             width=\"{side:.1}\" height=\"{side:.1}\" rx=\"{rx:.1}\"{spin}/>",
            cx - side / 2.0,
            cy - side / 2.0,
            p = e.seat,
            rx = match e.what {
                crate::analysis::Happened::Card => side / 2.0,
                // Sharp corners on the diamond, or it reads as a small square
                // that somebody nudged.
                crate::analysis::Happened::Tile => 0.0,
                _ => 1.0,
            },
            spin = if turned {
                format!(" transform=\"rotate(45 {cx:.1} {cy:.1})\"")
            } else {
                String::new()
            },
        );
        tips.push_str(&over_tip(
            shape,
            cx,
            cy,
            W,
            h,
            &format!("Turn {}: {} took {said}", e.turn, who[e.seat]),
        ));
        shape += 1;
    }
    b.push_str("</svg>");
    tips.push_str("</div>");
    b.push_str(&tips);
    b.push_str(&tip_rules(".strip", shape));
    b.push_str("</div>");

    // No names on the lanes. They needed an inset to fit, the inset pushed this
    // drawing's turn axis out of step with the chart above it, and being read
    // against that chart is the whole reason the strip exists. The legend between
    // the two already says which colour is whom.
    b.push_str(
        "<div class=\"key shapes\">\
         <span class=\"legend\"><span class=\"beat beat-house\"></span>settlement</span>\
         <span class=\"legend\"><span class=\"beat beat-city\"></span>city</span>\
         <span class=\"legend\"><span class=\"beat beat-card\"></span>card</span>\
         <span class=\"legend\"><span class=\"beat beat-tile\"></span>tile</span></div>",
    );
    b.push_str("</div>");
    b
}

/// How big a mark is, which is how much the thing it marks is worth.
fn size(what: crate::analysis::Happened) -> f64 {
    use crate::analysis::Happened;
    match what {
        Happened::Settlement => 7.0,
        Happened::City => 10.0,
        Happened::Card => 6.0,
        Happened::Tile => 8.0,
    }
}

/// Where the game's clock went, by the kind of decision it went on.
///
/// The card above says how long each seat took. This says what they were taking
/// it over, which the file has recorded per move since format 2 and the page has
/// never read. A table that spends a third of its game on the market is a
/// different table from one that spends it on the robber.
fn clock_split(study: &Study) -> String {
    let Some(spent) = &study.spent else {
        return String::new();
    };
    let total = spent.total();
    let moves: u32 = spent.by_kind.iter().map(|(_, n)| n).sum();
    if moves == 0 {
        return String::new();
    }
    // A table of bots decides a whole game inside a few milliseconds, which is
    // finer than the clock records. The counts still say what the game was made
    // of; the time columns go blank rather than printing a column of noughts and
    // inviting them to be read as findings.
    const LEGIBLE: u32 = 1_000;
    let timed = total >= LEGIBLE;

    let mut b = String::from(T_OPEN);
    b.push_str("<thead>");
    b.push_str(&head_row(&[
        ("", ""),
        (
            "decisions",
            "Moves of this kind, counting every seat's. A refusal counts as a \
             trading decision, because that is what it is: an offer being turned \
             down.",
        ),
        (
            "share",
            "How many of the game's decisions were of this kind. Not the same \
             question as the time beside it: a kind can be most of the moves and \
             almost none of the thinking.",
        ),
        (
            "time",
            "Wall clock from the previous move to this one, charged to the move \
             that ends the wait rather than the one before it: the gap is \
             somebody deciding what to do next, and what they decided is the move \
             that lands. Blank for a game decided faster than the clock records, \
             which is every game the bots play against each other.",
        ),
        (
            "each",
            "Time a decision of this kind took, on average. The interesting \
             column when there is one: a kind can be a third of the clock \
             because it is slow or because there are hundreds of it, and only \
             this tells the two apart.",
        ),
    ]));
    b.push_str("</thead><tbody>");
    for (kind, (ms, n)) in crate::analysis::KINDS.iter().zip(&spent.by_kind) {
        if *n == 0 {
            continue;
        }
        b.push_str(&row(
            &[
                (*kind).to_string(),
                n.to_string(),
                format!("{:.0}%", 100.0 * f64::from(*n) / f64::from(moves)),
                if timed { clock(*ms) } else { NONE.to_string() },
                if timed {
                    clock(ms / (*n).max(1))
                } else {
                    NONE.to_string()
                },
            ],
            false,
        ));
    }
    b.push_str("</tbody>");
    b.push_str(&totals(&[
        "the game".to_string(),
        moves.to_string(),
        NONE.to_string(),
        if timed {
            clock(total)
        } else {
            NONE.to_string()
        },
        if timed {
            clock(total / moves.max(1))
        } else {
            NONE.to_string()
        },
    ]));
    b.push_str(T_CLOSE);
    b
}

/// A number of cards gained or lost, written with its sign and blank at nought.
///
/// Cards are whole things, so no decimal: the rating card's two places are for a
/// quantity that genuinely has them.
fn cards(v: f64) -> String {
    if v.abs() < 0.5 {
        return NONE.to_string();
    }
    format!(
        "<span class=\"{}\">{v:+.0}</span>",
        if v > 0.0 { "up" } else { "down" }
    )
}

/// What was in the offers, rather than how many there were.
///
/// Three counts, offered and withdrawn and turned down, cannot tell a seat
/// nobody would deal with from a seat asking two cards for one. Those are
/// different problems with different answers and they wear the same counts, so
/// the ask itself is what this says.
fn offers(study: &Study, who: &[String], place: &[Option<usize>], seats: usize) -> String {
    let asks = &study.asks;
    if seats == 0 || asks.offers[..seats].iter().sum::<u32>() == 0 {
        return String::new();
    }
    let mut b = String::from(T_OPEN);
    b.push_str("<thead>");
    b.push_str(&head_row(&[
        ("", ""),
        (
            "wanted",
            "Cards asked for across every offer this seat put on the table, and \
             what they asked for on average in brackets.",
        ),
        (
            "put up",
            "Cards they offered in exchange, the same way. Offers, not trades: \
             most of these were never taken.",
        ),
        (
            "the ask",
            "Cards wanted for each card put up. One is an even swap. Above one is \
             a seat asking to come out ahead, which is anybody's right and also \
             the likeliest reason nobody took it.",
        ),
        (
            "taken up",
            "Offers of theirs that somebody accepted. Nought against a hundred \
             offers is the table's answer to the column beside it.",
        ),
    ]));
    b.push_str("</thead><tbody>");
    for p in 0..seats {
        let n = asks.offers[p];
        b.push_str(&row(
            &[
                placed(p, &who[p], place[p]),
                mean_of(asks.wanted[p], n),
                mean_of(asks.given[p], n),
                match asks.ask(p) {
                    Some(r) => format!("{r:.2}"),
                    None => NONE.to_string(),
                },
                if asks.taken[p] == 0 {
                    NONE.to_string()
                } else {
                    asks.taken[p].to_string()
                },
            ],
            false,
        ));
    }
    b.push_str("</tbody>");
    let sum = |v: &[u32; MAX_PLAYERS]| v[..seats].iter().sum::<u32>();
    let (wanted, given, made) = (sum(&asks.wanted), sum(&asks.given), sum(&asks.offers));
    b.push_str(&totals(&[
        "the table".to_string(),
        mean_of(wanted, made),
        mean_of(given, made),
        if given == 0 {
            NONE.to_string()
        } else {
            format!("{:.2}", f64::from(wanted) / f64::from(given))
        },
        if sum(&asks.taken) == 0 {
            NONE.to_string()
        } else {
            sum(&asks.taken).to_string()
        },
    ]));
    b.push_str(T_CLOSE);
    b
}

/// What kind of board this was, beyond how the pips were shared out.
///
/// The tables above answer "which resource did the deal favour". They cannot
/// answer "what sort of board is this", and two boards can owe every resource the
/// same pips and play completely differently: one with its ore spread around the
/// island and one with all of it in a corner are not the same game.
///
/// The clumping expectation is exact rather than simulated, which the shape of the
/// problem allows. The adjacency graph is fixed, so the neighbouring pairs can be
/// counted; and for a shuffled set of tiles the chance any given pair matches is
/// the chance two tiles drawn without replacement are the same terrain.
fn deal(study: &Study) -> String {
    let bd = &study.board;
    if bd.best == 0 {
        return String::new();
    }
    let mut b = String::from(T_OPEN);
    b.push_str("<thead>");
    b.push_str(&head_row(&[
        ("", ""),
        ("this board", "What was dealt."),
        (
            "an average board",
            "What a random deal of the same tiles gives, where that is a figure \
             rather than a coin toss. Blank where the question has no average: \
             the best intersection on a board is a maximum, and a maximum has no \
             closed form worth printing.",
        ),
    ]));
    b.push_str("</thead><tbody>");
    b.push_str(&row(
        &[
            format!(
                "<span data-tip=\"{}\">neighbours making the same thing</span>",
                esc(
                    "Pairs of touching hexes that produce the same resource.                      Clumping is what a pip total cannot see, and it decides                      whether a resource can be shut off by one robber or has to                      be chased around the island."
                )
            ),
            bd.same.to_string(),
            format!("{:.1}", bd.same_expected),
        ],
        false,
    ));
    b.push_str(&row(
        &[
            format!(
                "<span data-tip=\"{}\">a six beside an eight</span>",
                esc(
                    "Whether the two most likely numbers were dealt next to each                      other. Some rule sets forbid it and redeal; this one deals                      as it deals, so it is worth knowing which happened."
                )
            ),
            if bd.reds_touch {
                "yes".to_string()
            } else {
                "no".to_string()
            },
            NONE.to_string(),
        ],
        false,
    ));
    b.push_str(&row(
        &[
            format!(
                "<span data-tip=\"{}\">the best intersection</span>",
                esc(
                    "Pips on the hexes around the best spot on the board, with                      its numbers beside it. Whether anybody took it is the                      opening card's business."
                )
            ),
            format!(
                "{} <span class=\"worth\">{}</span>",
                bd.best,
                discs(&bd.best_numbers)
            ),
            NONE.to_string(),
        ],
        false,
    ));
    b.push_str(&row(
        &[
            format!(
                "<span data-tip=\"{}\">intersections over {} pips</span>",
                esc(
                    "How many places on the board were worth planning a game                      around: three hexes averaging better than three pips each.                      A board with two of them is a fight; a board with ten is a                      different game."
                ),
                crate::analysis::Board::RICH
            ),
            bd.rich.to_string(),
            NONE.to_string(),
        ],
        false,
    ));
    b.push_str(&row(
        &[
            format!(
                "<span data-tip=\"{}\">the ordinary intersection</span>",
                esc(
                    "Mean pips over every intersection touching land, which is                      what the best one has to be read against."
                )
            ),
            format!("{:.1}", bd.spot_mean),
            NONE.to_string(),
        ],
        false,
    ));
    b.push_str("</tbody>");
    b.push_str(T_CLOSE);
    b
}

/// Which cards a seat chose to throw away.
///
/// The rule takes half a hand on a seven (R-6.2) and the player picks which half,
/// so a discard is a decision and the ledger's single total cannot show it. What a
/// seat threw away is what it had decided it did not need, and read against the
/// production table it is often the resource the board was giving it most of.
fn thrown(study: &Study, who: &[String], place: &[Option<usize>], seats: usize) -> String {
    let h = &study.hands;
    if seats == 0
        || (0..seats)
            .flat_map(|p| h.thrown[p].iter().copied())
            .sum::<u32>()
            == 0
    {
        return String::new();
    }
    let mut b = String::from(T_OPEN);
    b.push_str("<thead>");
    let mut heads = vec![(
        "",
        "Cards thrown away to a seven, resource by resource. The rule takes half \
         the hand and the player chooses which half, so this is a decision rather \
         than a toll.",
    )];
    heads.extend(RESOURCE_NAMES.iter().map(|n| (*n, "")));
    heads.push((
        "all of it",
        "Every card this seat discarded, which is the ledger's discarded row \
         above reached from the other side.",
    ));
    b.push_str(&head_row(&heads));
    b.push_str("</thead><tbody>");
    let mut foot = [0u32; 5];
    for p in 0..seats {
        let mut cells = vec![placed(p, &who[p], place[p])];
        for res in 0..5 {
            foot[res] += h.thrown[p][res];
            cells.push(if h.thrown[p][res] == 0 {
                NONE.to_string()
            } else {
                h.thrown[p][res].to_string()
            });
        }
        let all: u32 = h.thrown[p].iter().sum();
        cells.push(if all == 0 {
            NONE.to_string()
        } else {
            all.to_string()
        });
        b.push_str(&row(&cells, false));
    }
    b.push_str("</tbody>");
    let mut cells = vec!["the table".to_string()];
    cells.extend(foot.iter().map(|n| n.to_string()));
    cells.push(foot.iter().sum::<u32>().to_string());
    b.push_str(&totals(&cells));
    b.push_str(T_CLOSE);
    b
}

/// How long each kind of development card sat in a hand before it was played.
///
/// The table above says how many were bought and how many were played. It cannot
/// say when, and a militia played on the turn it was drawn is a different decision
/// from one held for forty turns: the first is a seven happening to somebody, the
/// second is a player waiting until the robber was worth moving.
///
/// A card is matched to the oldest unplayed card of its kind, since cards of a kind
/// are interchangeable and any other rule would be arbitrary in the same way while
/// reading worse.
fn waiting(study: &Study) -> String {
    let w = &study.waits;
    let any = (0..5).any(|k| !w.held[k].is_empty() || !w.kept[k].is_empty());
    if !any {
        return String::new();
    }
    let mut b = String::from(T_OPEN);
    b.push_str("<thead>");
    b.push_str(&head_row(&[
        ("", ""),
        (
            "played",
            "Cards of this kind that were played, across every seat. The game's \
             count, not anybody's.",
        ),
        (
            "waited",
            "Turns between drawing a card and playing it, on average. Table turns, \
             not the holder's own: in a four-player game their next turn is four \
             of these, so four is a card played at the first opportunity, which \
             is also the earliest the rules allow (R-9.4).",
        ),
        (
            "longest wait",
            "The longest any one of them sat in a hand before it was played.",
        ),
        (
            "still held",
            "Cards of this kind in a hand when the game ended, with how long they \
             had been there on average. A card held to the end is a decision too, \
             and a mean over played cards alone would quietly leave it out. A \
             victory point card is never played at all (R-9.11), so all of them \
             are here.",
        ),
    ]));
    b.push_str("</thead><tbody>");
    for kind in 0..5 {
        let played = w.held[kind].len() as u32;
        let kept = &w.kept[kind];
        if played == 0 && kept.is_empty() {
            continue;
        }
        b.push_str(&row(
            &[
                format!("<span class=\"dot dev\"></span>{}", DEV_NAMES[kind]),
                if played == 0 {
                    NONE.to_string()
                } else {
                    played.to_string()
                },
                match w.mean(kind) {
                    Some(t) => format!("{t:.0}"),
                    None => NONE.to_string(),
                },
                match w.longest(kind) {
                    Some(t) => t.to_string(),
                    None => NONE.to_string(),
                },
                if kept.is_empty() {
                    NONE.to_string()
                } else {
                    format!(
                        "{} <span class=\"worth\">({:.0})</span>",
                        kept.len(),
                        kept.iter().map(|t| f64::from(*t)).sum::<f64>() / kept.len() as f64
                    )
                },
            ],
            false,
        ));
    }
    b.push_str("</tbody>");
    b.push_str(T_CLOSE);
    b
}

/// What the offers were asking *for*, resource by resource.
///
/// The counterparty question cannot be asked of these games: the offer generator
/// only ever makes open offers, on purpose, since addressing one multiplies the
/// action space by the number of opponents for nothing. So "who did they aim it
/// at" is nought for every seat, and the useful question about an offer is what
/// was in it.
///
/// Positive is a seat trying to buy that card, negative a seat trying to sell it.
/// Read beside the production card it says something neither says alone: whether a
/// seat went looking for what the board was failing to pay it, or for something
/// else entirely.
fn demand(study: &Study, who: &[String], place: &[Option<usize>], seats: usize) -> String {
    let asks = &study.asks;
    if seats == 0 || asks.offers[..seats].iter().sum::<u32>() == 0 {
        return String::new();
    }
    let mut b = String::from(T_OPEN);
    b.push_str("<thead>");
    let mut heads = vec![(
        "",
        "Cards asked for less cards put up, resource by resource, across every \
         offer this seat made. Positive is a seat trying to buy that card, \
         negative a seat trying to sell it. Offers, not trades, so this is what \
         they went looking for rather than what they got.",
    )];
    heads.extend(RESOURCE_NAMES.iter().map(|n| (*n, "")));
    b.push_str(&head_row(&heads));
    b.push_str("</thead><tbody>");
    for p in 0..seats {
        let mut cells = vec![placed(p, &who[p], place[p])];
        for res in 0..5 {
            let net = f64::from(asks.wanted_each[p][res]) - f64::from(asks.given_each[p][res]);
            cells.push(cards(net));
        }
        b.push_str(&row(&cells, false));
    }
    b.push_str("</tbody>");
    // Every seat wanting the same card is a fact about the board rather than
    // about any of them, which is what the foot is for.
    let mut foot = vec!["the table".to_string()];
    for res in 0..5 {
        let net: f64 = (0..seats)
            .map(|p| f64::from(asks.wanted_each[p][res]) - f64::from(asks.given_each[p][res]))
            .sum();
        foot.push(cards(net));
    }
    b.push_str(&totals(&foot));
    b.push_str(T_CLOSE);
    b
}

/// A count, with what it averages out to in brackets.
fn mean_of(total: u32, over: u32) -> String {
    if over == 0 {
        return NONE.to_string();
    }
    format!(
        "{total} <span class=\"worth\">({:.1})</span>",
        f64::from(total) / f64::from(over)
    )
}

/// What the trading was worth, rather than how much of it there was.
///
/// The counts above say a seat traded eleven times. They cannot say whether it
/// came out of those eleven ahead, what it paid, or which seat it spent the game
/// feeding. Cards are the unit here, and every figure is derived from the list
/// of deals the trades card already keeps, read from both sides: a deal is
/// recorded once, from the offering seat, and counting only that side would make
/// every counterparty look as though it had never traded.
fn flows(study: &Study, who: &[String], place: &[Option<usize>], seats: usize) -> String {
    let tr = &study.trades;
    if tr.deals.is_empty() || seats == 0 {
        return String::new();
    }
    // Who to measure "fed" against. The winner, if there was one, since that is
    // the question worth asking; otherwise nobody, and the column goes.
    let winner = study
        .report
        .winner
        .map(|w| w as usize)
        .filter(|w| *w < seats);

    let mut b = String::from(T_OPEN);
    b.push_str("<thead>");
    let fed = winner.map(|w| format!("to {}", esc(&who[w])));
    let mut heads = vec![
        ("", ""),
        (
            "gave",
            "Cards this seat handed over, across every trade it was a party to, \
             with a person or with the supply.",
        ),
        ("took", "Cards it took back."),
        (
            "net",
            "The difference. Trading with the supply always loses cards, since \
             the supply charges two, three or four for one, so a seat that spent \
             the game at the bank is deep in the red here and may still have \
             played well: what it bought with them is the ledger's business.",
        ),
        (
            "price",
            "Cards handed over for each card taken back. One is an even swap \
             with a person; four is the bank's own rate, and anything between is \
             a mixture of the two.",
        ),
    ];
    if let Some(f) = &fed {
        heads.push((
            f.as_str(),
            "Cards handed to the seat that won, less cards taken back from them. \
             Positive is a seat that fed the winner. A table can lose to the \
             player it kept trading with, and this is the column that says so.",
        ));
    }
    b.push_str(&head_row(&heads));
    b.push_str("</thead><tbody>");
    let (mut all_gave, mut all_took) = (0u32, 0u32);
    for p in 0..seats {
        let (gave, took) = tr.cards(p);
        all_gave += gave;
        all_took += took;
        let mut cells = vec![
            placed(p, &who[p], place[p]),
            gave.to_string(),
            took.to_string(),
            cards(f64::from(took) - f64::from(gave)),
            if took == 0 {
                NONE.to_string()
            } else {
                format!("{:.1}", f64::from(gave) / f64::from(took))
            },
        ];
        if let Some(w) = winner {
            cells.push(if p == w {
                NONE.to_string()
            } else {
                let (gave, took) = tr.cards_between(p, w);
                cards(f64::from(gave) - f64::from(took))
            });
        }
        b.push_str(&row(&cells, false));
    }
    b.push_str("</tbody>");
    // The two columns do not have to match, and where they do not is the
    // interesting part: a trade between two people moves cards sideways, and a
    // trade with the supply takes them out of the game. The gap is what the
    // table paid the bank and the ports for the privilege.
    let mut foot = vec![
        "the table".to_string(),
        all_gave.to_string(),
        all_took.to_string(),
        cards(f64::from(all_took) - f64::from(all_gave)),
        NONE.to_string(),
    ];
    if winner.is_some() {
        foot.push(NONE.to_string());
    }
    b.push_str(&totals(&foot));
    b.push_str(T_CLOSE);
    b
}

/// The robber as a blockade rather than as a thief.
///
/// The sankey above says who took cards from whom, which is the robber's other
/// job and the noisier one. This is the quiet one: a robber parked on the wheat
/// 8 for thirty turns decides a game without stealing a single card, and until
/// now the page had no way to say so. Turns blockaded is the exposure; cards
/// denied is what it cost, and comes from the decomposition card, so the two
/// tables cannot disagree about the same robber.
fn blockade(study: &Study, who: &[String], place: &[Option<usize>], seats: usize) -> String {
    let rb = &study.robber;
    if rb.turns == 0 || seats == 0 {
        return String::new();
    }
    let mut b = String::from(T_OPEN);
    b.push_str("<thead>");
    b.push_str(&head_row(&[
        ("", ""),
        (
            "blockaded",
            "Turns that ended with the robber sitting on a hex this seat had \
             built on. Nothing to do with being robbed: this is the hex not \
             paying, every roll, for as long as the piece sits there.",
        ),
        (
            "share",
            "How much of the game that was. A third here is a third of the game \
             played a hex short.",
        ),
        (
            "cards denied",
            "What those turns cost, in cards, from the deviation card above. \
             This is the robber column there, and the same figure by \
             construction.",
        ),
    ]));
    b.push_str("</thead><tbody>");
    for p in 0..seats {
        let turns = rb.blocked[p];
        b.push_str(&row(
            &[
                placed(p, &who[p], place[p]),
                if turns == 0 {
                    NONE.to_string()
                } else {
                    turns.to_string()
                },
                if turns == 0 {
                    NONE.to_string()
                } else {
                    format!("{:.0}%", 100.0 * f64::from(turns) / rb.turns as f64)
                },
                cost(study.production.decompose(p).robber_cost),
            ],
            false,
        ));
    }
    b.push_str("</tbody>");
    b.push_str(T_CLOSE);

    // And where it actually sat, which is the sentence a player wants: the
    // number, not the hex, because "the wheat 8" is a thing somebody remembers.
    let spots: Vec<_> = rb.spots.iter().take(3).collect();
    if spots.is_empty() {
        return b;
    }
    b.push_str(T_OPEN);
    b.push_str("<thead>");
    b.push_str(&head_row(&[
        ("", ""),
        (
            "turns",
            "Turns the piece spent on this hex. One robber, so these count the \
             same turns the table above splits between the seats it sat on.",
        ),
        ("share", "How much of the game it spent there."),
    ]));
    b.push_str("</thead><tbody>");
    for spot in spots {
        let name = match spot.resource {
            Some(r) => format!(
                "<span class=\"dot r{r}\"></span>{} {}",
                RESOURCE_NAMES[r],
                disc(spot.number)
            ),
            None => "the desert".to_string(),
        };
        b.push_str(&row(
            &[
                name,
                spot.turns.to_string(),
                format!("{:.0}%", 100.0 * f64::from(spot.turns) / rb.turns as f64),
            ],
            false,
        ));
    }
    b.push_str("</tbody>");
    b.push_str(T_CLOSE);
    b
}

/// One number, drawn as the board draws it.
fn disc(n: u8) -> String {
    if !(2..=12).contains(&n) {
        return String::new();
    }
    format!(
        "<span class=\"disc{}\" data-tip=\"{n} comes up on {ways} rolls in 36\">{n}</span>",
        if n == 6 || n == 8 { " hot" } else { "" },
        ways = 6 - (i32::from(n) - 7).abs(),
    )
}

/// The score, turn by turn, under the table that reports the end of it.
///
/// A result table says who won and by how much; it cannot say whether the game
/// was ever close. A seat that led for a hundred turns and lost, and a seat
/// that was fourth until the last ten, come out of that table looking the same.
///
/// The true score, hidden victory point cards included, which is what the table
/// above reports: the last point of every line is that seat's points column.
fn score_plot(study: &Study, who: &[String], place: &[Option<usize>], seats: usize) -> String {
    const W: f64 = 720.0;
    const H: f64 = 210.0;
    const PAD: f64 = 36.0;
    const FOOT: f64 = 26.0;

    let rows = &study.score;
    let turns = rows.len();
    if turns == 0 || seats == 0 {
        return String::new();
    }
    // The winning score is the ceiling: the game stopped there, so there is
    // nothing above it to draw and a taller axis would be empty paper.
    let top = f64::from(
        rows.iter()
            .flat_map(|r| r[..seats].iter().copied())
            .max()
            .unwrap_or(1)
            .max(1),
    );
    let x = |i: usize| PAD + (W - PAD * 2.0) * i as f64 / (turns - 1).max(1) as f64;
    let y = |v: f64| H - FOOT - PAD - (H - FOOT - PAD * 2.0) * v / top;
    let base = H - FOOT - PAD;

    let mut b = format!(
        "<div class=\"view\"><div class=\"frame\"><svg viewBox=\"0 0 {W} {H}\" \
         role=\"img\" aria-label=\"The score turn by turn\">"
    );
    // Gridlines on points somebody counts in, which for a game to ten is
    // every second point.
    let stride = if top > 14.0 { 4.0 } else { 2.0 };
    let mut v = stride;
    while v <= top + 1e-9 {
        let _ = write!(
            b,
            "<line class=\"grid\" x1=\"{PAD}\" x2=\"{r}\" y1=\"{gy}\" y2=\"{gy}\"/>\
             <text class=\"axis\" x=\"{tx}\" y=\"{ty}\">{v:.0}</text>",
            r = W - PAD,
            gy = y(v),
            tx = PAD - 6.0,
            ty = y(v) + 4.0,
        );
        v += stride;
    }
    let step = nice_step(turns);
    let mut ticks: Vec<usize> = (0..turns).step_by(step).collect();
    if ticks.last() != Some(&(turns - 1)) {
        if ticks.last().is_some_and(|last| turns - 1 - last < step / 2) {
            ticks.pop();
        }
        ticks.push(turns - 1);
    }
    for i in ticks {
        let _ = write!(
            b,
            "<line class=\"tick\" x1=\"{tx:.1}\" x2=\"{tx:.1}\" y1=\"{base}\" y2=\"{end}\"/>\
             <text class=\"axis mid\" x=\"{tx:.1}\" y=\"{ly}\">{n}</text>",
            tx = x(i),
            end = base + 5.0,
            ly = base + 18.0,
            n = i + 1,
        );
    }
    let _ = write!(
        b,
        "<text class=\"axis start unit\" x=\"{PAD}\" y=\"14\">points</text>"
    );
    // Where the game ends, drawn, because "was it close" is read against the
    // finish and not against the top of the paper.
    let win = f64::from(carranta_core::state::WINNING_VP);
    if win <= top {
        let _ = write!(
            b,
            "<line class=\"finish\" x1=\"{PAD}\" x2=\"{r}\" y1=\"{fy:.1}\" \
             y2=\"{fy:.1}\"/><text class=\"axis mid finish-mark\" x=\"{tx:.1}\" \
             y=\"{ty:.1}\">{win:.0} to win</text>",
            r = W - PAD,
            fy = y(win),
            tx = W - PAD - 30.0,
            ty = y(win) - 5.0,
        );
    }
    // A score holds until something changes it, so the line is a stair rather
    // than a slope: drawing it as a slope would show points arriving through a
    // turn nobody scored in.
    //
    // Two lines a seat: the true score solid, and the score the rest of the
    // table could see dotted under it. They part the moment a victory point card
    // is drawn and never rejoin, and the gap is what nobody else knew about.
    for p in 0..seats {
        for (values, dash) in [(rows, ""), (&study.seen, " owed")] {
            let mut path = String::new();
            for i in 0..turns {
                if i > 0 {
                    let _ = write!(path, "{:.1},{:.1} ", x(i), y(f64::from(values[i - 1][p])));
                }
                let _ = write!(path, "{:.1},{:.1} ", x(i), y(f64::from(values[i][p])));
            }
            let _ = write!(
                b,
                "<polyline class=\"line k{n} f{p}{dash}\" points=\"{}\"/>",
                path.trim(),
                n = p + 1,
            );
        }
    }
    b.push_str("</svg>");

    let slot = (W - PAD * 2.0) / (turns - 1).max(1) as f64;
    b.push_str("<div class=\"over\">");
    for i in 0..turns {
        let mut said = format!("Turn {}", i + 1);
        for p in 0..seats {
            let (all, open) = (rows[i][p], study.seen[i][p]);
            let _ = write!(said, "\n{}: {all}", who[p]);
            if all != open {
                let _ = write!(said, ", {open} in the open");
            }
        }
        let _ = write!(
            b,
            "<div class=\"slot{}\" style=\"left:{lx:.3}%;width:{lw:.3}%;top:{lt:.2}%;\
             height:{lh:.2}%\" data-tip=\"{}\"></div>",
            if x(i) > W / 2.0 { " to-left" } else { "" },
            esc(&said),
            lx = 100.0 * (x(i) - slot / 2.0) / W,
            lw = 100.0 * slot / W,
            lt = 100.0 * PAD / H,
            lh = 100.0 * (base - PAD) / H,
        );
    }
    b.push_str("</div></div>");
    b.push_str(&key(who, place, seats, "sc"));
    b.push_str("</div>");
    b
}

/// What each seat's engine was worth a roll, turn by turn, and how fast it grew.
///
/// The premise is an assumption worth stating: an economy that compounds beats
/// one that is merely large. Cards a turn buy buildings, buildings buy more
/// cards a turn, and a seat whose rate keeps climbing ends the game with an
/// engine nobody can catch. So an economy is rated on its slope rather than its
/// size, and the slope is fitted in logs, which is what makes it a rate.
///
/// The engine, not the earnings: what one roll was worth given the buildings
/// standing at the time. The cards that actually arrived are this plus the dice.
fn engine_card(study: &Study, who: &[String], place: &[Option<usize>], seats: usize) -> String {
    const W: f64 = 720.0;
    const H: f64 = 220.0;
    const PAD: f64 = 36.0;
    const FOOT: f64 = 26.0;

    let rows = &study.engine;
    let turns = rows.len();
    if turns < 2 || seats == 0 {
        return String::new();
    }
    let top = rows
        .iter()
        .flat_map(|r| r[..seats].iter().copied())
        .fold(0.1f64, f64::max);
    let x = |i: usize| PAD + (W - PAD * 2.0) * i as f64 / (turns - 1).max(1) as f64;
    let y = |v: f64| H - FOOT - PAD - (H - FOOT - PAD * 2.0) * v / top;
    let base = H - FOOT - PAD;

    let mut b = String::from("<section>");
    b.push_str(&card_head(
        "Engine",
        "What one roll was worth to each seat, in cards, at the end of every \
         turn: the buildings they had standing, not what the dice paid them. \
         Rated on how fast it grew rather than on how big it got, on the \
         assumption that an economy which compounds beats one that is merely \
         large.",
    ));
    let _ = write!(
        b,
        "<div class=\"view\"><div class=\"frame\"><svg viewBox=\"0 0 {W} {H}\" \
         role=\"img\" aria-label=\"What a roll was worth to each seat\">"
    );
    for k in 1..=4 {
        let v = top * f64::from(k) / 4.0;
        let _ = write!(
            b,
            "<line class=\"grid\" x1=\"{PAD}\" x2=\"{r}\" y1=\"{gy}\" y2=\"{gy}\"/>\
             <text class=\"axis\" x=\"{tx}\" y=\"{ty}\">{v:.1}</text>",
            r = W - PAD,
            gy = y(v),
            tx = PAD - 6.0,
            ty = y(v) + 4.0,
        );
    }
    let step = nice_step(turns);
    let mut ticks: Vec<usize> = (0..turns).step_by(step).collect();
    if ticks.last() != Some(&(turns - 1)) {
        if ticks.last().is_some_and(|last| turns - 1 - last < step / 2) {
            ticks.pop();
        }
        ticks.push(turns - 1);
    }
    for i in ticks {
        let _ = write!(
            b,
            "<line class=\"tick\" x1=\"{tx:.1}\" x2=\"{tx:.1}\" y1=\"{base}\" y2=\"{end}\"/>\
             <text class=\"axis mid\" x=\"{tx:.1}\" y=\"{ly}\">{n}</text>",
            tx = x(i),
            end = base + 5.0,
            ly = base + 18.0,
            n = i + 1,
        );
    }
    let _ = write!(
        b,
        "<text class=\"axis start unit\" x=\"{PAD}\" y=\"14\">a roll</text>"
    );
    // A stair again, since an engine changes when a building goes up and not
    // between times.
    for p in 0..seats {
        let mut path = String::new();
        for i in 0..turns {
            if i > 0 {
                let _ = write!(path, "{:.1},{:.1} ", x(i), y(rows[i - 1][p]));
            }
            let _ = write!(path, "{:.1},{:.1} ", x(i), y(rows[i][p]));
        }
        let _ = write!(
            b,
            "<polyline class=\"line k{n} f{p}\" points=\"{}\"/>",
            path.trim(),
            n = p + 1,
        );
    }
    b.push_str("</svg>");
    let slot = (W - PAD * 2.0) / (turns - 1).max(1) as f64;
    b.push_str("<div class=\"over\">");
    for i in 0..turns {
        let mut said = format!("Turn {}", i + 1);
        for p in 0..seats {
            let _ = write!(said, "\n{}: {:.2} a roll", who[p], rows[i][p]);
        }
        let _ = write!(
            b,
            "<div class=\"slot{}\" style=\"left:{lx:.3}%;width:{lw:.3}%;top:{lt:.2}%;\
             height:{lh:.2}%\" data-tip=\"{}\"></div>",
            if x(i) > W / 2.0 { " to-left" } else { "" },
            esc(&said),
            lx = 100.0 * (x(i) - slot / 2.0) / W,
            lw = 100.0 * slot / W,
            lt = 100.0 * PAD / H,
            lh = 100.0 * (base - PAD) / H,
        );
    }
    b.push_str("</div></div>");
    b.push_str(&key(who, place, seats, "en"));
    b.push_str("</div>");

    // The rating itself.
    b.push_str(T_OPEN);
    b.push_str("<thead>");
    b.push_str(&head_row(&[
        ("", ""),
        (
            "opening",
            "Cards a roll across the first quarter of the game, which is close \
             to what the two settlements bought.",
        ),
        (
            "end",
            "And across the last quarter, which is the engine the game finished \
             with.",
        ),
        (
            "multiple",
            "How many times over the engine grew between those two. A seat that \
             trebled it built three times the economy they started with.",
        ),
        (
            "growth",
            "What the engine multiplied by each turn, fitted as a straight line \
             through the log of its size. Two percent a turn compounds to a \
             doubling in thirty-five. Greyed where the shape column says the \
             figure is not describing anything.",
        ),
        (
            "doubling",
            "Turns to double at that rate, if it held. Blank for an engine that \
             never grew, and blank when the answer runs past the length of the \
             game: a doubling five games away is not a fact about this one.",
        ),
        (
            "shape",
            "Which account of the engine the numbers actually support. \
             Compounding is a rate multiplying, which is the thing worth having. \
             Steady is climbing by the same amount every turn, which a growth \
             percentage flatters. Flat is neither. Both accounts are fitted and \
             the wider fit wins, because over the range one game covers the log \
             of a straight ramp is very nearly straight too, and a good log fit \
             on its own cannot tell the two apart.",
        ),
        (
            "fit",
            "How straight the log line was, from nought to one, and the honest \
             half of the growth figure. Compounding here is bounded at both \
             ends: the opening is a standing start, the pieces run out, and the \
             game stops at ten points.",
        ),
    ]));
    b.push_str("</thead><tbody>");
    for p in 0..seats {
        let cells = match crate::analysis::growth_of(rows, p) {
            None => vec![placed(p, &who[p], place[p])],
            Some(g) => vec![
                placed(p, &who[p], place[p]),
                format!("{:.2}", g.early),
                format!("{:.2}", g.late),
                if g.early > 0.005 {
                    format!("{:.1}x", g.late / g.early)
                } else {
                    NONE.to_string()
                },
                format!(
                    "<span class=\"{}\">{:+.1}%</span>",
                    // Greyed rather than hidden when the shape does not support
                    // it: the figure is still what was fitted, and reading it as
                    // a rate would be the mistake.
                    match (g.believable(), g.per_turn > 0.0) {
                        (false, _) => "worth",
                        (true, true) => "up",
                        (true, false) => "down",
                    },
                    g.per_turn * 100.0
                ),
                match g.doubling {
                    // A doubling time past the end of the game is arithmetic
                    // rather than a finding: at a tenth of a percent a turn the
                    // answer is six hundred turns, and the game lasted a
                    // hundred and fifty.
                    Some(t) if t <= g.turns as f64 => format!("{t:.0}"),
                    _ => NONE.to_string(),
                },
                match g.shape() {
                    crate::analysis::Shape::Compounding => {
                        "<span class=\"up\">compounding</span>".to_string()
                    }
                    crate::analysis::Shape::Steady => "steady".to_string(),
                    crate::analysis::Shape::Flat => "<span class=\"worth\">flat</span>".to_string(),
                },
                format!("{:.2}", g.fit),
            ],
        };
        b.push_str(&row(&cells, false));
    }
    b.push_str("</tbody>");
    b.push_str(T_CLOSE);
    b.push_str("</section>");
    b
}

/// A chart's legend, which is also its control: a checkbox a seat, so clicking a
/// name takes that seat's lines off the chart.
///
/// The same markup the production card uses, so the same positional rules switch
/// the lines off. An `id` only has to be unique on the page; the rules count
/// inputs rather than name them.
fn key(who: &[String], place: &[Option<usize>], seats: usize, tag: &str) -> String {
    let mut b = String::from("<div class=\"key\">");
    for p in 0..seats {
        let _ = write!(
            b,
            "<input type=\"checkbox\" id=\"{tag}-{p}\" checked>\
             <label for=\"{tag}-{p}\"><span class=\"swatch f{p}\"></span>{name}</label>",
            name = placed(p, &who[p], place[p]),
        );
    }
    b.push_str("</div>");
    b
}

/// How often the board paid each seat, turn by turn.
///
/// The companion to the production chart above it, and a different question
/// about the same board: that one is how much a seat collected, this is how
/// often they collected at all. A seat can hold a fifth of the pips on the
/// board and still be paid on a quarter of the rolls, which is a game spent
/// waiting rather than trading, and no cumulative total shows it.
///
/// Solid is what the seat actually collected on, robber and all; dotted, in the
/// same colour, is what their buildings reach. The gap between the two is what
/// a blockade cost in rolls rather than in cards.
fn reach(study: &Study, who: &[String], place: &[Option<usize>], seats: usize) -> String {
    const W: f64 = 720.0;
    const H: f64 = 280.0;
    const PAD: f64 = 36.0;
    const FOOT: f64 = 26.0;

    let c = &study.cover;
    let turns = c.turns();
    if turns == 0 || seats == 0 {
        return String::new();
    }
    let x = |i: usize| PAD + (W - PAD * 2.0) * i as f64 / (turns - 1).max(1) as f64;
    // The scale runs from nought to a quarter of certain, in quarters. Fixed
    // steps rather than a scale fitted to this game, so that two games are drawn
    // on the same axis and a line's height means something on its own; but only
    // as far up as the game reached, since a quarter of empty paper says
    // nothing either.
    let ceiling = c
        .open
        .iter()
        .flat_map(|r| r[..seats].iter().copied())
        .fold(0.25f64, f64::max);
    let top = (ceiling * 4.0).ceil() / 4.0;
    let y = |v: f64| H - FOOT - PAD - (H - FOOT - PAD * 2.0) * v / top;
    let base = H - FOOT - PAD;

    let mut b = String::from("<section>");
    b.push_str(&card_head(
        "Coverage",
        "The chance a roll pays a seat anything at all, turn by turn. Every \
         number their buildings reach, weighted by how often the dice make it \
         and counted once however many buildings sit on it. Solid is what they \
         collected on; dotted is what the buildings reach, with the robber \
         ignored, so the gap is what a blockade cost in rolls.",
    ));
    let _ = write!(
        b,
        "<div class=\"view\"><div class=\"frame\"><svg viewBox=\"0 0 {W} {H}\" \
         role=\"img\" aria-label=\"How often the board paid each seat\">"
    );
    // Quarters of certain, which are the gridlines a probability wants.
    for k in 1..=(top * 4.0).round() as u32 {
        let v = f64::from(k) / 4.0;
        let _ = write!(
            b,
            "<line class=\"grid\" x1=\"{PAD}\" x2=\"{r}\" y1=\"{gy}\" y2=\"{gy}\"/>\
             <text class=\"axis\" x=\"{tx}\" y=\"{ty}\">{p:.0}%</text>",
            r = W - PAD,
            gy = y(v),
            tx = PAD - 6.0,
            ty = y(v) + 4.0,
            p = v * 100.0,
        );
    }
    let step = nice_step(turns);
    let mut ticks: Vec<usize> = (0..turns).step_by(step).collect();
    if ticks.last() != Some(&(turns - 1)) {
        if ticks.last().is_some_and(|last| turns - 1 - last < step / 2) {
            ticks.pop();
        }
        ticks.push(turns - 1);
    }
    for i in ticks {
        let _ = write!(
            b,
            "<line class=\"tick\" x1=\"{tx:.1}\" x2=\"{tx:.1}\" y1=\"{base}\" y2=\"{end}\"/>\
             <text class=\"axis mid\" x=\"{tx:.1}\" y=\"{ly}\">{n}</text>",
            tx = x(i),
            end = base + 5.0,
            ly = base + 18.0,
            n = i + 1,
        );
    }
    let _ = write!(
        b,
        "<text class=\"axis start unit\" x=\"{PAD}\" y=\"14\">a roll pays</text>"
    );
    for p in 0..seats {
        for (rows, dash) in [(&c.live, ""), (&c.open, " owed")] {
            let path: String = (0..turns)
                .map(|i| format!("{:.1},{:.1}", x(i), y(rows[i][p])))
                .collect::<Vec<_>>()
                .join(" ");
            let _ = write!(
                b,
                "<polyline class=\"line k{n} f{p}{dash}\" points=\"{path}\"/>",
                n = p + 1,
            );
        }
    }
    b.push_str("</svg>");

    // The same per-turn slots the production chart uses, and for the same
    // reason: a line's height is readable and its exact figure is not.
    let slot = (W - PAD * 2.0) / (turns - 1).max(1) as f64;
    b.push_str("<div class=\"over\">");
    for i in 0..turns {
        let mut said = format!("Turn {}", i + 1);
        for p in 0..seats {
            let _ = write!(
                said,
                "\n{}: {:.0}%, reaching {:.0}%",
                who[p],
                c.live[i][p] * 100.0,
                c.open[i][p] * 100.0,
            );
        }
        let _ = write!(
            b,
            "<div class=\"slot{}\" style=\"left:{lx:.3}%;width:{lw:.3}%;top:{lt:.2}%;\
             height:{lh:.2}%\" data-tip=\"{}\"></div>",
            if x(i) > W / 2.0 { " to-left" } else { "" },
            esc(&said),
            lx = 100.0 * (x(i) - slot / 2.0) / W,
            lw = 100.0 * slot / W,
            lt = 100.0 * PAD / H,
            lh = 100.0 * (base - PAD) / H,
        );
    }
    b.push_str("</div></div>");

    // The legend, which is also the control, exactly as the chart above.
    b.push_str("<div class=\"key\">");
    for p in 0..seats {
        let _ = write!(
            b,
            "<input type=\"checkbox\" id=\"cv-{p}\" checked>\
             <label for=\"cv-{p}\"><span class=\"swatch f{p}\"></span>{name}</label>",
            name = placed(p, &who[p], place[p]),
        );
    }
    b.push_str("</div></div>");

    // And the figures the lines cannot be read off to the percent.
    b.push_str(T_OPEN);
    b.push_str("<thead>");
    b.push_str(&head_row(&[
        ("", ""),
        (
            "opening",
            "What the two settlements alone were paid on, which is the coverage \
             column on the opening card.",
        ),
        (
            "average",
            "Across every turn of the game, robber and all. The figure to \
             compare seats on: coverage that was high for ten turns and low for \
             a hundred was low.",
        ),
        (
            "at the end",
            "The board they finished on, which is what all that building came \
             to.",
        ),
        (
            "blocked",
            "How much of the average the robber took, in rolls. A tenth here is \
             a tenth of all rolls that would have paid this seat and did not.",
        ),
        (
            "a payout",
            "Cards on a roll that pays: the engine divided by the coverage. Two \
             seats can be owed the same cards a turn and collect them in \
             halves or in threes, and this is which of the two they were doing.",
        ),
    ]));
    b.push_str("</thead><tbody>");
    for p in 0..seats {
        let (live, open) = (c.mean(p, true), c.mean(p, false));
        b.push_str(&row(
            &[
                placed(p, &who[p], place[p]),
                percent(study.opening[p].coverage),
                percent(live),
                percent(c.live[turns - 1][p]),
                if open - live < 0.005 {
                    NONE.to_string()
                } else {
                    format!("<span class=\"down\">-{:.0}%</span>", (open - live) * 100.0)
                },
                // The engine is what a roll is owed on average, over all rolls;
                // dividing by the share of rolls that pay leaves what one of
                // those pays.
                match (mean_engine(study, p), c.mean(p, false)) {
                    (e, cov) if cov > 0.005 => format!("{:.1}", e / cov),
                    _ => NONE.to_string(),
                },
            ],
            false,
        ));
    }
    b.push_str("</tbody>");
    b.push_str(T_CLOSE);
    b.push_str(&per_resource(study, who, place, seats));
    b.push_str(&trail(study, who, seats));
    b.push_str("</section>");
    b
}

/// A seat's engine averaged over the whole game, in cards a roll.
fn mean_engine(study: &Study, seat: usize) -> f64 {
    if study.engine.is_empty() {
        return 0.0;
    }
    study.engine.iter().map(|row| row[seat]).sum::<f64>() / study.engine.len() as f64
}

/// Coverage a resource at a time, which is the builder's version of the question.
///
/// The column above answers "does a roll pay me anything", which is what a trader
/// wants to know. A builder wants something sharper: a settlement costs a brick,
/// a wood, a wool and a wheat, and a seat covered on four numbers that all make
/// wool is not covered for anything it is trying to build. A blank here is a
/// resource this seat could only ever get by trading for it.
fn per_resource(study: &Study, who: &[String], place: &[Option<usize>], seats: usize) -> String {
    let c = &study.cover;
    if c.each.is_empty() || seats == 0 {
        return String::new();
    }
    let mut b = String::from(T_OPEN);
    b.push_str("<thead>");
    let mut heads = vec![(
        "",
        "The chance a roll pays this seat this resource, averaged over the game, \
         robber and all. Not a share of anything: the five do not add to the \
         coverage above, because one roll can pay two resources at once and is \
         counted under both.",
    )];
    heads.extend(RESOURCE_NAMES.iter().map(|n| (*n, "")));
    b.push_str(&head_row(&heads));
    b.push_str("</thead><tbody>");
    for p in 0..seats {
        let mut cells = vec![placed(p, &who[p], place[p])];
        for res in 0..5 {
            let share = c.mean_of(p, res);
            cells.push(if share < 0.005 {
                NONE.to_string()
            } else {
                percent(share)
            });
        }
        b.push_str(&row(&cells, false));
    }
    b.push_str("</tbody>");
    b.push_str(T_CLOSE);
    b
}

/// The two halves of an economy plotted against each other: how often it pays,
/// and how much it pays when it does.
///
/// One point a quarter and a line joining them, so a seat is a path rather than
/// a dot. Which direction the path runs is the whole point. Up and to the right
/// is an engine getting both bigger and broader, which is what building a fifth
/// settlement on new numbers does. Straight up is more of the same numbers: the
/// payouts get bigger and no more frequent, which is a boom-and-bust economy
/// that spends most of its turns unable to trade. Straight right is the
/// opposite, and rare in practice.
///
/// A point a turn instead of a point a quarter would be a cloud of a hundred
/// and fifty dots per seat with no direction visible in it, which is why this is
/// quarters: the same four spans the production card divides the game into.
fn trail(study: &Study, who: &[String], seats: usize) -> String {
    const W: f64 = 720.0;
    const H: f64 = 300.0;
    const PAD: f64 = 44.0;
    const FOOT: f64 = 30.0;
    const QUARTERS: usize = 4;

    let c = &study.cover;
    let turns = c.turns();
    if turns < QUARTERS * 2 || seats == 0 || study.engine.len() != turns {
        return String::new();
    }
    // The same spans the production card cuts the game into.
    let cut = |k: usize| (turns * k).div_ceil(QUARTERS).min(turns);
    let span = |k: usize| (if k == 0 { 0 } else { cut(k) }, cut(k + 1));
    let mean = |from: usize, to: usize, of: &dyn Fn(usize) -> f64| {
        if to <= from {
            return of(from.min(turns - 1));
        }
        (from..to).map(of).sum::<f64>() / (to - from) as f64
    };

    // A point a seat a quarter: how often it paid, and what it was worth a roll.
    let point = |p: usize, k: usize| {
        let (from, to) = span(k);
        (
            mean(from, to, &|i| c.live[i][p]),
            mean(from, to, &|i| study.engine[i][p]),
        )
    };
    let top = (0..seats)
        .flat_map(|p| (0..QUARTERS).map(move |k| (p, k)))
        .map(|(p, k)| point(p, k).1)
        .fold(0.2f64, f64::max);
    let wide = (0..seats)
        .flat_map(|p| (0..QUARTERS).map(move |k| (p, k)))
        .map(|(p, k)| point(p, k).0)
        .fold(0.25f64, f64::max);
    let right = (wide * 4.0).ceil() / 4.0;
    // The axis starts at the last quarter below the narrowest economy rather
    // than at nought: nobody's coverage is near nothing, and a third of the
    // drawing would be paper.
    let narrow = (0..seats)
        .flat_map(|p| (0..QUARTERS).map(move |k| (p, k)))
        .map(|(p, k)| point(p, k).0)
        .fold(1.0f64, f64::min);
    let left = (narrow * 4.0).floor() / 4.0;
    let x = |v: f64| PAD + (W - PAD * 2.0) * (v - left) / (right - left).max(0.25);
    let y = |v: f64| H - FOOT - PAD - (H - FOOT - PAD * 2.0) * v / top;

    let mut b = String::from(
        "<div class=\"view\"><div class=\"frame\"><svg viewBox=\"0 0 720 300\" \
         role=\"img\" aria-label=\"How often each economy paid against how much\">",
    );
    for k in 1..=4 {
        let v = top * f64::from(k) / 4.0;
        let _ = write!(
            b,
            "<line class=\"grid\" x1=\"{PAD}\" x2=\"{r}\" y1=\"{gy}\" y2=\"{gy}\"/>\
             <text class=\"axis\" x=\"{tx}\" y=\"{ty}\">{v:.1}</text>",
            r = W - PAD,
            gy = y(v),
            tx = PAD - 6.0,
            ty = y(v) + 4.0,
        );
    }
    let base = H - FOOT - PAD;
    let mut at = left + 0.25;
    while at <= right + 1e-9 {
        let _ = write!(
            b,
            "<line class=\"tick\" x1=\"{tx:.1}\" x2=\"{tx:.1}\" y1=\"{base}\" y2=\"{end}\"/>\
             <text class=\"axis mid\" x=\"{tx:.1}\" y=\"{ly}\">{p:.0}%</text>",
            tx = x(at),
            end = base + 5.0,
            ly = base + 18.0,
            p = at * 100.0,
        );
        at += 0.25;
    }
    // Both names at the top, since the horizontal one on its own axis lands on
    // the last tick and neither can be read.
    let _ = write!(
        b,
        "<text class=\"axis start unit\" x=\"{PAD}\" y=\"14\">cards a roll</text>\
         <text class=\"axis unit\" x=\"{rx}\" y=\"14\">rolls that pay</text>",
        rx = W - PAD,
    );

    let mut tips = String::from("<div class=\"over\">");
    let mut shape = 0;
    for p in 0..seats {
        let path: String = (0..QUARTERS)
            .map(|k| {
                let (cov, size) = point(p, k);
                format!("{:.1},{:.1}", x(cov), y(size))
            })
            .collect::<Vec<_>>()
            .join(" ");
        let _ = write!(b, "<polyline class=\"trail f{p}\" points=\"{path}\"/>");
        for k in 0..QUARTERS {
            let (cov, size) = point(p, k);
            // The last quarter is where the economy ended, so it is the solid
            // one; the earlier ones are where it came from.
            // Earlier quarters are fainter and smaller, so the path reads as a
            // path without hovering it: the solid dot is where the economy
            // ended and the trail behind it is where it came from.
            let _ = write!(
                b,
                "<circle class=\"stop f{p} q{k} k{shape}{}\" cx=\"{:.1}\" cy=\"{:.1}\" \
                 r=\"{:.1}\"/>",
                if k + 1 == QUARTERS { " last" } else { "" },
                x(cov),
                y(size),
                3.0 + k as f64,
            );
            tips.push_str(&over_tip(
                shape,
                x(cov),
                y(size),
                W,
                H,
                &format!(
                    "{}, {} quarter\n{:.0}% of rolls pay\n{:.2} cards a roll\n{:.1} on a \
                     roll that pays",
                    who[p],
                    ordinal(k + 1),
                    cov * 100.0,
                    size,
                    if cov > 0.005 { size / cov } else { 0.0 },
                ),
            ));
            shape += 1;
        }
        let (cov, size) = point(p, QUARTERS - 1);
        tips.push_str(&over_label(
            x(cov) + 12.0,
            y(size),
            W,
            H,
            "start",
            &format!("<span class=\"dot s{p}\"></span>{}", esc(&who[p])),
        ));
    }
    b.push_str("</svg>");
    tips.push_str("</div>");
    b.push_str(&tips);
    b.push_str(&tip_rules(".trails", shape));
    b.push_str("</div></div>");
    format!("<div class=\"trails\">{b}</div>")
}

/// A chance, as a whole percentage, since a tenth of a percent of a roll is not
/// a thing anybody plays around.
fn percent(v: f64) -> String {
    format!("{:.0}%", v * 100.0)
}

/// Production against expectation, turn by turn, with a switch above it.
///
/// The default is every seat at once: a solid line for what each collected and
/// a dotted one, in the same colour, for what the pips led them to expect. Pick a seat
/// and the same chart is drawn a resource at a time, which is the only way to
/// see *which* card a placement was short of.
///
/// The switch is radio inputs and a sibling selector, and the legend below is
/// checkboxes doing the same trick, so the page still carries no script.
fn curves(study: &Study, who: &[String], place: &[Option<usize>], seats: usize) -> String {
    let s = &study.series;
    if s.turns() < 2 {
        return String::new();
    }

    let mut b = String::from("<section>");
    b.push_str(&card_head(
        "Production per turn",
        "Solid is what the board actually paid; dotted is what the pips through \
         the buildings standing at each roll should have paid at fair odds. \
         Both running \
         totals, so each line only climbs and the gap between a pair is \
         everything that has happened to that seat so far. The robber is \
         ignored in the expectation, so a seat under blockade watches its solid \
         line fall away from its dotted one, which is what a blockade costs. \
         Click a name below a chart to take its lines off it.",
    ));

    // The switch. One radio per view, named together so they are one control.
    b.push_str("<div class=\"modes\">");
    for (i, label) in std::iter::once("everybody".to_string())
        .chain((0..seats).map(|p| esc(&who[p])))
        .enumerate()
    {
        // A seat's own colour, on the mark left of the name as everywhere else,
        // and on the whole pill once it is the one being looked at. Picking
        // Ines and then reading four teal lines is a moment of doubt the
        // control can spend a colour to remove.
        let seat = i.checked_sub(1).map(|p| p.min(MAX_PLAYERS - 1));
        let _ = write!(
            b,
            "<input type=\"radio\" name=\"view\" id=\"view{i}\"{on}>\
             <label for=\"view{i}\"{class}>{mark}{label}</label>",
            on = if i == 0 { " checked" } else { "" },
            // `m` for the mode, and not the `s` the seat marks use: those set a
            // background on anything wearing them, which painted every pill in
            // the row rather than the one that was picked.
            class = seat.map_or(String::new(), |p| format!(" class=\"seat m{p}\"")),
            mark = seat.map_or(String::new(), |p| format!(
                "<span class=\"dot s{p}\"></span>"
            )),
        );
    }
    b.push_str("</div><div class=\"views\">");

    // Everybody: two lines a seat, in the seat's colour.
    let all: Vec<Curve> = (0..seats)
        .map(|p| Curve {
            colour: format!("f{p}"),
            name: label(&who[p], place[p]),
            badge: placed(p, &who[p], place[p]),
            actual: (0..s.turns())
                .map(|i| f64::from(s.actual[i][p].iter().sum::<u32>()))
                .collect(),
            owed: (0..s.turns())
                .map(|i| s.expected[i][p].iter().sum::<f64>())
                .collect(),
        })
        .collect();
    b.push_str(&plot(
        0,
        &all,
        s.ceiling(seats),
        s.turns(),
        &table_all(study, who, place, seats),
    ));

    // And one view a seat, drawn a resource at a time.
    for p in 0..seats {
        let each: Vec<Curve> = RESOURCE_NAMES
            .iter()
            .enumerate()
            .map(|(res, name)| Curve {
                colour: format!("r{res}"),
                name: (*name).to_string(),
                badge: (*name).to_string(),
                actual: (0..s.turns())
                    .map(|i| f64::from(s.actual[i][p][res]))
                    .collect(),
                owed: (0..s.turns()).map(|i| s.expected[i][p][res]).collect(),
            })
            .collect();
        b.push_str(&plot(
            p + 1,
            &each,
            s.ceiling_of(p),
            s.turns(),
            &table_one(study, p),
        ));
    }
    b.push_str("</div></section>");
    b
}

/// Every seat against every resource: what the board paid, with what the pips
/// led them to expect in brackets.
fn table_all(study: &Study, who: &[String], place: &[Option<usize>], seats: usize) -> String {
    let s = &study.series;
    let last = s.turns() - 1;
    let mut b = String::from(T_OPEN);
    b.push_str("<thead>");
    let mut heads = vec![("", "")];
    heads.extend(RESOURCE_NAMES.iter().map(|n| (*n, "")));
    heads.push((
        "total",
        "Every card the board paid this seat, against everything the pips led \
         them to expect. The gap is the robber and the dice together, which the \
         ledger above splits apart.",
    ));
    heads.push((
        "deviation",
        "How far the total ran over or under what was expected, as a share of \
         it. Minus a fifth means this seat collected four cards for every five \
         the board owed them.",
    ));
    b.push_str(&head_row(&heads));
    b.push_str("</thead><tbody>");
    let (mut got, mut owed) = ([0u32; 5], [0.0f64; 5]);
    for p in 0..seats {
        let mut cells = vec![placed(p, &who[p], place[p])];
        for res in 0..5 {
            got[res] += s.actual[last][p][res];
            owed[res] += s.expected[last][p][res];
            cells.push(against(s.actual[last][p][res], s.expected[last][p][res]));
        }
        let (a, e) = (
            f64::from(s.actual[last][p].iter().sum::<u32>()),
            s.expected[last][p].iter().sum::<f64>(),
        );
        cells.push(against(s.actual[last][p].iter().sum(), e));
        cells.push(share(a, e));
        b.push_str(&row(&cells, false));
    }
    b.push_str("</tbody>");
    let mut foot = vec!["the board".to_string()];
    foot.extend((0..5).map(|res| against(got[res], owed[res])));
    foot.push(against(got.iter().sum(), owed.iter().sum()));
    foot.push(share(
        f64::from(got.iter().sum::<u32>()),
        owed.iter().sum::<f64>(),
    ));
    b.push_str(&totals(&foot));
    b.push_str(T_CLOSE);

    // The same question asked four times across the game, because when a seat
    // was starved matters: cards missing early delay everything they would
    // have bought, and the same shortfall at the end costs one purchase.
    let labels: Vec<String> = (0..seats).map(|p| placed(p, &who[p], place[p])).collect();
    b.push_str(&quarters(s, &labels, |p, i| {
        (
            f64::from(s.actual[i][p].iter().sum::<u32>()),
            s.expected[i][p].iter().sum::<f64>(),
        )
    }));
    b
}

/// How far each row ran over or under, four times across the game and once for
/// the whole of it.
///
/// The rows differ and the question does not: seats under the everybody chart,
/// resources under a seat's own. `at(row, i)` is that row's running total and
/// running expectation at sample `i`, and a quarter is what the two grew by
/// across it.
fn quarters(
    s: &crate::analysis::Series,
    labels: &[String],
    at: impl Fn(usize, usize) -> (f64, f64),
) -> String {
    let last = s.turns().saturating_sub(1);
    let mut b = String::from(T_OPEN);
    b.push_str("<thead>");
    let cut = |k: usize| (s.turns() * k).div_ceil(4).min(s.turns()).saturating_sub(1);
    // Named alike and carrying the turns they cover, since "the third quarter"
    // of a hundred and fifty turns is not a span anybody holds in their head.
    let spans: Vec<String> = (0..4)
        .map(|k| {
            let from = if k == 0 { 1 } else { cut(k) + 2 };
            format!(
                "{} quarter<span class=\"range\">turns {from} to {}</span>",
                ordinal(k + 1),
                cut(k + 1) + 1,
            )
        })
        .collect();
    // Every column carries the same note, since a reader hovering the fourth
    // quarter should not have to find the first one to learn what the brackets
    // hold.
    const OVER: &str = "How far this span ran over or under what was expected, \
                        as a share of it, with the cards that arrived and the \
                        cards expected in brackets. A quarter of four cards \
                        swings further than one of forty.";
    let mut heads = vec![("", "")];
    heads.extend(spans.iter().map(|s| (s.as_str(), OVER)));
    heads.push((
        "total",
        "The four quarters together, which is the deviation column above, with \
         everything that arrived and everything expected in brackets.",
    ));
    b.push_str(&head_row(&heads));
    b.push_str("</thead><tbody>");
    for (r, label) in labels.iter().enumerate() {
        let mut cells = vec![label.clone()];
        for k in 0..4 {
            let (from, to) = (if k == 0 { None } else { Some(cut(k)) }, cut(k + 1));
            let (a1, e1) = at(r, to);
            let (a0, e0) = match from {
                Some(i) => at(r, i),
                None => (0.0, 0.0),
            };
            cells.push(share_of(a1 - a0, e1 - e0));
        }
        let (a, e) = at(r, last);
        cells.push(share_of(a, e));
        b.push_str(&row(&cells, false));
    }
    b.push_str("</tbody>");
    b.push_str(T_CLOSE);
    b
}

/// The same, with the two figures it was worked out from in brackets.
///
/// A share on its own hides how much it was taken from, and a quarter of a
/// short game can hold a handful of cards: minus a third of six is one bad
/// roll, and minus a third of sixty is a game being lost. Both numbers rather
/// than one, in the order the rest of the page writes them, so the percentage
/// can be checked against what it came from.
fn share_of(got: f64, owed: f64) -> String {
    match share(got, owed).as_str() {
        NONE => NONE.to_string(),
        s => format!("{s} <span class=\"worth\">({got:.0} of {owed:.1})</span>"),
    }
}

/// How far a figure ran over or under what was expected, as a share of it.
fn share(got: f64, owed: f64) -> String {
    if owed < 0.05 {
        return NONE.to_string();
    }
    let d = (got - owed) / owed * 100.0;
    format!(
        "<span class=\"{}\">{d:+.0}%</span>",
        if d >= 0.0 { "up" } else { "down" }
    )
}

/// One seat, a resource a row: what arrived, what was expected, the gap between
/// them, and how much of their production each resource was.
///
/// A row per resource rather than a column, because this table answers a
/// different question from the one above it. That one asks who did better; this
/// one asks what this seat was living on, and what it was short of.
fn table_one(study: &Study, seat: usize) -> String {
    let s = &study.series;
    let last = s.turns() - 1;
    let total: u32 = s.actual[last][seat].iter().sum();
    let mut b = String::from(T_OPEN);
    b.push_str("<thead>");
    b.push_str(&head_row(&[
        ("", ""),
        ("production", "Cards of this kind the board paid them."),
        (
            "expected",
            "What the pips through their buildings led them to expect at fair \
             odds, the robber ignored.",
        ),
        (
            "difference",
            "What arrived less what was expected. Negative is the robber and \
             the dice together; over a whole game they rarely cancel.",
        ),
        (
            "share",
            "How much of everything this seat collected was this resource. A \
             hand is what the board gave it, and this is the shape of that.",
        ),
    ]));
    b.push_str("</thead><tbody>");
    for res in 0..5 {
        let got = s.actual[last][seat][res];
        let owed = s.expected[last][seat][res];
        let d = f64::from(got) - owed;
        b.push_str(&row(
            &[
                format!("<span class=\"dot r{res}\"></span>{}", RESOURCE_NAMES[res]),
                got.to_string(),
                format!("{owed:.1}"),
                format!(
                    "<span class=\"{}\">{:+.1}</span>",
                    if d >= 0.0 { "up" } else { "down" },
                    d
                ),
                if total == 0 {
                    NONE.to_string()
                } else {
                    format!("{:.0}%", 100.0 * f64::from(got) / f64::from(total))
                },
            ],
            false,
        ));
    }
    b.push_str("</tbody>");
    let owed: f64 = s.expected[last][seat].iter().sum();
    b.push_str(&totals(&[
        "all of it".to_string(),
        total.to_string(),
        format!("{owed:.1}"),
        format!(
            "<span class=\"{}\">{:+.1}</span>",
            if f64::from(total) >= owed {
                "up"
            } else {
                "down"
            },
            f64::from(total) - owed
        ),
        if total == 0 {
            NONE.to_string()
        } else {
            "100%".to_string()
        },
    ]));
    b.push_str(T_CLOSE);
    // And the same question the everybody view asks of the seats, asked of this
    // seat's five resources: which of them dried up, and when. A seat that lost
    // its ore in the third quarter was not short of ore all game, and the
    // whole-game figure above cannot tell the two apart.
    let labels: Vec<String> = (0..5)
        .map(|res| format!("<span class=\"dot r{res}\"></span>{}", RESOURCE_NAMES[res]))
        .collect();
    b.push_str(&quarters(s, &labels, |res, i| {
        (f64::from(s.actual[i][seat][res]), s.expected[i][seat][res])
    }));
    b
}

/// What arrived, with what was expected in brackets.
///
/// The same shape the result table uses for points and the development table
/// for cards: two figures that belong together, read off one another.
fn against(got: u32, owed: f64) -> String {
    if got == 0 && owed < 0.05 {
        NONE.to_string()
    } else {
        format!("{got} <span class=\"worth\">({owed:.1})</span>")
    }
}

/// About how many labels an axis wants before it is a wall of numbers.
const TICKS: usize = 8;

/// A step for the turn axis that lands on numbers somebody would choose.
fn nice_step(turns: usize) -> usize {
    [1, 2, 5, 10, 20, 25, 50, 100, 200]
        .into_iter()
        .find(|s| turns / s <= TICKS)
        .unwrap_or(turns.max(1))
}

/// One chart: a pair of lines per curve, on one axis.
///
/// Every curve shares the ceiling, or the gap between a solid line and its
/// dotted one would be a picture of two scales rather than of a difference.
fn plot(view: usize, curves: &[Curve], ceiling: f64, turns: usize, table: &str) -> String {
    const W: f64 = 720.0;
    const H: f64 = 280.0;
    const PAD: f64 = 36.0;
    const FOOT: f64 = 26.0;
    let top = if ceiling > 0.0 { ceiling } else { 1.0 };
    let x = |i: usize| PAD + (W - PAD * 2.0) * i as f64 / (turns - 1).max(1) as f64;
    let y = |v: f64| H - FOOT - PAD - (H - FOOT - PAD * 2.0) * v / top;

    let mut b = format!(
        "<div class=\"view\"><div class=\"frame\"><svg viewBox=\"0 0 {W} {H}\" \
         role=\"img\" aria-label=\"Cumulative production against expectation\">"
    );
    // Gridlines and their values, so the height of a line can be read.
    for k in 1..=4 {
        let v = top * f64::from(k) / 4.0;
        let _ = write!(
            b,
            "<line class=\"grid\" x1=\"{PAD}\" x2=\"{r}\" y1=\"{gy}\" y2=\"{gy}\"/>\
             <text class=\"axis\" x=\"{tx}\" y=\"{ty}\">{v:.0}</text>",
            r = W - PAD,
            gy = y(v),
            tx = PAD - 6.0,
            ty = y(v) + 4.0,
        );
    }
    // Turns along the bottom, at a step somebody would have chosen, with the
    // last one always named: where the game ended is the one turn a reader
    // looks for and an even step will usually miss it.
    let step = nice_step(turns);
    let base = H - FOOT - PAD;
    let mut ticks: Vec<usize> = (0..turns).step_by(step).collect();
    if ticks.last() != Some(&(turns - 1)) {
        // Drop a label that would collide with the last one.
        if ticks.last().is_some_and(|last| turns - 1 - last < step / 2) {
            ticks.pop();
        }
        ticks.push(turns - 1);
    }
    for i in ticks {
        let _ = write!(
            b,
            "<line class=\"tick\" x1=\"{tx:.1}\" x2=\"{tx:.1}\" y1=\"{base}\" y2=\"{end}\"/>\
             <text class=\"axis mid\" x=\"{tx:.1}\" y=\"{ly}\">{n}</text>",
            tx = x(i),
            end = base + 5.0,
            ly = base + 18.0,
            n = i + 1,
        );
    }
    // Only the vertical axis is named. "Turn" on the horizontal would collide
    // with the last tick, and the heading above already says per turn.
    let _ = write!(
        b,
        "<text class=\"axis start unit\" x=\"{PAD}\" y=\"14\">cards</text>"
    );

    for (k, c) in curves.iter().enumerate() {
        for (points, dash) in [(&c.actual, ""), (&c.owed, " owed")] {
            let path: String = points
                .iter()
                .enumerate()
                .map(|(i, v)| format!("{:.1},{:.1}", x(i), y(*v)))
                .collect::<Vec<_>>()
                .join(" ");
            // A line says nothing on hover: the slots below cover the drawing
            // and take the pointer first, and the legend already names it.
            let _ = write!(
                b,
                "<polyline class=\"line k{n} {colour}{dash}\" points=\"{path}\"/>",
                n = k + 1,
                colour = c.colour,
            );
        }
    }

    b.push_str("</svg>");

    // A slot per turn, laid over the drawing rather than drawn in it: page
    // HTML, so it carries the same tooltip a table column carries and says its
    // figures at the same size. The guide under the pointer is the slot's own
    // left edge, drawn by the stylesheet.
    //
    // The box is hung from the top of the chart rather than from the pointer,
    // because the lines climb as the game goes on and a box that followed the
    // pointer up would sit on the very figures it is describing.
    let slot = (W - PAD * 2.0) / (turns - 1).max(1) as f64;
    b.push_str("<div class=\"over\">");
    for i in 0..turns {
        let mut said = format!("Turn {}", i + 1);
        for c in curves {
            let _ = write!(
                said,
                "\n{}: {:.0}, expected {:.1}",
                c.name, c.actual[i], c.owed[i]
            );
        }
        let _ = write!(
            b,
            "<div class=\"slot{}\" style=\"left:{lx:.3}%;width:{lw:.3}%;top:{lt:.2}%;\
             height:{lh:.2}%\" data-tip=\"{}\"></div>",
            if x(i) > W / 2.0 { " to-left" } else { "" },
            esc(&said),
            lx = 100.0 * (x(i) - slot / 2.0) / W,
            lw = 100.0 * slot / W,
            lt = 100.0 * PAD / H,
            lh = 100.0 * (base - PAD) / H,
        );
    }
    b.push_str("</div></div>");

    // The legend, which is also the control: a checkbox a curve, so clicking a
    // name takes its two lines off the chart.
    b.push_str("<div class=\"key\">");
    for (k, c) in curves.iter().enumerate() {
        let _ = write!(
            b,
            "<input type=\"checkbox\" id=\"k{view}-{k}\" checked>\
             <label for=\"k{view}-{k}\"><span class=\"swatch {colour}\"></span>{name}</label>",
            colour = c.colour,
            // Already markup: a name with its place badge, as a table writes it.
            name = c.badge,
        );
    }
    b.push_str("</div>");
    b.push_str(table);
    b.push_str("</div>");
    b
}

/// A place, written as a place: 1st, 2nd, 3rd, 4th.
///
/// A rank is an ordinal and a bare "2" in a column of figures reads as a
/// quantity. Only the small numbers this pool ever produces are handled well,
/// which is every number it produces.
fn ordinal(n: usize) -> String {
    let suffix = match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

/// A duration, at whatever precision it is worth reading at./// A duration, at whatever precision it is worth reading at.
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

pub fn page(saved: &Saved, study: &Study, account: &str) -> String {
    let r = &study.report;
    let seats = r.players as usize;
    let who = names(saved, seats);
    let place = places(r, seats);
    let mut b = String::new();

    let _ = write!(
        b,
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>Carranta, game {id}</title>{ICON}\
         <style>{css}</style></head><body>",
        id = esc(&saved.id),
        css = CSS
    );

    // ---- the header: which game, and what happened in it --------------------
    b.push_str(&masthead_as(
        "",
        &[
            (&format!("/{}/", esc(&saved.id)), "The board"),
            ("/corpus", "Across games"),
            ("/lobby", "New game"),
        ],
        account,
    ));
    let _ = write!(
        b,
        "<main><h1>Game {id}</h1>\
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
        "Result",
        "The five things that score and nothing else (R-11.3), each as how many \
         were held with what they were worth in brackets, so the brackets add \
         across to the total. Read off the final position rather than off what \
         was built, which is a different number: a settlement upgraded to a \
         city stopped being a settlement and was still built.",
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
        let mut cells = vec![
            placed(s, &who[s], place[s]),
            format!("<strong>{}</strong>", r.vp[s]),
        ];
        cells.extend(study.points[s].parts().iter().map(|p| scored(*p)));
        b.push_str(&row(&cells, false));
    }
    b.push_str("</tbody>");
    b.push_str(T_CLOSE);
    // Under the table, the same figures turn by turn: whether the game was ever
    // close is not in a final score, and it is the first thing anybody asks.
    b.push_str(&score_plot(study, &who, &place, seats));
    // And what was happening while those lines moved, on the same turn axis.
    b.push_str(&timeline(study, &who, seats));
    b.push_str(&race(study, &who, &place, seats));
    b.push_str("</section>");

    // ---- the turns ----------------------------------------------------------
    b.push_str(&turn_bar(study, &who, &place, seats));

    // ---- ratings ------------------------------------------------------------
    b.push_str("<section>");
    b.push_str(&card_head(
        "Ratings",
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
                "rating before",
                "The conservative estimate going in: three standard deviations \
                 below the mean, which is what a rating is worth believing \
                 rather than what it is.",
            ),
            (
                "rating after",
                "The same figure once this game was folded in.",
            ),
            (
                "change",
                "An update is a redistribution, so these very nearly cancel: \
                 somebody gains what somebody else loses.",
            ),
            (
                "rank before",
                "Where they stood in the whole pool going in, best first. The \
                 pool, not this table: a rating is a claim about every player on \
                 this server. Blank before they had a rating to stand anywhere \
                 with, since an unrated player is not last, they are absent.",
            ),
            (
                "rank after",
                "And where they stand now. A rating can rise and a rank fall, \
                 if somebody else rose further.",
            ),
            (
                "total games played",
                "Every game this player has been rated on here, this one \
                 included, which is how much the figure beside it is worth \
                 believing. Early games move a rating a long way because the \
                 belief starts wide.",
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
                    placed(s, &who[s], place[s]),
                    n1(m.before.conservative()),
                    n1(m.after.conservative()),
                    format!("<span class=\"{cls}\">{}</span>", signed(d)),
                    m.rank_before.map_or(NONE.to_string(), ordinal),
                    m.rank_after.map_or(NONE.to_string(), ordinal),
                    // `games` is what each had behind them going in, and this
                    // game is now one of them.
                    (m.games + 1).to_string(),
                ],
                false,
            ));
        }
        b.push_str("</tbody>");
        // No totals row. Before and after are positions rather than
        // quantities, four players' games played is not a number of anything,
        // and the changes do not cancel exactly enough for their sum to be
        // worth printing: a row of totals here would be four cells of noise.
        b.push_str(T_CLOSE);
    }
    b.push_str("</section>");

    // ---- the dice -----------------------------------------------------------
    let total: u32 = r.rolls.iter().sum();
    b.push_str("<section>");
    b.push_str(&card_head(
        "Dice",
        "No p-value, deliberately (§10.1). Across enough games one in twenty \
         clears p<0.05 by construction, and those are precisely the games \
         somebody screenshots as proof of rigging. The percentile carries the \
         same information with no significance claim attached, and until there \
         is a second game it is withheld, because a percentile of one game is \
         not a percentile. Whether the generator itself is fair is a different \
         question asked of millions of pooled rolls, never of one game.",
    ));
    // What a fair pair owes each number, at this many rolls.
    let expect = |n: u32| total as f64 * (6 - (n as i32 - 7).abs()) as f64 / 36.0;
    b.push_str("<div class=\"tw\"><table class=\"rolls\"><thead>");
    // The rolls as bars, with the fair-dice expectation marked across them.
    //
    // Drawn as a row of the table rather than as a chart above it, so the
    // alignment between a bar and its column is the table's own and cannot come
    // apart: eleven columns lined up by hand would need lining up again every
    // time a padding changed.
    //
    // Both scaled to the tallest thing either of them reaches, so the bars and
    // the marks are on one axis. Without that the marks would sit at heights
    // that mean nothing next to the bars.
    let tallest = (2..=12u32)
        .map(|n| expect(n).max(f64::from(r.rolls[n as usize - 2])))
        .fold(0.0, f64::max);
    if tallest > 0.0 {
        b.push_str("<tr class=\"chart\"><td></td>");
        for n in 2..=12u32 {
            let got = r.rolls[n as usize - 2];
            let _ = write!(
                b,
                "<td><div class=\"col\" data-tip=\"{n}: rolled {got}, expected {e}\">\
                 <div class=\"stem\" style=\"height:{h:.1}%\"></div>\
                 <div class=\"owed\" style=\"bottom:{m:.1}%\"></div></div></td>",
                e = n1(expect(n)),
                h = 100.0 * f64::from(got) / tallest,
                m = 100.0 * expect(n) / tallest,
            );
        }
        b.push_str("</tr>");
    }
    let labels: Vec<String> = (2..=12).map(|n: u32| n.to_string()).collect();
    let mut heads = vec![("", "")];
    heads.extend(labels.iter().map(|s| (s.as_str(), "")));
    b.push_str(&head_row(&heads));
    b.push_str("</thead><tbody>");
    let mut actual = vec!["rolled".to_string()];
    actual.extend(r.rolls.iter().map(u32::to_string));
    b.push_str(&row(&actual, false));
    let mut expected = vec!["expected".to_string()];
    expected.extend((2..=12u32).map(|n| n1(expect(n))));
    b.push_str(&row(&expected, false));
    b.push_str("</tbody>");
    b.push_str(T_CLOSE);
    // How far the two rows above are apart, and how that compares. Scalars, so
    // their own table rather than a sentence over the histogram.
    b.push_str(T_OPEN);
    b.push_str("<thead>");
    b.push_str(&head_row(&[
        ("", ""),
        (
            "out of place",
            "How many rolls landed on a different number than a fair spread \
             would put them on: the count that would have to move to make the \
             histogram fair. The same deviation as the bits beside it, in a unit \
             somebody can picture, since nobody has ever looked at a game and \
             thought in bits.",
        ),
        (
            "deviation",
            "How far these rolls fell from a fair pair, as a KL divergence in \
             bits, less the bias a finite sample puts into it. An effect size, \
             which is the figure §10.1 asks for in place of a significance \
             claim. Nought means these dice were indistinguishable from fair, \
             which most games are.",
        ),
        (
            "standing",
            "Where this game sits among every finished game recorded here, most \
             deviant first. A place rather than a percentile while the corpus is \
             small, because a percentile of six games moves twenty points when a \
             seventh is played. Blank until there is a second game to stand \
             against.",
        ),
        (
            "games compared",
            "Other finished games at this table's settings.",
        ),
    ]));
    b.push_str("</thead><tbody>");
    b.push_str(&row(
        &[
            "these dice".to_string(),
            format!(
                "{:.0} <span class=\"worth\">of {}</span>",
                study.dice.misplaced, study.dice.rolls
            ),
            format!("{:.3} bits", study.dice.kl_fair),
            match (study.dice_rank, study.dice_percentile) {
                // A rank is what the corpus knows while it is small. Past a
                // score of games a percentile carries more than a place does.
                (Some((rank, of)), _) if of < 20 => {
                    format!("{} <span class=\"worth\">of {of}</span>", ordinal(rank))
                }
                // "More than 100% of five games" is not a figure anybody wants
                // to read. At the top of the range the answer is all of them.
                (_, Some(p)) if p >= 99.5 => "every one".to_string(),
                (_, Some(p)) if p < 0.5 => "none".to_string(),
                (_, Some(p)) => format!("{p:.0}%"),
                _ => NONE.to_string(),
            },
            study.corpus_games.to_string(),
        ],
        false,
    ));
    b.push_str("</tbody>");
    b.push_str(T_CLOSE);
    b.push_str("</section>");

    // ---- production ---------------------------------------------------------
    // Every card that reached a hand or left it, by what moved it. This is the
    // steal grid and the production decomposition in one place, because they
    // were two views of one thing: where the cards went.
    b.push_str("<section>");
    b.push_str(&card_head(
        "Production",
        "Every card that reached a hand or left it, and what moved it. Read \
         down: what came in, less what went out, is what was still in hand when \
         the game ended, which is what makes this a ledger rather than a list. \
         Road Building is not here because it pays in roads rather than in \
         cards, and a victory point card is never played at all.",
    ));
    b.push_str(T_OPEN);
    b.push_str("<thead>");
    let mut heads = vec![("", "")];
    let named: Vec<String> = (0..seats).map(|s| placed(s, &who[s], place[s])).collect();
    heads.extend(named.iter().map(|n| (n.as_str(), "")));
    b.push_str(&head_row(&heads));
    b.push_str("</thead><tbody>");
    const WHY: [(&str, &str); 10] = [
        (
            "production",
            "Paid by the board on a roll, and by the second opening \
                        settlement, which pays for the hexes it touches.",
        ),
        (
            "invention",
            "Taken with an Invention card, two of anything (R-9.9).",
        ),
        (
            "monopoly",
            "Taken from every other hand with a Monopoly (R-9.8).",
        ),
        (
            "stolen",
            "Taken from a hand by the robber, on a seven or on a militia \
                    (R-6.4).",
        ),
        (
            "traded in",
            "Arrived in a trade, with a person or with the supply.",
        ),
        (
            "built",
            "Spent on a road, a settlement, a city or a development card.",
        ),
        (
            "discarded",
            "Thrown away to a seven (R-6.2). Nobody's choice but the \
                       dice's.",
        ),
        ("robbed", "Taken out of this hand by the robber."),
        (
            "monopolised",
            "Taken out of this hand by somebody else's Monopoly.",
        ),
        (
            "traded out",
            "Left in a trade, to a person or to the supply.",
        ),
    ];
    let cell = |v: u32, sign: bool| {
        if v == 0 {
            NONE.to_string()
        } else if sign {
            v.to_string()
        } else {
            format!("&minus;{v}")
        }
    };
    let mut written = 0;
    for (i, (name, why)) in WHY.iter().enumerate() {
        // The subtotals fall between the two halves and at the end, where the
        // reader has just finished adding the rows above them.
        if i == 5 {
            b.push_str(&sub_row(
                "came in",
                &(0..seats)
                    .map(|s| study.ledger[s].came_in().to_string())
                    .collect::<Vec<_>>(),
            ));
        }
        let mut cells = vec![format!("<span data-tip=\"{}\">{name}</span>", esc(why))];
        for s in 0..seats {
            let led = study.ledger[s];
            cells.push(cell(led.rows()[i].1, led.rows()[i].2));
        }
        b.push_str(&row(&cells, false));
        written += 1;
    }
    debug_assert_eq!(written, WHY.len());
    b.push_str(&sub_row(
        "went out",
        &(0..seats)
            .map(|s| format!("&minus;{}", study.ledger[s].went_out()))
            .collect::<Vec<_>>(),
    ));
    b.push_str(&sub_row(
        "left in hand",
        &(0..seats)
            .map(|s| study.ledger[s].held.to_string())
            .collect::<Vec<_>>(),
    ));
    b.push_str(&row(
        &std::iter::once(format!(
            "<span data-tip=\"{}\">most at once</span>",
            esc(
                "The most cards this seat ever held at one time. Not part of \
                 the ledger's arithmetic: a peak is a moment rather than a \
                 flow, and it is here because it is a fact about the same hand."
            )
        ))
        .chain((0..seats).map(|s| r.peak_hand[s].to_string()))
        .collect::<Vec<_>>(),
        false,
    ));
    // What made the discards possible, which is a different fact from what they
    // cost: a seat that ended thirty turns holding eight cards was betting every
    // one of them that the next seven belonged to somebody else.
    if study.hands.turns > 0 {
        b.push_str(&row(
            &std::iter::once(format!(
                "<span data-tip=\"{}\">turns over seven</span>",
                esc(
                    "Turns this seat ended holding more than seven cards, which \
                     is the hand a seven takes half of (R-6.2). The discarded row \
                     is what that cost; this is how long they were exposed to it. \
                     Discarding nothing all game is careful play or a quiet \
                     table, and only the two rows together say which."
                )
            ))
            .chain((0..seats).map(|s| {
                let over = study.hands.over[s];
                if over == 0 {
                    NONE.to_string()
                } else {
                    format!(
                        "{over} <span class=\"worth\">({:.0}%)</span>",
                        100.0 * f64::from(over) / study.hands.turns as f64
                    )
                }
            }))
            .collect::<Vec<_>>(),
            false,
        ));
    }
    b.push_str("</tbody>");
    b.push_str(T_CLOSE);
    b.push_str(&thrown(study, &who, &place, seats));
    b.push_str("</section>");

    // ---- production per turn -------------------------------------------------
    b.push_str(&curves(study, &who, &place, seats));

    // ---- what became of the expectation --------------------------------------
    // The deviation column above is three causes in one number, and the engine
    // has always known which was which (§10.2).
    b.push_str(&deviation_card(study, &who, &place, seats));

    // ---- the militia --------------------------------------------------------
    b.push_str("<section>");
    b.push_str(&card_head(
        "Militia",
        "Where the robber went and what it took. The cards themselves are in \
         the ledger above, as stolen and robbed; this is who took them from \
         whom, which a per-player column cannot say. The robber moves on a \
         seven as well as on a militia, and most of these are sevens.",
    ));
    b.push_str(&sankey(r, &who, &place, seats));
    b.push_str(T_OPEN);
    b.push_str("<thead>");
    b.push_str(&head_row(&[
        ("", ""),
        (
            "moved",
            "Times the robber was placed, on a seven or by a militia. One \
             piece, so this is the game's count and not any seat's.",
        ),
        (
            "found nothing",
            "Robberies that reached for a hand and found it empty (R-6.4). The \
             move still happened and still cost the victim's hexes.",
        ),
    ]));
    b.push_str("</thead><tbody>");
    b.push_str(&row(
        &[
            "the robber".to_string(),
            r.robber_moves.to_string(),
            r.empty_robberies.to_string(),
        ],
        false,
    ));
    b.push_str("</tbody>");
    b.push_str(T_CLOSE);
    b.push_str(&blockade(study, &who, &place, seats));
    b.push_str("</section>");

    // ---- the market ---------------------------------------------------------
    b.push_str("<section>");
    b.push_str(&card_head(
        "Trades",
        "Negotiation churn, which under an open market is most of the \
         interaction in a game (H-4). A completed trade is counted for both \
         sides of it, so that column totals to twice the number of trades. The \
         circle above counts each trade once, and takes the bank and the ports \
         as parties, since a trade with the supply is still a trade.",
    ));
    b.push_str(&chord(study, &who, &place, seats));
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
                placed(s, &who[s], place[s]),
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
    b.push_str(&flows(study, &who, &place, seats));
    b.push_str(&offers(study, &who, &place, seats));
    b.push_str(&demand(study, &who, &place, seats));
    b.push_str("</section>");

    // ---- building -----------------------------------------------------------
    // Where the cards went, which the ledger's built row could not say.
    b.push_str(&building(study, &who, &place, seats));

    // ---- development cards --------------------------------------------------
    b.push_str("<section>");
    b.push_str(&card_head(
        "Development cards",
        "Each column is how many of that card were drawn, with how many were \
         played in brackets. The two differ by what was still in hand at the \
         end: a card is drawn once and then either played or held, and a played \
         card never goes back to the deck (R-8.10). The victory point column \
         has no brackets, because a victory point card is never played: it \
         counts from the moment it is drawn (R-9.11).",
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
    let mut drawn = [0u32; 5];
    let mut played = [0u32; 5];
    for s in 0..seats {
        let mut cells = vec![placed(s, &who[s], place[s]), r.dev_bought[s].to_string()];
        for card in 0..5 {
            // Bought is played plus still held: a card is drawn once and then
            // either played or in the hand at the end, and nothing else can
            // happen to it (R-8.10).
            let held = study.dev_held[s][card];
            let out = r.dev_played[s][card];
            drawn[card] += out + held;
            played[card] += out;
            // A victory point card is never played: it counts the moment it is
            // drawn (R-9.11). Brackets saying "nought played" on every one of
            // them is a column of noughts that answers nothing.
            cells.push(if card == VICTORY_POINT {
                if out + held == 0 {
                    NONE.to_string()
                } else {
                    (out + held).to_string()
                }
            } else {
                bracketed(out + held, out)
            });
        }
        b.push_str(&row(&cells, false));
    }
    b.push_str("</tbody>");
    let mut foot = vec![
        "the deck".to_string(),
        r.dev_bought[..seats].iter().sum::<u32>().to_string(),
    ];
    foot.extend((0..5).map(|c| {
        if c == VICTORY_POINT {
            drawn[c].to_string()
        } else {
            bracketed(drawn[c], played[c])
        }
    }));
    b.push_str(&totals(&foot));
    b.push_str(T_CLOSE);
    b.push_str(&waiting(study));
    b.push_str("</section>");

    // ---- the board ----------------------------------------------------------
    b.push_str("<section>");
    b.push_str(&card_head(
        "Board",
        "What this board dealt, against what an average one deals. The discs \
         are a fixed set laid on a fixed set of hexes, so the average is not a \
         simulation: it is the mean pips of a disc times the hexes a resource \
         has. Every disc lands somewhere, so the pips always add to the same \
         total and the difference column always cancels. What it says is which \
         resource the deal favoured, before anybody placed anything.",
    ));
    let bd = &study.board;
    b.push_str(T_OPEN);
    b.push_str("<thead>");
    b.push_str(&head_row(&[
        ("", ""),
        (
            "hexes",
            "How many hexes produce this, which the tile set fixes.",
        ),
        ("pips", "Dots actually laid on them."),
        (
            "average board",
            "Those hexes times the mean pips of a disc, which is what a random \
             deal gives them over an unbounded number of boards.",
        ),
        (
            "difference",
            "How far this deal fell from that. It cancels across the column, \
             because a pip given to one resource was taken from another.",
        ),
    ]));
    b.push_str("</thead><tbody>");
    for res in 0..5 {
        let owed = bd.expected(res);
        let d = f64::from(bd.pips[res]) - owed;
        b.push_str(&row(
            &[
                format!("<span class=\"dot r{res}\"></span>{}", RESOURCE_NAMES[res]),
                bd.hexes[res].to_string(),
                bd.pips[res].to_string(),
                format!("{owed:.1}"),
                format!(
                    "<span class=\"{}\">{:+.1}</span>",
                    if d >= 0.0 { "up" } else { "down" },
                    d
                ),
            ],
            false,
        ));
    }
    b.push_str("</tbody>");
    // Whole numbers across this row, because it is exact: every hex times the
    // mean pips of a disc is every pip on the board, so the expectation is the
    // total and the difference is nought. A decimal point here would suggest a
    // rounding that is not happening.
    b.push_str(&totals(&[
        "the land".to_string(),
        bd.hexes.iter().sum::<u32>().to_string(),
        bd.pips.iter().sum::<u32>().to_string(),
        format!("{:.0}", (0..5).map(|r| bd.expected(r)).sum::<f64>()),
        "0".to_string(),
    ]));
    b.push_str(T_CLOSE);

    // And the same question asked of the coast: was a port worth building on?
    b.push_str(T_OPEN);
    b.push_str("<thead>");
    b.push_str(&head_row(&[
        ("", ""),
        ("spots", "Intersections carrying this port."),
        (
            "hexes",
            "Producing hexes those intersections touch, counted once per touch: \
             a hex two of them share is two, because either spot could take it.",
        ),
        ("pips", "Dots on those hexes, counted the same way."),
        (
            "a hex",
            "Pips over hexes, which is the figure to compare ports on. The \
             board's mean is 3.2, so a port above that sat on better land than \
             a random hex and one below it sat on worse.",
        ),
        (
            "average board",
            "Those hexes times the mean pips of a disc. It differs between \
             ports because their spots touch different numbers of hexes, which \
             is where the port sits on the coast rather than anything the dice \
             did: a port spot touches one or two land hexes depending on the \
             layout, and the layout is the same on every board.",
        ),
        (
            "difference",
            "How far this deal fell from that. Only this column is chance; the \
             expectation beside it is fixed by the geometry.",
        ),
    ]));
    b.push_str("</thead><tbody>");
    let (mut spots, mut touching, mut pips, mut owed_all) = (0, 0, 0, 0.0);
    for kind in 0..PORT_KINDS {
        let land = bd.ports[kind];
        if land.spots == 0 {
            continue;
        }
        let owed = bd.port_expected(kind);
        let d = f64::from(land.pips) - owed;
        spots += land.spots;
        touching += land.touching;
        pips += land.pips;
        owed_all += owed;
        b.push_str(&row(
            &[
                // The disc says which resource by its colour, as it does on the
                // board and in the opening, so the name beside it was saying
                // the same thing twice. It hangs off the disc for the reader
                // still learning the colours.
                match kind.checked_sub(1) {
                    None => "<span class=\"port any\" data-tip=\"three to one, any \
                             resource\">3:1</span>"
                        .to_string(),
                    Some(res) => format!(
                        "<span class=\"port r{res}\" data-tip=\"two to one, {}\">2:1</span>",
                        RESOURCE_NAMES[res]
                    ),
                },
                land.spots.to_string(),
                land.touching.to_string(),
                land.pips.to_string(),
                if land.touching == 0 {
                    NONE.to_string()
                } else {
                    format!("{:.1}", f64::from(land.pips) / f64::from(land.touching))
                },
                format!("{owed:.1}"),
                format!(
                    "<span class=\"{}\">{:+.1}</span>",
                    if d >= 0.0 { "up" } else { "down" },
                    d
                ),
            ],
            false,
        ));
    }
    b.push_str("</tbody>");
    let d = f64::from(pips) - owed_all;
    b.push_str(&totals(&[
        "the coast".to_string(),
        spots.to_string(),
        touching.to_string(),
        pips.to_string(),
        format!("{:.1}", f64::from(pips) / f64::from(touching)),
        format!("{owed_all:.1}"),
        format!(
            "<span class=\"{}\">{:+.1}</span>",
            if d >= 0.0 { "up" } else { "down" },
            d
        ),
    ]));
    b.push_str(T_CLOSE);
    b.push_str(&deal(study));
    b.push_str("</section>");

    // ---- the opening --------------------------------------------------------
    b.push_str("<section>");
    b.push_str(&card_head(
        "Opening",
        "What the first two settlements bought, before anybody had a turn, and \
         what the game turned it into.",
    ));
    b.push_str(T_OPEN);
    b.push_str("<thead>");
    b.push_str(&head_row(&[
        ("", ""),
        (
            "pips",
            "A row per resource, a hex per dot. The dots on every number the \
             two settlements touch are the standard measure of how much \
             production a placement buys, and split by resource they also say \
             what it buys. A row with nothing on it is a resource this opening \
             cannot produce at all.",
        ),
        (
            "per turn",
            "The same pips as cards per turn at fair odds, which is the unit \
             somebody plays in. A pip is a thirty-sixth of a card.",
        ),
        (
            "numbers",
            "Every number the placement sits on, drawn as the board draws them. \
             A number twice is two settlements on it.",
        ),
        (
            "coverage",
            "The chance a roll pays this placement anything at all. Pips say \
             how much an opening collects; this says how often. Eight pips on \
             one number and eight spread over three are the same production and \
             a very different game, and only this tells them apart.",
        ),
        (
            "ports",
            "Ports the two settlements sit on, at the rate they trade.",
        ),
    ]));
    b.push_str("</thead><tbody>");
    for s in 0..seats {
        let o = &study.opening[s];
        b.push_str(&row(
            &[
                placed(s, &who[s], place[s]),
                pip_rows(&o.pips),
                turn_rows(&o.per_turn),
                discs(&o.numbers),
                format!("{:.0}%", o.coverage * 100.0),
                port_marks(&o.ports),
            ],
            false,
        ));
    }
    b.push_str("</tbody>");
    // No totals row. Four openings' pips added together is a number about the
    // board rather than about anybody, and drawn as fifty hexes it is a picture
    // of nothing; numbers, coverage and ports are all per placement.
    b.push_str(T_CLOSE);
    b.push_str("</section>");

    // ---- the engine ---------------------------------------------------------
    b.push_str(&engine_card(study, &who, &place, seats));

    // ---- coverage -----------------------------------------------------------
    // How often the board paid, turn by turn, under the opening it started
    // from: the opening card says what a placement covered on the day it was
    // made, and this says what became of that over a hundred turns of building
    // and blockading.
    b.push_str(&reach(study, &who, &place, seats));

    // The corpus card lived here and does not any more: seat win rates are a
    // claim about many games, and a report on one game is the wrong place to
    // make it. It belongs on a page that reads the whole store, which is the
    // cumulative statistics work still to come.

    b.push_str("</main></body></html>");
    b
}

/// The page's own styles.
///
/// The same ink, paper and faces as the board, written out rather than shared:
/// the board's stylesheet is a game's worth of rules about cards and hexes, and
/// a document needs almost none of it.
pub(crate) const CSS: &str = "
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
  /* ---- the shell ----
     One gutter for the whole application: the distance from the window's edge to
     the mark, to the board's rails and to the content column, on every page. It
     is a token rather than a number in four places because it is the one
     measurement that has to agree across two stylesheets, and the board page
     declares the same one. */
  --gutter: clamp(16px, 2.2vw, 32px);
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
/* ---- the table these pages are laid on ----
   The board's own ground, brought here so the four screens are one place rather
   than a game with a warm grained table under it and two documents on flat
   cream. Two things: wide faint pools of colour, so the paper reads as depth
   instead of absence, and the grain itself.

   Six pools rather than three, and the second three are the first three again.
   Same colours, same technique, moved: warm high on the right, then amber low
   on the left, orange off the left edge at the middle, teal into the bottom
   right corner. Three of them all along the top meant the colour was a corner
   treatment, and everything below the first screenful was flat cream. A page
   with one card on it is mostly table, so the table has to hold the whole
   window rather than its top edge.

   Fixed to the viewport, so the percentages are of the window and not of the
   document: a short page and a long one are lit the same way, and scrolling
   moves the content across the light rather than dragging the light with it.

   The grain is baked into a data URI rather than run as a live `filter: url()`,
   which would be re-rasterized on every repaint; multiplied and pinned behind
   the flow, so the cards stay the smooth things laid on the rough one rather
   than being grained twice. Same noise, same numbers as the board's. */
body { margin: 0; color: var(--foreground);
       font: 16px/1.55 Figtree, system-ui, sans-serif;
       -webkit-font-smoothing: antialiased;
       min-height: 100vh;
       background:
         radial-gradient(1200px 620px at 85% -18%, rgba(232, 84, 47, .12), transparent 62%),
         radial-gradient(900px 520px at 96% 6%, rgba(245, 168, 28, .12), transparent 58%),
         radial-gradient(700px 520px at 62% -12%, rgba(49, 175, 201, .10), transparent 60%),
         radial-gradient(1000px 680px at 6% 104%, rgba(245, 168, 28, .11), transparent 60%),
         radial-gradient(820px 640px at -8% 58%, rgba(232, 84, 47, .09), transparent 58%),
         radial-gradient(760px 560px at 104% 92%, rgba(49, 175, 201, .09), transparent 60%),
         var(--background);
       background-attachment: fixed; }
body::after {
  content: ''; position: fixed; inset: 0; z-index: -1; pointer-events: none;
  background-image: url(\"data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='180' height='180'%3E%3Cfilter id='g' x='0' y='0' width='100%25' height='100%25'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.9' numOctaves='4' stitchTiles='stitch'/%3E%3CfeColorMatrix type='matrix' values='0 0 0 0 0.42 0 0 0 0 0.32 0 0 0 0 0.19 0.66 0 0 0 -0.2'/%3E%3C/filter%3E%3Crect width='180' height='180' filter='url(%23g)'/%3E%3C/svg%3E\");
  background-size: 180px 180px;
  mix-blend-mode: multiply; opacity: .55;
}
/* ---- the header ----
   The same header the board wears, in the same place: hard against the page's
   gutter, so the mark is the same distance from the same corner whichever screen
   you are on. It was briefly centred with the column beneath it, which lined it
   up with the heading and put it somewhere different from where the board keeps
   it, and being in the same place on every page is worth more than being level
   with one thing on two of them. The content column is centred underneath; the
   header belongs to the window, which is where a site's mark lives. */
header { padding: 1.1rem var(--gutter);
         display: flex; align-items: center; gap: 16px; flex-wrap: wrap; }
/* Line height 1, as the board sets it: left to the font's own metrics the mark
   sat three pixels lower here than there, which is exactly the kind of drift two
   stylesheets describing one header produce. */
.mark { font: 400 22px/1 Audiowide, system-ui, sans-serif; color: var(--primary);
        text-decoration: none; margin: 0; letter-spacing: .01em; }
/* Which build is serving this. Quiet enough to ignore and legible enough to
   read out, which is the whole specification: it exists so that the question a
   stale process makes somebody ask, whether they are even looking at the new
   code, is answerable by looking. */
header .build { font: 400 12px/1 Figtree, system-ui, sans-serif;
                color: var(--muted-foreground); opacity: .55;
                font-variant-numeric: tabular-nums; }
/* What this page is about, beside the mark rather than instead of it: the body
   face and a quieter colour, so the mark stays the mark. */
.gameName { font: 500 14px/1 Figtree, system-ui, sans-serif;
            color: var(--muted-foreground); letter-spacing: .01em;
            min-width: 0; overflow: hidden; text-overflow: ellipsis;
            white-space: nowrap; }
.gameName::before { content: ''; display: inline-block; width: 1px; height: 1em;
                    background: var(--border); margin-right: 12px;
                    vertical-align: -.15em; }
/* The ways out, held together in one group and pushed away from the mark as a
   group rather than each one being pushed off the last. */
.headLinks { margin-left: auto; display: flex; align-items: center; gap: 16px; }
.headLink { color: var(--muted-foreground); font: 500 13px/1 Figtree, system-ui, sans-serif;
            text-decoration: none; padding-bottom: 1px;
            border-bottom: 1px solid var(--border); }
.headLink:hover { color: var(--primary); border-color: var(--primary); }
/* Who is reading, beside the way to stop being them. Quiet: an account is not
   what this page is for, and a name in the corner that shouts is a name that
   reads as a notification. */
.headWho { color: var(--dim); font: 500 13px/1 Figtree, system-ui, sans-serif;
           max-width: 12rem; overflow: hidden; text-overflow: ellipsis;
           white-space: nowrap; }
/* A button that reads as the links beside it, because it does the same kind of
   thing and only needs to be a button so that pressing it is a POST. */
a.headWho { text-decoration: none; }
a.headWho:hover { color: var(--primary); }
.headOut { display: flex; margin: 0; }
.headOut .headLink { appearance: none; background: none; cursor: pointer;
                     border: 0; border-bottom: 1px solid var(--border);
                     padding: 0 0 1px; }
main { max-width: 62rem; margin: 0 auto; padding: 0 var(--gutter) 5rem;
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
section > p { margin: 0 0 1rem; }

/* ---- table ----
   In its own bordered box, headers muted and sentence-cased rather than
   shouted, rows that answer to the pointer. Numbers stay right-aligned: that
   is a column of figures, whatever the design language. */
.tw { border: 1px solid var(--border); border-radius: var(--radius-md);
      overflow-x: auto; }
/* A card with a second table, for the figures that belong to nobody's row. */
.tw + .tw { margin-top: 1rem; }
table { width: 100%; border-collapse: collapse; font-size: 14px;
        font-variant-numeric: tabular-nums; }
th, td { text-align: right; padding: .7em .9em;
         border-bottom: 1px solid var(--border); }
/* The label column never wraps: a name and its place badge belong on one
   line, and breaking them makes a row twice as tall for nothing. */
th:first-child, td:first-child { text-align: left; white-space: nowrap; }
thead th { font-weight: 500; color: var(--muted-foreground); white-space: nowrap; }
/* Which turns a column covers, under its name. */
.range { display: block; font-weight: 400; font-size: 11px; opacity: .75; }
tbody tr { transition: background .12s ease; }
tbody tr:hover { background: var(--muted); }
tbody tr:last-child td { border-bottom: 0; }
/* Fixed layout, so the eleven number columns are the same width and the bars
   above them are too. Left to itself the table sizes each column to its
   content, and a three-figure number is wider than a one-figure one, which put
   the bars out of step with each other and with the figures under them. */
.rolls { table-layout: fixed; }
.rolls th:first-child, .rolls td:first-child { width: 5.5rem; }
.rolls th, .rolls td { padding: .7em .3em; }

/* ---- the roll chart ----
   A row of the table, so a bar and its column are aligned by the table rather
   than by a number kept in step by hand. Bars are what was rolled; the mark
   across each is what a fair pair owed it. Both on one axis, the tallest of
   either. */
.chart td { padding: .9em .45em .35em; vertical-align: bottom; border-bottom: 0; }
.col { position: relative; height: 84px; display: flex; align-items: flex-end; }
.stem { width: 100%; min-height: 1px; background: var(--muted-foreground);
        border-radius: 3px 3px 0 0; }
.owed { position: absolute; left: -3px; right: -3px; height: 2px;
        margin-bottom: -1px; background: var(--primary); border-radius: 1px; }
/* Anything that explains itself on hover says so, quietly. */
thead th[data-tip], .card-head h2[data-tip] { cursor: help;
                  text-decoration: underline dotted #BFAF9C;
                  text-underline-offset: 4px; }
/* The totals, ruled off above and set in the ink of a figure that matters. */
tfoot td { border-top: 1px solid var(--border); border-bottom: 0;
           font-weight: 600; color: var(--foreground); }
tbody tr:last-child td { border-bottom: 0; }
tfoot tr:hover { background: transparent; }
/* What a thing was worth, beside how many of it there were. */
.worth { color: var(--muted-foreground); }

/* ---- tooltips ----
   Most of what this page has to say about itself is said on hover, and the
   browser's own tooltip is a grey system box, in a system font, at a size the
   page never uses: the one element on a carefully set report that belongs to
   no design at all. So the page draws its own, in the card's ink on the card's
   paper, at the family radius.

   It hangs off `data-tip` rather than `title` because the two cannot coexist:
   leave the title on and the native box opens over the drawn one a moment
   later. The cost is that a `title` is reachable by keyboard and this is not,
   which is a real loss and the reason nothing here is *only* in a tooltip.

   It opens downwards, since a header is the thing most often asked about and
   there is always table under it. A table sits in a box that scrolls sideways,
   and a box that scrolls on one axis clips on the other, so the box stops
   clipping for as long as a tooltip is open: a one-row table is shorter than
   its own explanation, and the alternative is a tooltip cut in half. */
/* The box itself, in ems throughout, because the same box is used inside the
   drawings where a pixel is whatever the drawing has been scaled to. */
[data-tip]::after, .tipin {
  display: block; width: max-content; max-width: 20em;
  padding: .55em .7em;
  background: var(--foreground); color: var(--card);
  border-radius: .65em;
  box-shadow: 0 .5em 1.5em rgba(51, 38, 27, .22);
  font-weight: 400; line-height: 1.45; text-align: left; text-transform: none;
  /* The figures a chart gives for a turn are one line each, and a line break
     in an explanation is a line break. */
  white-space: pre-line; }
[data-tip] { position: relative; }
/* Out of layout entirely rather than merely invisible. A hidden box is still a
   box: absolutely positioned inside a table that scrolls, it stretched the
   scrollable area, and a one-row table came out with a scrollbar and its own
   header scrolled out of sight. */
[data-tip]::after {
  content: attr(data-tip); display: none;
  position: absolute; z-index: 20; top: calc(100% + 7px); left: 0;
  font-size: 12.5px; font-family: Figtree, system-ui, sans-serif;
  pointer-events: none; }
[data-tip]:hover::after { display: block; }
/* The tooltip is wider than a number column, so near the right edge of a table
   it hangs from its own right and opens inwards instead of off the side. */
tr > :nth-last-child(-n+3)[data-tip]::after,
tr > :nth-last-child(-n+3) [data-tip]::after { left: auto; right: 0; }
/* The bottom row has nothing underneath it to open over, so it opens up. */
tfoot [data-tip]::after, tbody tr:last-child [data-tip]::after {
  top: auto; bottom: calc(100% + 7px); }
.tw:has([data-tip]:hover) { overflow: visible; }

/* The same box over a drawing. The drawing is scaled to whatever width the
   card gives it, so a tooltip is anchored at a percentage of that box and
   hangs off the corner the server picked, which is always the one that opens
   inwards. The layer takes no pointer events: the shapes underneath are what
   the reader is pointing at. */
.frame { position: relative; }
.over { position: absolute; inset: 0; pointer-events: none; }
.tipat { display: none; position: absolute; z-index: 20; }
.tipat.to-left { transform: translateX(-100%); }
.tipat.up { transform: translateY(-100%); }
.tipat.to-left.up { transform: translate(-100%, -100%); }
.tipin { font: 12.5px Figtree, system-ui, sans-serif; }
/* ---- the opening ----
   Pips as tiles, a row per resource, so a column of them lines up down the
   table and a resource nobody can produce reads as the gap it is. */
.tile { width: 13px; height: auto; overflow: visible; }
.tile .on.r0 { fill: #C0563B; } .tile .on.r1 { fill: #1F5E3A; }
.tile .on.r2 { fill: #8DBE4A; } .tile .on.r3 { fill: #E2A32B; }
.tile .on.r4 { fill: #5C6B78; }
.pips { display: inline-flex; flex-direction: column; gap: 2px; }
/* One height for both columns, so a resource's hexes and its rate sit on the
   same line. Left to themselves a row of tiles and a row of text are different
   heights and the two columns drift apart down the cell. */
.pip-row { display: flex; justify-content: flex-end; align-items: center;
           gap: 2px; height: 18px; }
.pips.rates .pip-row { font-variant-numeric: tabular-nums; font-size: 13px;
                       line-height: 18px; color: var(--muted-foreground); }
/* The placement's own total, ruled off above like a table's foot, since that is
   what it is: the column of five added up. */
.pips .sum { margin-top: 2px; padding-top: 3px; height: 21px;
             border-top: 1px solid var(--border);
             font: 600 13px/18px Figtree, system-ui, sans-serif;
             font-variant-numeric: tabular-nums; color: var(--foreground); }
/* Numbers and ports as the board draws them: a disc with the figure on it, and
   six and eight in the red everybody looks for. */
.discs { display: inline-flex; flex-wrap: wrap; gap: 3px; justify-content: flex-end; }
.disc, .port { display: inline-flex; align-items: center; justify-content: center;
               width: 26px; height: 26px; border-radius: 50%; background: #fff;
               font: 600 12px Figtree, system-ui, sans-serif; color: var(--foreground);
               font-variant-numeric: tabular-nums; }
.disc.hot { color: #C2492A; }
.port { font-size: 10px; font-weight: 700; color: #fff; }
.port.any { background: var(--muted); color: var(--muted-foreground);
            box-shadow: inset 0 0 0 1px var(--border); }
.port.r0 { background: #C0563B; } .port.r1 { background: #1F5E3A; }
.port.r2 { background: #8DBE4A; color: var(--foreground); }
.port.r3 { background: #E2A32B; color: var(--foreground); }
.port.r4 { background: #5C6B78; }

/* A subtotal inside the body, ruled off from what it adds up. */
tbody tr.sub td { font-weight: 600; color: var(--foreground);
                  border-top: 1px solid var(--border); }
tbody tr.sub:hover { background: transparent; }

/* ---- the steal flow ----
   Thieves down the left, victims down the right, a ribbon between each pair as
   thick as the cards that moved along it. Laid out on the server, since every
   position is a fraction of a total known the moment the game ends. */
.flow svg { display: block; width: 100%; height: auto; }
.flow .frame { margin: 0 0 1rem; }
.ribbon { opacity: .45; }
.ribbon:hover { opacity: .8; }
/* A name inside a drawing, which is the page's own markup in a foreignObject
   so it cannot drift from the same name in a table. */
/* The box is a fixed width hung off the anchor, so the text is what has to sit
   inside the drawing; `ink` is what a check can measure. */
/* The line height is the page's own, not a tighter one, because a place badge
   takes its height from the line it sits on and has to come out the same pill
   here as in a table. */
.name { position: absolute; display: flex; align-items: center; gap: .4em;
        font: 500 13px/1.55 Figtree, system-ui, sans-serif;
        color: var(--foreground); white-space: nowrap;
        transform: translateY(-50%); }
.name.to-end { transform: translate(-100%, -50%); }
.name.to-mid { transform: translate(-50%, -50%); }
/* The colour is already the node it labels; a dot beside it says it twice. */
.name .dot, .key .dot { display: none; }
.ribbon.f0, .chord.f0 { fill: var(--p0); } .ribbon.f1, .chord.f1 { fill: var(--p1); }
.ribbon.f2, .chord.f2 { fill: var(--p2); } .ribbon.f3, .chord.f3 { fill: var(--p3); }
.node.n0, .rim.n0 { fill: var(--p0); } .node.n1, .rim.n1 { fill: var(--p1); }
.node.n2, .rim.n2 { fill: var(--p2); } .node.n3, .rim.n3 { fill: var(--p3); }

/* ---- the production curves ----
   Five views in the page and a radio deciding which is on screen, so the
   switch needs no script. The inputs themselves are never seen: their labels
   are the control. */
.modes { display: flex; flex-wrap: wrap; gap: .4rem; margin: 0 0 1rem; }
.modes input { position: absolute; opacity: 0; pointer-events: none; }
.modes label { display: inline-flex; align-items: center; gap: .45em;
               cursor: pointer; padding: .3em .9em; font-size: 14px;
               border: 1px solid var(--border); border-radius: 999px;
               color: var(--muted-foreground); }
.modes label:hover { color: var(--foreground); background: var(--muted); }
.modes input:checked + label { background: var(--foreground);
                               border-color: var(--foreground);
                               color: var(--card); font-weight: 500; }
/* A seat's pill is the seat's colour once it is the one on screen, and the
   mark beside the name goes white so it still reads as a mark rather than
   disappearing into the pill it is on. */
.modes input:checked + label.seat { color: var(--primary-foreground); }
.modes input:checked + label.m0 { background: var(--p0); border-color: var(--p0); }
.modes input:checked + label.m1 { background: var(--p1); border-color: var(--p1); }
.modes input:checked + label.m2 { background: var(--p2); border-color: var(--p2); }
.modes input:checked + label.m3 { background: var(--p3); border-color: var(--p3); }
.modes input:checked + label .dot { background: var(--primary-foreground); }
.modes input:focus-visible + label { outline: 2px solid var(--primary);
                                     outline-offset: 2px; }
/* Every view is drawn; exactly one is shown. `nth-of-type` counts the views in
   the order the radios above them run, which is the order they are written. */
.views > .view { display: none; }
.modes:has(input:nth-of-type(1):checked) ~ .views > .view:nth-of-type(1),
.modes:has(input:nth-of-type(2):checked) ~ .views > .view:nth-of-type(2),
.modes:has(input:nth-of-type(3):checked) ~ .views > .view:nth-of-type(3),
.modes:has(input:nth-of-type(4):checked) ~ .views > .view:nth-of-type(4),
.modes:has(input:nth-of-type(5):checked) ~ .views > .view:nth-of-type(5) { display: block; }
.view svg { display: block; width: 100%; height: auto; }
.line { fill: none; stroke-width: 2; stroke-linejoin: round; stroke-linecap: round; }
/* What was owed, drawn as a claim rather than as a fact. */
.line.owed { stroke-dasharray: 4 4; stroke-width: 1.5; opacity: .75; }
.grid { stroke: var(--border); stroke-width: 1; }
/* Where the game ends. Drawn in the colour a win is written in, since it is the
   same fact: ten points. */
.finish { stroke: var(--primary); stroke-width: 1; opacity: .5;
          stroke-dasharray: 6 4; }
.finish-mark { fill: var(--primary); opacity: .85; font-weight: 500; }
.axis { font: 400 11px Figtree, system-ui, sans-serif; fill: var(--muted-foreground);
        text-anchor: end; }
.axis.start { text-anchor: start; }
.axis.mid { text-anchor: middle; }
.axis.unit { font-weight: 500; }
/* The legend is centred, and it is also the control: a checkbox a curve, so
   clicking a name takes its two lines off the chart. */
.key { display: flex; flex-wrap: wrap; justify-content: center;
       gap: .3rem .5rem; margin: .5rem 0 0; font-size: 13px; }
.key input { position: absolute; opacity: 0; pointer-events: none; }
.key label { display: inline-flex; align-items: center; gap: .45em;
             cursor: pointer; padding: .15em .55em; border-radius: 999px;
             color: var(--muted-foreground); }
.key label:hover { background: var(--muted); color: var(--foreground); }
/* A curve that has been switched off says so: its name greys and its swatch
   goes hollow, which is the same language the opening tiles use. */
.key input:not(:checked) + label { opacity: .45; }
.key input:not(:checked) + label .swatch { background: none;
                                           box-shadow: inset 0 0 0 1.5px currentColor; }
.key input:focus-visible + label { outline: 2px solid var(--primary);
                                   outline-offset: 1px; }
.swatch { width: .7em; height: .7em; border-radius: 2px; }
/* Which checkbox is off decides which pair of lines is hidden. Positional
   rather than by id, so one rule covers the same curve in every view. */
.view:has(.key input:nth-of-type(1):not(:checked)) .line.k1,
.view:has(.key input:nth-of-type(2):not(:checked)) .line.k2,
.view:has(.key input:nth-of-type(3):not(:checked)) .line.k3,
.view:has(.key input:nth-of-type(4):not(:checked)) .line.k4,
.view:has(.key input:nth-of-type(5):not(:checked)) .line.k5 { display: none; }
/* One slot per turn, over everything, carrying that turn's figures. */
/* One turn's worth of chart, over the drawing: the pointer's target, its own
   tooltip, and a guide down the chart drawn as its left edge. The layer above
   it takes no pointer events, so a slot has to ask for them back. */
.slot { position: absolute; pointer-events: auto; }
.slot::before { content: ''; position: absolute; left: 50%; top: 0; bottom: -1.2rem;
                border-left: 1px solid var(--muted-foreground); opacity: 0;
                transition: opacity .12s ease; }
.slot:hover::before { opacity: .55; }
/* The box hangs from the top of the chart at the turn it belongs to, and turns
   in the right half of the chart open leftwards. */
.slot::after { top: .4rem; left: 50%; }
.slot.to-left::after { left: auto; right: 50%; }
.tick { stroke: var(--border); stroke-width: 1; }
.f0, .swatch.f0 { stroke: var(--p0); background: var(--p0); }
.f1, .swatch.f1 { stroke: var(--p1); background: var(--p1); }
.f2, .swatch.f2 { stroke: var(--p2); background: var(--p2); }
.f3, .swatch.f3 { stroke: var(--p3); background: var(--p3); }
/* The five resources, in the colours their terrain wears on the board. The
   compound selectors are for the row marks, which carry `.dot` as well and
   would otherwise take its colour on equal specificity. */
.r0, .swatch.r0, .dot.r0 { stroke: #C0563B; background: #C0563B; }
.r1, .swatch.r1, .dot.r1 { stroke: #1F5E3A; background: #1F5E3A; }
.r2, .swatch.r2, .dot.r2 { stroke: #8DBE4A; background: #8DBE4A; }
.r3, .swatch.r3, .dot.r3 { stroke: #E2A32B; background: #E2A32B; }
.r4, .swatch.r4, .dot.r4 { stroke: #5C6B78; background: #5C6B78; }

/* ---- the trade ring ----
   A chord, because trading is symmetric: there is no side a trade goes from,
   and a sankey would invent a direction the game does not have. */
/* Over the table and across the whole card. Beside it, the circle was small
   and the table was squeezed out of its own columns; the card has one measure
   and both of them want all of it. */
.ring svg { display: block; width: 100%; height: auto; }
.ring .frame { margin: 0 0 1rem; }
/* ---- the timeline ----
   A lane a seat and a mark a thing, on the same turn axis as the chart above it.
   Events rather than quantities, so nothing is measured up the page and nothing
   pretends to be: the marks differ in shape and size rather than in height. */
.strip { position: relative; margin: 1rem 0 0; }
.strip svg { display: block; width: 100%; height: auto; overflow: visible; }
/* No inset: this drawing's turn axis has to sit under the chart above it, and a
   margin here would put every mark a few turns off. */
.lane { stroke: var(--border); stroke-width: 1; }
/* Prefixed names throughout, because `mark` is the header's wordmark and `tile`
   is the opening's hex, and an SVG rect takes CSS `width` and `height` over its
   own attributes: reusing `tile` flattened every diamond here to nothing. */
.beat-house, .beat-city { fill: currentColor; }
.beat-card { fill: var(--card); stroke: currentColor; stroke-width: 1.5; }
.beat-tile { fill: currentColor; stroke: var(--card); stroke-width: 1; }
.strip .beat.f0 { color: var(--p0); } .strip .beat.f1 { color: var(--p1); }
.strip .beat.f2 { color: var(--p2); } .strip .beat.f3 { color: var(--p3); }
/* A key to the four shapes, drawn as the shapes themselves rather than named. */
.key.shapes { color: var(--muted-foreground); }
.legend { display: inline-flex; align-items: center; gap: .4em; }
.legend .beat { display: inline-block; width: 9px; height: 9px; border-radius: 1.5px;
                background: var(--muted-foreground); }
.legend .beat-card { border-radius: 50%; background: var(--card);
                     box-shadow: inset 0 0 0 1.5px var(--muted-foreground); }
.legend .beat-house { width: 7px; height: 7px; }
.legend .beat-city { width: 11px; height: 11px; }
.legend .beat-tile { border-radius: 0; transform: rotate(45deg); }

/* ---- the trail ----
   The two halves of an economy against each other, a point a quarter, joined so
   a seat is a path with a direction rather than a dot. */
.trail { fill: none; stroke-width: 1.5; opacity: .55; stroke-dasharray: 3 3; }
.stop { fill: var(--card); stroke-width: 2; }
.stop.q0 { opacity: .45; } .stop.q1 { opacity: .6; } .stop.q2 { opacity: .8; }
.stop.last { fill: currentColor; }
.stop.f0 { fill: var(--card); color: var(--p0); }
.stop.f1 { fill: var(--card); color: var(--p1); }
.stop.f2 { fill: var(--card); color: var(--p2); }
.stop.f3 { fill: var(--card); color: var(--p3); }
.stop.last.f0 { fill: var(--p0); } .stop.last.f1 { fill: var(--p1); }
.stop.last.f2 { fill: var(--p2); } .stop.last.f3 { fill: var(--p3); }
.trails svg { display: block; width: 100%; height: auto; overflow: visible; }
.trails .frame { margin: 1rem 0 0; }
.chord { opacity: .4; }
.chord:hover { opacity: .75; }
.rim.supply { fill: var(--muted-foreground); }
.rim { stroke: var(--card); stroke-width: 1; }

/* ---- badge ---- */
.tag { display: inline-block; padding: .05em .45em; border-radius: var(--radius-sm);
       background: var(--primary); color: var(--primary-foreground);
       font-size: 12px; font-weight: 600; letter-spacing: .01em;
       vertical-align: 1px; }
/* Every place but first, which keeps the colour the win had. */
.tag.quiet { background: transparent; color: var(--muted-foreground);
             border: 1px solid var(--border); font-weight: 500; }
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

/* ---- the seat mark ----
   The colour a player played in, always immediately left of their name, so a
   row can be found by colour rather than by reading down the names. */
/* A card is not a seat and not a resource, so its mark is the ink of the page. */
.dot.dev { background: var(--muted-foreground); }
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
    use crate::store::{Chair, Setup, game_id};
    use carranta_core::state::TradeMode;

    /// One person and three bots, which is what a solo table is written down as.
    /// Names on this page come from the chairs, so a fixture without them is a
    /// game nobody was at.
    fn sat(name: &str) -> Setup {
        Setup {
            chairs: vec![
                Chair::person("egonkey000000000", name),
                Chair::bot(),
                Chair::bot(),
                Chair::bot(),
            ],
            ..Default::default()
        }
    }

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
            by: String::new(),
            dealt: seed,
            winner: s.winner(),
            setup: sat("Egon"),
            moves: s.moves().to_vec(),
            times: s.times().to_vec(),
        }
    }

    #[test]
    fn one_header_sits_above_every_screen_in_the_application() {
        // Four screens, two stylesheets, and the board page's copy is written by
        // hand. Same classes on both sides or the header is four headers that
        // look alike until one of them is edited.
        const PAGE: &str = include_str!("../assets/index.html");
        let report = masthead("", &[("/abcd-efgh-ijkl/", "The board")]);
        let home = masthead_home(&[("/lobby", "New game")], "");
        for html in [report.as_str(), home.as_str(), PAGE] {
            assert!(html.contains("<header>"), "a header");
            assert!(html.contains("class=\"mark\""), "one name for the mark");
            assert!(html.contains("class=\"headLinks\""));
            assert!(html.contains("class=\"headLink\""));
        }
        // The mark leads home from everywhere it is a link, and is the heading on
        // the one page it would otherwise link to.
        assert!(report.contains("<a class=\"mark\" href=\"/\">"));
        assert!(home.contains("<h1 class=\"mark\">"));
        assert!(!home.contains("<a class=\"mark\""), "no link to this page");
        // New game means the lobby, on every page that offers it, and it is a
        // link rather than a button on all of them.
        assert!(PAGE.contains("<a class=\"headLink\" href=\"/lobby\">New game</a>"));
        assert!(home.contains("href=\"/lobby\">New game</a>"));
        // A New game button would be that: the lobby has an address, so offering
        // it as anything but a link is a second way to reach one place. Leaving
        // a seat is a button because it is not somewhere to go.
        assert!(
            !PAGE.contains(">New game</button>"),
            "one label, one destination, one kind of element"
        );
        // And the rules the two stylesheets share are named the same in both.
        for rule in [".headLinks", ".headLink", ".gameName"] {
            assert!(CSS.contains(rule), "the report styles {rule}");
            assert!(PAGE.contains(rule), "the board styles {rule}");
        }
        // Both carry the tab icon, which only the board used to.
        assert!(PAGE.contains("rel=\"icon\""));
        assert!(ICON.contains("rel=\"icon\""));
    }

    #[test]
    fn every_screen_is_laid_on_the_same_shell() {
        // The three things that make four screens one place: where the header
        // sits, and the ground under it. All of it is declared twice, once per
        // stylesheet, so all of it can drift.
        const PAGE: &str = include_str!("../assets/index.html");
        // One gutter, and the header is positioned by it rather than by a number
        // of its own. That is what puts the mark the same distance from the same
        // corner on every screen.
        let gutter = "--gutter: clamp(16px, 2.2vw, 32px);";
        assert!(CSS.contains(gutter), "the report declares the gutter");
        assert!(PAGE.contains(gutter), "the board declares the same one");
        for sheet in [CSS, PAGE] {
            assert!(
                sheet.contains("padding: 1.1rem var(--gutter)"),
                "header inset"
            );
        }
        // The same table under all of them: six pools of colour and the grain.
        // Three warm and cool along the top, and the same three again low and
        // to the left, so the light crosses the window rather than sitting in
        // one corner with flat cream under it.
        for wash in [
            "radial-gradient(1200px 620px at 85% -18%, rgba(232, 84, 47, .12), transparent 62%)",
            "radial-gradient(900px 520px at 96% 6%, rgba(245, 168, 28, .12), transparent 58%)",
            "radial-gradient(700px 520px at 62% -12%, rgba(49, 175, 201, .10), transparent 60%)",
            "radial-gradient(1000px 680px at 6% 104%, rgba(245, 168, 28, .11), transparent 60%)",
            "radial-gradient(820px 640px at -8% 58%, rgba(232, 84, 47, .09), transparent 58%)",
            "radial-gradient(760px 560px at 104% 92%, rgba(49, 175, 201, .09), transparent 60%)",
        ] {
            assert!(CSS.contains(wash), "the report is laid on the same table");
            assert!(PAGE.contains(wash), "and so is the board");
        }
        for sheet in [CSS, PAGE] {
            assert!(
                sheet.contains("background-attachment: fixed"),
                "the pools are of the window, not of the document"
            );
            assert!(sheet.contains("feTurbulence"), "the grain");
            assert!(
                sheet.contains("background-size: 180px 180px"),
                "at one scale"
            );
            assert!(
                sheet.contains("mix-blend-mode: multiply; opacity: .55"),
                "one weight"
            );
        }
    }

    #[test]
    fn the_dock_scales_to_its_column_rather_than_wrapping() {
        // The rule the strip under the board lives by: shrink as far as it takes
        // to stay on one row and no further, and only wrap when shrinking more
        // would cost more than a second row does. Three things make that true and
        // all three are easy to undo by accident.
        const PAGE: &str = include_str!("../assets/index.html");
        // It is sized by the column it is in, not by the window. A `vw` here is
        // the bug this replaced: the rails stop growing at 400px and the window
        // does not, so the two stopped growing together.
        assert!(
            PAGE.contains("font-size: clamp(9px, 1.16cqw, 14px)"),
            "sized by cqw"
        );
        assert!(
            PAGE.contains("container-type: inline-size"),
            "and something has to be the container"
        );
        // Every measurement inside it is in `em`, so the strip is one shape at
        // one scale and its width is a fixed multiple of its type size. A `px`
        // gap or padding here breaks the arithmetic the coefficient rests on.
        assert!(PAGE.contains("gap: 1.25em;"), "the gap between groups");
        assert!(
            PAGE.contains("padding: 1em 1.1em;"),
            "and the strip's own inset"
        );
        // The push between the hand and the controls costs no width. It was an
        // empty group, which cost a gap on each side of a thing with none.
        assert!(PAGE.contains(".dockGroup.pushed { margin-left: auto; }"));
        assert!(
            !PAGE.contains("dockGroup spacer"),
            "no empty group paying for two gaps"
        );
    }

    #[test]
    fn the_page_says_what_the_game_did() {
        let history: Vec<Saved> = (0..3u64).map(played).collect();
        let s = study(&history[1], &history).expect("it studies");
        let html = page(&history[1], &s, "");
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.ends_with("</html>"));
        // Everybody at the table is named, and the board is a click away. The
        // bots are named in the order they are sitting rather than by seat
        // number, so a table with one person at it reads Ada, Bram, Ines
        // wherever the draw happened to put the person.
        for name in ["Egon", "Ada", "Bram", "Ines"] {
            assert!(html.contains(name), "{name} is on the page");
        }
        assert!(html.contains(&format!("/{}/", history[1].id)));
        // The result table is a decomposition of the score, so it names the
        // five things that score and not the things that do not.
        assert!(html.contains("victory points"));
        assert!(html.contains("largest militia"));
        // In the result card, which is the one that decomposes a score. Roads are
        // a column on the building card, where what they score is not the point.
        let result = html
            .split("</section>")
            .next()
            .expect("the result card is the first section");
        assert!(!result.contains(">roads<"), "roads score nothing (R-11.3)");
        // The sections that were asked for, by their headings.
        for heading in [
            "Result",
            "Turns",
            "Ratings",
            "Dice",
            "Production",
            "Deviation",
            "Militia",
            "Trades",
            "Development cards",
            "Board",
            "Opening",
            "Engine",
            "Coverage",
        ] {
            assert!(html.contains(heading), "{heading} is a section");
        }
        // And the thing §10.1 forbids is not on it.
        assert!(!html.contains("p-value ="), "no significance claim");
        assert!(html.contains("bits"), "an effect size instead");
    }

    #[test]
    fn the_timeline_stands_under_the_score_it_explains() {
        let g = played(4);
        let s = study(&g, std::slice::from_ref(&g)).expect("it studies");
        let html = page(&g, &s, "");
        // The strip is read against the chart above it, so the two turn axes have
        // to be the same axis. Both drawings are laid out in the same coordinates
        // and given the same box, so their tick positions come out identical, and
        // this is what fails if either drifts.
        let card = html
            .split("</section>")
            .next()
            .expect("the result card is the first section");
        let ticks = |from: &str| -> Vec<String> {
            from.match_indices("class=\"axis mid\" x=\"")
                .map(|(at, key)| {
                    let rest = &from[at + key.len()..];
                    rest[..rest.find('"').unwrap_or(0)].to_string()
                })
                .collect()
        };
        let (chart, strip) = card
            .split_once("class=\"strip\"")
            .expect("the card carries both drawings");
        let (above, below) = (ticks(chart), ticks(strip));
        assert!(above.len() >= 4, "the chart is labelled along its length");
        assert_eq!(above, below, "the two axes are one axis");
        // And the marks are prefixed, because the plain names belong to the
        // header's wordmark and to the opening's hexes, and reusing either
        // silently flattens every mark on the strip.
        assert!(strip.contains("class=\"beat beat-"), "prefixed marks");
        for taken in ["class=\"mark beat", "beat tile\""] {
            assert!(!html.contains(taken), "{taken} collides with something");
        }
    }

    #[test]
    fn the_deviation_columns_add_across_to_what_arrived() {
        let g = played(4);
        let s = study(&g, std::slice::from_ref(&g)).expect("it studies");
        for p in 0..s.report.players as usize {
            let d = s.production.decompose(p);
            // The §10.2 identity, which is what makes the card a decomposition
            // rather than four numbers printed beside each other.
            assert!(d.residual().abs() < 1e-9, "seat {p}: {d:?}");
        }
        let html = page(&g, &s, "");
        assert!(html.contains("sd)"), "the dice column carries its own z");
        assert!(
            html.contains("only the dice column is chance"),
            "and the card says which column is which"
        );
    }

    #[test]
    fn every_tooltip_on_the_page_is_one_the_page_drew() {
        let g = played(4);
        let s = study(&g, std::slice::from_ref(&g)).expect("it studies");
        let html = page(&g, &s, "");
        // Not one native tooltip left: a `title` attribute anywhere, or a
        // `<title>` inside a drawing, is the browser's grey box coming back.
        assert!(!html.contains("title=\""), "no native tooltip attributes");
        assert_eq!(
            html.matches("<title>").count(),
            1,
            "the only <title> is the document's own"
        );
        // The drawings carry the page's own box instead, laid over them.
        let tips = html.matches("class=\"tipat t").count();
        assert!(tips > 20, "the drawings explain themselves too");
        // And every one of those is reachable: a shape to hover, and a rule
        // tying the two together, since without a script nothing else can.
        for i in 0..tips {
            if !html.contains(&format!(" t{i}\"")) && !html.contains(&format!(" t{i} ")) {
                continue;
            }
            assert!(
                html.contains(&format!(":has(.k{i}:hover) .t{i}")),
                "tooltip {i} is tied to its shape"
            );
        }
        assert_eq!(
            html.matches(":hover) .t").count(),
            tips,
            "one rule a tooltip, and no rule without one"
        );
    }

    #[test]
    fn a_scoring_column_says_how_many_and_what_they_were_worth() {
        let g = played(2);
        let s = study(&g, std::slice::from_ref(&g)).expect("it studies");
        let html = page(&g, &s, "");
        // Every column of the result table carries its own rule rather than a
        // paragraph under the table carrying all five.
        assert!(
            html.contains("<th data-tip="),
            "the columns explain themselves"
        );
        // And the page draws its own tooltip rather than leaving it to the
        // browser's grey system box, which belongs to no design at all.
        assert!(!html.contains("<th title="), "not the native tooltip");
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
        assert_eq!(scored(crate::analysis::Scored::default()), "");
    }

    #[test]
    fn the_turns_are_a_table_and_nothing_else() {
        let g = played(4);
        let s = study(&g, std::slice::from_ref(&g)).expect("it studies");
        let html = page(&g, &s, "");
        // The bar is gone, and with it every trace of how it was drawn.
        for gone in ["class=\"seg", "class=\"bar\"", "flex-grow"] {
            assert!(!html.contains(gone), "{gone} went with the bar");
        }
        // A game played through a session carries its clock, so the column is
        // there and the totals row adds up to the whole of it.
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
    fn a_chart_can_be_read_and_taken_apart() {
        let g = played(4);
        let s = study(&g, std::slice::from_ref(&g)).expect("it studies");
        let html = page(&g, &s, "");
        let turns = s.series.turns();
        // Coverage is sampled with production, so the two charts have the same
        // turns under them and a slot means the same thing on both.
        assert_eq!(s.cover.turns(), turns, "one clock for both charts");
        // A slot per turn in every production view, and one more chart's worth
        // each for the score, the engine and coverage.
        assert_eq!(
            html.matches("class=\"slot").count(),
            turns * (4 + s.report.players as usize)
        );
        // The score chart ends where the result table says it should.
        assert_eq!(s.score.len(), turns, "one clock for every chart");
        for p in 0..s.report.players as usize {
            assert_eq!(
                s.score[turns - 1][p],
                s.points[p].total(),
                "the last point is the result"
            );
        }
        assert!(html.contains(&format!("Turn {turns}")));
        assert!(
            html.contains(", expected "),
            "a slot says the expectation too"
        );
        // A checkbox per curve, checked, so every line starts on the chart.
        let seats = s.report.players as usize;
        let boxes = html.matches("checkbox\" id=\"k").count();
        // One a seat in the first view, then five a seat in their own view.
        assert_eq!(boxes, seats + seats * RESOURCE_NAMES.len());
        // And the coverage chart has the same legend, a seat a line.
        assert_eq!(html.matches("checkbox\" id=\"cv-").count(), seats);
        // And each curve's two lines carry the class its checkbox switches.
        for k in 1..=RESOURCE_NAMES.len() {
            assert!(html.contains(&format!("line k{k} ")), "k{k} is drawn");
        }
        // The turn axis is labelled along its length, not just at its ends.
        assert!(html.matches("class=\"tick\"").count() >= 4 * (4 + s.report.players as usize));
    }

    #[test]
    fn each_view_carries_its_own_table() {
        let g = played(5);
        let s = study(&g, std::slice::from_ref(&g)).expect("it studies");
        let html = page(&g, &s, "");
        let seats = s.report.players as usize;
        // One table a view, inside the view, so it switches with the chart.
        let views = &html[html.find("class=\"views\"").expect("the views")..];
        let views = &views[..views.find("</section>").unwrap()];
        assert_eq!(views.matches("<tfoot>").count(), 1 + seats);
        // The first is every seat against every resource; the rest are one
        // seat's own, a resource a row.
        assert!(views.contains("the board"), "the seats against each other");
        assert_eq!(views.matches("all of it").count(), seats);
        assert!(views.contains(">difference<") && views.contains(">share<"));
        // A seat's rows add across to their total in both.
        let last = s.series.turns() - 1;
        for seat in 0..seats {
            let total: u32 = s.series.actual[last][seat].iter().sum();
            assert!(views.contains(&format!("<td>{total}</td>")), "seat {seat}");
        }
    }

    #[test]
    fn a_seat_wears_its_colour_left_of_its_name_in_every_table() {
        let history: Vec<Saved> = (10..12u64).map(played).collect();
        let s = study(&history[1], &history).expect("it studies");
        let html = page(&history[1], &s, "");
        // Every table on the page names seats down its first column, so every
        // table has as many marks as it has seats, and each sits immediately
        // before the name rather than after it.
        let seats = s.report.players as usize;
        assert!(
            html.matches("class=\"dot s").count() >= seats * 6,
            "six tables' worth of seats are marked"
        );
        for (seat, name) in names(&history[1], seats).iter().enumerate() {
            assert!(
                html.contains(&format!("<span class=\"dot s{seat}\"></span>{name}")),
                "{name} wears seat {seat}'s colour, on the left"
            );
        }
    }

    #[test]
    fn the_rolls_are_drawn_against_what_was_owed() {
        let g = played(3);
        let s = study(&g, std::slice::from_ref(&g)).expect("it studies");
        let html = page(&g, &s, "");
        let r = &s.report;
        // A bar and a mark for each of the eleven numbers, and no total column
        // on the table under them.
        assert_eq!(html.matches("class=\"stem\"").count(), 11);
        // `owed`, not `mark`: `.mark` is the header wordmark, and a second rule
        // of that name was quietly absolutely-positioning it.
        assert_eq!(html.matches("class=\"owed\"").count(), 11);
        assert_eq!(html.matches("class=\"mark\"").count(), 1, "the wordmark");
        let dice = &html[html.find("class=\"rolls\"").expect("the histogram")..];
        assert!(
            !dice[..dice.find("</table>").unwrap()].contains(">total<"),
            "the histogram has no total column"
        );
        // Both scaled to the same axis, so the tallest thing on it is full
        // height and everything else is its true fraction of that.
        let total: u32 = r.rolls.iter().sum();
        let expect = |n: u32| total as f64 * (6 - (n as i32 - 7).abs()) as f64 / 36.0;
        let tallest = (2..=12u32)
            .map(|n| expect(n).max(f64::from(r.rolls[n as usize - 2])))
            .fold(0.0, f64::max);
        assert!(tallest > 0.0);
        assert!(
            html.contains("height:100.0%"),
            "the tallest bar or mark is full"
        );
        for n in 2..=12u32 {
            let got = f64::from(r.rolls[n as usize - 2]);
            assert!(
                html.contains(&format!("height:{:.1}%", 100.0 * got / tallest)),
                "{n} is drawn at its true height"
            );
            assert!(
                html.contains(&format!("bottom:{:.1}%", 100.0 * expect(n) / tallest)),
                "{n}'s mark sits at what a fair pair owed it"
            );
        }
    }

    #[test]
    fn nothing_is_written_as_nothing() {
        let g = played(4);
        let s = study(&g, std::slice::from_ref(&g)).expect("it studies");
        let html = page(&g, &s, "");
        // No stand-in mark anywhere: a column with no value is blank, and the
        // blank already says it.
        assert!(
            !html.contains("&middot;"),
            "no dots standing in for absence"
        );
        // Which means the cells that have no total really are empty.
        assert!(html.contains("<td></td>"));
    }

    #[test]
    fn a_total_is_shown_only_where_a_column_has_one() {
        let g = played(6);
        let s = study(&g, std::slice::from_ref(&g)).expect("it studies");
        let html = page(&g, &s, "");
        // Every table that claims a total has one row of them and no more.
        assert_eq!(
            html.matches("<tfoot>").count(),
            html.matches("</tfoot>").count()
        );
        assert!(html.matches("<tfoot>").count() >= 4, "the summable tables");
        // The turns add across: the seats' turns are the game's turns.
        assert!(html.contains(&format!("<td>{}</td>", s.turns.len())));
        // A figure that does not belong to the arithmetic around it says so
        // rather than being quietly left out of it.
        assert!(
            html.contains("Not part of the ledger's arithmetic"),
            "the peak says why it is not in the sums"
        );
    }

    #[test]
    fn every_long_explanation_is_a_tooltip_now() {
        // Two games, so the dice card has something to compare against.
        let history: Vec<Saved> = (7..9u64).map(played).collect();
        let s = study(&history[1], &history).expect("it studies");
        let html = page(&history[1], &s, "");
        // Seat win rates are a claim about many games, so they are not on a
        // report about one, whatever the corpus behind it.
        assert!(!html.contains("Across every game here"), "not on this page");
        assert!(!html.contains("win rate"));
        // The paragraphs that used to sit under the tables are gone, and what
        // they said is on the card or the column it was about.
        for gone in [
            "dots on every number",
            "never goes back to the deck",
            "Counted for both sides",
            "No p-value, deliberately",
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
        // In the chair, which is where a name on this page comes from: it is
        // what its owner typed into their own seat and is somebody else's text.
        g.setup = sat("<script>alert(1)</script>");
        let s = study(&g, std::slice::from_ref(&g)).expect("it studies");
        let html = page(&g, &s, "");
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
        let html = page(&only, &s, "");
        // Nothing to compare with, so the percentile is blank and the count of
        // games it would have been drawn from is nought. Withheld rather than
        // guessed: a percentile of one game is not a percentile.
        assert_eq!(s.dice_percentile, None);
        assert_eq!(s.corpus_games, 0);
        assert!(html.contains("<td>0</td>"), "no games compared");
        assert!(html.contains("Blank until there is a second game to stand"));
        // The deviation is legible without knowing what a bit is.
        assert!(
            html.contains("out of place"),
            "and in rolls as well as bits"
        );
        // And the card that only exists once there is a corpus does not.
        assert!(!html.contains("Across every game here"));
    }
}
