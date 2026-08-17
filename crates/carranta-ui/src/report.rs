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

use crate::analysis::{Study, Trades, seat_name};
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
        "<div class=\"card-head\"><h2 title=\"{}\">{title}</h2></div>",
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
            let _ = write!(out, "<th title=\"{}\">{label}</th>", esc(why));
        }
    }
    out.push_str("</tr>");
    out
}

/// The five resources as board tiles, the missing ones left hollow.
///
/// A hex apiece, in the colour its terrain wears on the board, because "5 of 5"
/// says how many without saying which, and which is the question: a placement
/// short of ore plays differently from one short of brick.
fn tiles(touched: &[bool; 5]) -> String {
    // A flat-top hex, the shape the board is made of, at a size that sits on a
    // line of text.
    const HEX: &str = "M9 0 L18 5.2 L18 15.6 L9 20.8 L0 15.6 L0 5.2 Z";
    let mut out = String::from("<span class=\"tiles\">");
    for (r, on) in touched.iter().enumerate() {
        let _ = write!(
            out,
            "<svg viewBox=\"-1 -1 20 22.8\" class=\"tile\" role=\"img\" \
             aria-label=\"{name}{miss}\"><title>{name}{miss}</title>\
             <path class=\"{state} r{r}\" d=\"{HEX}\"/></svg>",
            name = RESOURCE_NAMES[r],
            miss = if *on { "" } else { ", not touched" },
            state = if *on { "on" } else { "off" },
        );
    }
    out.push_str("</span>");
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
fn names(saved: &Saved, seats: usize) -> Vec<String> {
    (0..seats).map(|s| seat_name(s, &saved.name)).collect()
}

/// A name with where it finished, for places a badge cannot go.
fn label(name: &str, place: Option<usize>) -> String {
    match place {
        Some(n) => format!("{name} {}", ordinal(n)),
        None => name.to_string(),
    }
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

    let mut b = format!(
        "<div class=\"flow\"><svg viewBox=\"0 0 {W} {h}\" role=\"img\" \
         aria-label=\"Who took cards from whom\">",
        h = H + TOP * 2.0
    );

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
            let _ = write!(
                b,
                "<path class=\"ribbon f{thief}\" d=\"M{x0} {a} C{bmid} {a} {cmid} {c} {x1} {c} \
                 L{x1} {c2} C{cmid} {c2} {bmid} {a2} {x0} {a2} Z\"><title>{title}</title></path>",
                x0 = LEFT + NODE,
                x1 = RIGHT - NODE,
                a2 = a + thick,
                c2 = c + thick,
                title = esc(&format!("{} took {n} from {}", who[thief], who[victim])),
            );
            out_at[thief] += thick;
            in_at[victim] += thick;
        }
    }

    // The blocks, and a name beside each.
    for s in 0..seats {
        if took[s] > 0 {
            let _ = write!(
                b,
                "<rect class=\"node n{s}\" x=\"{x}\" y=\"{y}\" width=\"{NODE}\" \
                 height=\"{h}\" rx=\"3\"/>\
                 <text class=\"who end\" x=\"{tx}\" y=\"{ty}\">{name}</text>",
                x = LEFT,
                y = from[s].0,
                h = from[s].1 - from[s].0,
                tx = LEFT - 10.0,
                ty = (from[s].0 + from[s].1) / 2.0 + 5.0,
                name = esc(&label(&who[s], place[s])),
            );
        }
        if lost[s] > 0 {
            let _ = write!(
                b,
                "<rect class=\"node n{s}\" x=\"{x}\" y=\"{y}\" width=\"{NODE}\" \
                 height=\"{h}\" rx=\"3\"/>\
                 <text class=\"who\" x=\"{tx}\" y=\"{ty}\">{name}</text>",
                x = RIGHT - NODE,
                y = to[s].0,
                h = to[s].1 - to[s].0,
                tx = RIGHT + 10.0,
                ty = (to[s].0 + to[s].1) / 2.0 + 5.0,
                name = esc(&label(&who[s], place[s])),
            );
        }
    }
    b.push_str("</svg></div>");
    b
}

/// Production against expectation, turn by turn, with a switch above it.
///
/// The default is every seat at once: a solid line for what each collected and
/// a dotted one, in the same colour, for what the pips owed them. Pick a seat
/// and the same chart is drawn a resource at a time, which is the only way to
/// see *which* card a placement was short of.
///
/// The switch is five radio inputs and a sibling selector, so the page still
/// carries no script. Every view is drawn into the page and CSS decides which
/// one is visible: five charts of a few hundred points each is a smaller thing
/// to ship than a script that would build one.
fn curves(study: &Study, who: &[String], place: &[Option<usize>], seats: usize) -> String {
    let s = &study.series;
    if s.turns() < 2 {
        return String::new();
    }

    let mut b = String::from("<section>");
    b.push_str(&card_head(
        "Production per turn",
        "Solid is what the board actually paid; dotted is what the pips through \
         the buildings standing at each roll owed at fair odds. Both running \
         totals, so each line only climbs and the gap between a pair is \
         everything that has happened to that seat so far. The robber is \
         ignored in the expectation, so a seat under blockade watches its solid \
         line fall away from its dotted one, which is what a blockade costs.",
    ));

    // The switch. One radio per view, named together so they are one control.
    b.push_str("<div class=\"modes\">");
    for (i, label) in std::iter::once("everybody".to_string())
        .chain((0..seats).map(|p| esc(&who[p])))
        .enumerate()
    {
        let _ = write!(
            b,
            "<input type=\"radio\" name=\"view\" id=\"view{i}\"{on}>\
             <label for=\"view{i}\">{label}</label>",
            on = if i == 0 { " checked" } else { "" },
        );
    }
    b.push_str("</div><div class=\"views\">");

    // Everybody: two lines a seat, in the seat's colour.
    let ceiling = s.ceiling(seats);
    let mut lines = Vec::new();
    for p in 0..seats {
        lines.push((
            format!("f{p}"),
            true,
            label(&who[p], place[p]),
            (0..s.turns())
                .map(|i| f64::from(s.actual[i][p].iter().sum::<u32>()))
                .collect::<Vec<f64>>(),
        ));
        lines.push((
            format!("f{p}"),
            false,
            format!("{} expected", label(&who[p], place[p])),
            (0..s.turns())
                .map(|i| s.expected[i][p].iter().sum::<f64>())
                .collect(),
        ));
    }
    b.push_str(&plot(&lines, ceiling, s.turns(), "cards"));

    // And one view a seat, drawn a resource at a time.
    for p in 0..seats {
        let ceiling = s.ceiling_of(p);
        let mut lines = Vec::new();
        for (res, name) in RESOURCE_NAMES.iter().enumerate() {
            lines.push((
                format!("r{res}"),
                true,
                (*name).to_string(),
                (0..s.turns())
                    .map(|i| f64::from(s.actual[i][p][res]))
                    .collect::<Vec<f64>>(),
            ));
            lines.push((
                format!("r{res}"),
                false,
                format!("{name} expected"),
                (0..s.turns()).map(|i| s.expected[i][p][res]).collect(),
            ));
        }
        b.push_str(&plot(&lines, ceiling, s.turns(), "cards"));
    }
    b.push_str("</div></section>");
    b
}

/// One chart: a polyline per series, on one axis.
///
/// `lines` is (colour class, solid, name, points). Every series shares the
/// ceiling, or the gap between a pair of them would be a picture of two scales
/// rather than of a difference.
fn plot(
    lines: &[(String, bool, String, Vec<f64>)],
    ceiling: f64,
    turns: usize,
    unit: &str,
) -> String {
    const W: f64 = 720.0;
    const H: f64 = 260.0;
    const PAD: f64 = 34.0;
    let top = if ceiling > 0.0 { ceiling } else { 1.0 };
    let x = |i: usize| PAD + (W - PAD * 2.0) * i as f64 / (turns - 1).max(1) as f64;
    let y = |v: f64| H - PAD - (H - PAD * 2.0) * v / top;

    let mut b = format!(
        "<div class=\"view\"><svg viewBox=\"0 0 {W} {H}\" role=\"img\" \
         aria-label=\"Cumulative production against expectation\">"
    );
    // Four gridlines and their values, so the height of a line can be read.
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
    let _ = write!(
        b,
        "<text class=\"axis start\" x=\"{PAD}\" y=\"{ty}\">turn 1</text>\
         <text class=\"axis\" x=\"{r}\" y=\"{ty}\">turn {turns}</text>\
         <text class=\"axis start unit\" x=\"{PAD}\" y=\"14\">{unit}</text>",
        r = W - PAD,
        ty = H - 10.0,
    );
    for (colour, solid, name, points) in lines {
        let path: String = points
            .iter()
            .enumerate()
            .map(|(i, v)| format!("{:.1},{:.1}", x(i), y(*v)))
            .collect::<Vec<_>>()
            .join(" ");
        let _ = write!(
            b,
            "<polyline class=\"line {colour}{dash}\" points=\"{path}\">\
             <title>{name}</title></polyline>",
            dash = if *solid { "" } else { " owed" },
            name = esc(name),
        );
    }
    b.push_str("</svg><div class=\"key\">");
    // A key, since eight lines in four colours need saying once.
    for (colour, solid, name, _) in lines {
        if !solid {
            continue;
        }
        let _ = write!(
            b,
            "<span class=\"pair\"><span class=\"swatch {colour}\"></span>{}</span>",
            esc(name)
        );
    }
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
        "<div class=\"ring\"><svg viewBox=\"0 0 {W} {H}\" role=\"img\" \
         aria-label=\"Who traded with whom\">"
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
        let _ = write!(
            b,
            "<path class=\"chord f{seat}\" d=\"M{x0:.1} {y0:.1} A{R} {R} 0 0 1 {x1:.1} {y1:.1} \
             Q{mx} {my} {u1:.1} {v1:.1} A{R} {R} 0 0 1 {u0:.1} {v0:.1} Q{mx} {my} {x0:.1} {y0:.1} Z\">\
             <title>{}</title></path>",
            esc(&format!(
                "Turn {}: {} gave {}, took {}, {counter}",
                d.turn,
                who[d.seat],
                hand_text(&d.gave),
                hand_text(&d.took),
            )),
            seat = d.seat.min(MAX_PLAYERS - 1),
        );
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
        let _ = write!(
            b,
            "<path class=\"rim {class}\" d=\"M{x0:.1} {y0:.1} A{R} {R} 0 0 1 {x1:.1} {y1:.1} \
             L{x2:.1} {y2:.1} A{r2} {r2} 0 0 0 {x3:.1} {y3:.1} Z\"><title>{}</title></path>",
            esc(&format!("{} trades", tr.ends(*party))),
            r2 = R + BAND,
        );
        let centre = (a0 + a1) / 2.0;
        let (tx, ty) = point(centre, R + BAND + 14.0);
        let name = match *party {
            w if w == Trades::BANK => "the bank".to_string(),
            w if w == Trades::PORT => "ports".to_string(),
            w => label(&who[w], place[w]),
        };
        let _ = write!(
            b,
            "<text class=\"who {anchor}\" x=\"{tx:.1}\" y=\"{ty:.1}\">{}</text>",
            esc(&name),
            // A name runs outwards from the rim, so which way it is anchored
            // depends on where round the circle it sits. At the top and bottom
            // neither end is outwards, and it is centred.
            anchor = match centre.cos() {
                c if c < -0.3 => "end",
                c if c > 0.3 => "",
                _ => "mid",
            },
        );
    }
    b.push_str("</svg></div>");
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
    b.push_str("</section>");
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
    let place = places(r, seats);
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
                "<td><div class=\"col\" title=\"{n}: rolled {got}, expected {e}\">\
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
            "deviation",
            "How far these rolls fell from a fair pair, as a KL divergence in \
             bits. An effect size, which is the figure §10.1 asks for in place \
             of a significance claim.",
        ),
        (
            "percentile",
            "How much of the recorded corpus these dice deviated further than. \
             Blank until there is a second finished game to compare with, since \
             a percentile of one game is not a percentile.",
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
            format!("{:.3} bits", study.dice.kl_bits),
            match study.dice_percentile {
                // "More than 100% of five games" is not a figure anybody wants
                // to read. At the top of the range the answer is all of them.
                Some(p) if p >= 99.5 => "every one".to_string(),
                Some(p) if p < 0.5 => "none".to_string(),
                Some(p) => format!("{p:.0}%"),
                None => NONE.to_string(),
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
        let mut cells = vec![format!("<span title=\"{}\">{name}</span>", esc(why))];
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
    b.push_str("</tbody>");
    b.push_str(T_CLOSE);

    b.push_str("</section>");

    // ---- production per turn -------------------------------------------------
    b.push_str(&curves(study, &who, &place, seats));

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
    b.push_str("<div class=\"market\">");
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
    b.push_str("</div></section>");

    // ---- development cards --------------------------------------------------
    b.push_str("<section>");
    b.push_str(&card_head(
        "Development cards",
        "Each column is how many of that card were drawn, with how many were \
         played in brackets. The two differ by what was still in hand at the \
         end: a card is drawn once and then either played or held, and a played \
         card never goes back to the deck (R-8.10). A victory point card is \
         never played at all (R-9.11), so its brackets are always empty and the \
         column is kept so the five read in the order the cards do.",
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
            cells.push(bracketed(out + held, out));
        }
        b.push_str(&row(&cells, false));
    }
    b.push_str("</tbody>");
    let mut foot = vec![
        "the deck".to_string(),
        r.dev_bought[..seats].iter().sum::<u32>().to_string(),
    ];
    foot.extend((0..5).map(|c| bracketed(drawn[c], played[c])));
    b.push_str(&totals(&foot));
    b.push_str(T_CLOSE);
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
            "The dots on every number the starting settlements touch, which is \
             the standard measure of how much production a placement buys.",
        ),
        (
            "resources",
            "Which of the five the opening touches at all, in the colours their \
             terrain wears on the board, with the ones it misses left hollow. A \
             placement can be rich in pips and still be missing something it \
             will need. Per placement, so it has no total.",
        ),
        ("ports", "Ports the starting settlements sit on."),
        (
            "biggest hand",
            "The most cards this seat ever held at once, anywhere in the game. \
             Not an opening figure, and here because it is the other half of \
             the same question: what the placement turned into. A maximum, so \
             it has no total.",
        ),
    ]));
    b.push_str("</thead><tbody>");
    for s in 0..seats {
        b.push_str(&row(
            &[
                placed(s, &who[s], place[s]),
                r.opening[s].pips.to_string(),
                tiles(&study.opening_touches[s]),
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
        NONE.to_string(),
        r.opening[..seats]
            .iter()
            .map(|o| o.ports)
            .sum::<u32>()
            .to_string(),
        NONE.to_string(),
    ]));
    b.push_str(T_CLOSE);
    b.push_str("</section>");

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
tbody tr { transition: background .12s ease; }
tbody tr:hover { background: var(--muted); }
tbody tr:last-child td { border-bottom: 0; }
.rolls th, .rolls td { padding: .7em .45em; }

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
thead th[title], .card-head h2[title] { cursor: help;
                  text-decoration: underline dotted #BFAF9C;
                  text-underline-offset: 4px; }
/* The totals, ruled off above and set in the ink of a figure that matters. */
tfoot td { border-top: 1px solid var(--border); border-bottom: 0;
           font-weight: 600; color: var(--foreground); }
tbody tr:last-child td { border-bottom: 0; }
tfoot tr:hover { background: transparent; }
/* What a thing was worth, beside how many of it there were. */
.worth { color: var(--muted-foreground); }
/* The five resources as tiles: touched ones filled, missed ones hollow. */
.tiles { display: inline-flex; gap: 3px; vertical-align: -3px; }
.tile { width: 15px; height: auto; overflow: visible; }
.tile .off { fill: none; stroke: var(--border); stroke-width: 1.5; }
.tile .on.r0 { fill: #C0563B; } .tile .on.r1 { fill: #1F5E3A; }
.tile .on.r2 { fill: #8DBE4A; } .tile .on.r3 { fill: #E2A32B; }
.tile .on.r4 { fill: #5C6B78; }

/* A subtotal inside the body, ruled off from what it adds up. */
tbody tr.sub td { font-weight: 600; color: var(--foreground);
                  border-top: 1px solid var(--border); }
tbody tr.sub:hover { background: transparent; }

/* ---- the steal flow ----
   Thieves down the left, victims down the right, a ribbon between each pair as
   thick as the cards that moved along it. Laid out on the server, since every
   position is a fraction of a total known the moment the game ends. */
.flow svg { display: block; width: 100%; height: auto; margin: 0 0 1rem; }
.ribbon { opacity: .45; }
.ribbon:hover { opacity: .8; }
.who { font: 500 13px Figtree, system-ui, sans-serif; fill: var(--foreground); }
.who.end { text-anchor: end; }
.who.mid { text-anchor: middle; }
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
.modes label { cursor: pointer; padding: .3em .9em; font-size: 14px;
               border: 1px solid var(--border); border-radius: 999px;
               color: var(--muted-foreground); }
.modes label:hover { color: var(--foreground); background: var(--muted); }
.modes input:checked + label { background: var(--foreground);
                               border-color: var(--foreground);
                               color: var(--card); font-weight: 500; }
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
.axis { font: 400 11px Figtree, system-ui, sans-serif; fill: var(--muted-foreground);
        text-anchor: end; }
.axis.start { text-anchor: start; }
.axis.unit { font-weight: 500; }
.key { display: flex; flex-wrap: wrap; gap: .2rem 1.1rem; margin: .6rem 0 0;
       font-size: 13px; color: var(--muted-foreground); }
.pair { display: inline-flex; align-items: center; gap: .45em; }
.swatch { width: .7em; height: .7em; border-radius: 2px; }
.f0, .swatch.f0 { stroke: var(--p0); background: var(--p0); }
.f1, .swatch.f1 { stroke: var(--p1); background: var(--p1); }
.f2, .swatch.f2 { stroke: var(--p2); background: var(--p2); }
.f3, .swatch.f3 { stroke: var(--p3); background: var(--p3); }
/* The five resources, in the colours their terrain wears on the board. */
.r0, .swatch.r0 { stroke: #C0563B; background: #C0563B; }
.r1, .swatch.r1 { stroke: #1F5E3A; background: #1F5E3A; }
.r2, .swatch.r2 { stroke: #8DBE4A; background: #8DBE4A; }
.r3, .swatch.r3 { stroke: #E2A32B; background: #E2A32B; }
.r4, .swatch.r4 { stroke: #5C6B78; background: #5C6B78; }

/* ---- the trade ring ----
   A chord, because trading is symmetric: there is no side a trade goes from,
   and a sankey would invent a direction the game does not have. */
/* The circle and its table side by side while there is width for it, so the
   card is not a drawing floating in a field of nothing. */
.market { display: flex; flex-wrap: wrap; gap: 1.25rem; align-items: center; }
/* The table needs its columns; the circle only needs to be legible. So the
   circle takes a fixed slice and the table takes the rest, and they wrap onto
   two rows before either is squeezed. */
.market > .ring { flex: 0 1 290px; min-width: 0; }
.market > .tw { flex: 1 1 470px; min-width: 0; }
.ring svg { display: block; width: 100%; height: auto; }
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
            "Result",
            "Turns",
            "Ratings",
            "Dice",
            "Production",
            "Militia",
            "Trades",
            "Development cards",
            "Opening",
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
        assert_eq!(scored(crate::analysis::Scored::default()), "");
    }

    #[test]
    fn the_turns_are_a_table_and_nothing_else() {
        let g = played(4);
        let s = study(&g, std::slice::from_ref(&g)).expect("it studies");
        let html = page(&g, &s);
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
    fn a_seat_wears_its_colour_left_of_its_name_in_every_table() {
        let history: Vec<Saved> = (10..12u64).map(played).collect();
        let s = study(&history[1], &history).expect("it studies");
        let html = page(&history[1], &s);
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
        let html = page(&g, &s);
        let r = &s.report;
        // A bar and a mark for each of the eleven numbers, and no total column
        // on the table under them.
        assert_eq!(html.matches("class=\"stem\"").count(), 11);
        // `owed`, not `mark`: `.mark` is the header wordmark, and a second rule
        // of that name was quietly absolutely-positioning it.
        assert_eq!(html.matches("class=\"owed\"").count(), 11);
        assert_eq!(html.matches("class=\"mark\"").count(), 1, "the wordmark");
        assert!(!html.contains(">total<"));
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
        let html = page(&g, &s);
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
        let html = page(&g, &s);
        // Every table that claims a total has one row of them and no more.
        assert_eq!(
            html.matches("<tfoot>").count(),
            html.matches("</tfoot>").count()
        );
        assert!(html.matches("<tfoot>").count() >= 4, "the summable tables");
        // The turns add across: the seats' turns are the game's turns.
        assert!(html.contains(&format!("<td>{}</td>", s.turns.len())));
        // A maximum is not totalled, and the column that carries it says why.
        assert!(html.contains("A maximum, so it has no total."));
    }

    #[test]
    fn every_long_explanation_is_a_tooltip_now() {
        // Two games, so the dice card has something to compare against.
        let history: Vec<Saved> = (7..9u64).map(played).collect();
        let s = study(&history[1], &history).expect("it studies");
        let html = page(&history[1], &s);
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
        // Nothing to compare with, so the percentile is blank and the count of
        // games it would have been drawn from is nought. Withheld rather than
        // guessed: a percentile of one game is not a percentile.
        assert_eq!(s.dice_percentile, None);
        assert_eq!(s.corpus_games, 0);
        assert!(html.contains("<td>0</td>"), "no games compared");
        assert!(html.contains("Blank until there is a second finished game"));
        // And the card that only exists once there is a corpus does not.
        assert!(!html.contains("Across every game here"));
    }
}
