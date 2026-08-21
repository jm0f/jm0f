//! Across games: the §10.3 corpus battery, sliced the way §10.6 allows.
//!
//! The report answers "what happened in this game"; this page answers "what
//! happens", and everything §10.6 warns about lives in the gap between those
//! two questions. Three boundaries are drawn before anything here is averaged,
//! and each one is a pitfall from that list made structural:
//!
//! - **Configuration** (pitfall 4). Trade mode and rules version change the
//!   game, so every section is one configuration and nothing crosses it. The
//!   corpus type refuses mixed games on its own; this page never asks it to.
//! - **People against self-play** (pitfall 5). Bot corpora outnumber human
//!   ones by orders of magnitude, so a pooled average is a bot average with
//!   people as rounding error. Games with a person at the table are their own
//!   sections, listed first.
//! - **Seats are not samples** (pitfall 3). The intervals on the player table
//!   are clustered by game, from the analytics crate, because in self-play one
//!   actor holds several chairs of one game and their results are one result.
//!
//! Sliced by who was playing wherever the record can say: an agent is its
//! name and build (`trained@378` across a thousand games is one row), a
//! person is their durable key with their claims followed (P-1), shown under
//! the name their latest chair carried. No script, like the report and for
//! the same reason: everything here settled when the games ended.

use std::collections::HashMap;
use std::fmt::Write as _;

use carranta_analytics::corpus::{ActorRow, Config, Corpus, Who, has_human};
use carranta_core::state::MAX_PLAYERS;

use crate::analysis::{player_number, to_log_as};
use crate::people::Aliases;
use crate::report::{CSS, ICON};
use crate::store::Saved;

/// One configuration's games, people and self-play kept apart.
struct Section {
    config: Config,
    /// Whether a person sat in any of these games.
    people: bool,
    corpus: Corpus,
}

/// The page, from every saved game.
///
/// Order of `history` does not matter to the numbers; the one thing it decides
/// is which spelling of a person's name wins, and the caller passes oldest
/// first so the latest one does.
pub fn page(history: &[Saved], who: &dyn Aliases, viewer: &crate::home::Who) -> String {
    let mut sections: Vec<Section> = Vec::new();
    let mut names: HashMap<u64, String> = HashMap::new();
    let mut unreadable = 0usize;

    for saved in history {
        // The name a person's row will carry, from the chair they sat in.
        // Keyed the way the analytics key them, resolved through the claims,
        // so the label and the grouping cannot disagree.
        for chair in &saved.setup.chairs {
            if chair.is_person() && !chair.name.trim().is_empty() {
                names.insert(player_number(&who.resolve(&chair.who)), chair.name.clone());
            }
        }
        let Some(log) = to_log_as(saved, who) else {
            // A file this build cannot replay is not silently a smaller
            // corpus: it is counted and said, because a number computed over
            // "most of the games" has to say so to be honest.
            unreadable += 1;
            continue;
        };
        let config = Config::of(&log);
        let people = has_human(&log);
        let section = match sections
            .iter_mut()
            .find(|s| s.config == config && s.people == people)
        {
            Some(s) => s,
            None => {
                sections.push(Section {
                    config,
                    people,
                    corpus: Corpus::new(config),
                });
                sections.last_mut().expect("just pushed")
            }
        };
        // No Monte Carlo per game here: the pooled audit and the effect
        // sizes are the corpus questions, and the per-game p-value already
        // has a home on the game's own report.
        if !section.corpus.add(&log, 0) {
            unreadable += 1;
        }
    }

    // People first, because those sections are the small ones this page
    // exists to keep out of the bots' shadow; then newest rules first, then
    // the trade modes in their engine order.
    sections.sort_by(|a, b| {
        b.people
            .cmp(&a.people)
            .then(b.config.rules_version.cmp(&a.config.rules_version))
            .then((a.config.trade_mode as u8).cmp(&(b.config.trade_mode as u8)))
    });

    // The console: one tab per analysis, the pattern the account and history
    // pages already wear, so the page itself never scrolls; a long table
    // scrolls inside its pane. Inside every pane the configurations stay
    // apart: one block per section, headed by its full name, because the
    // boundary between them is the page's whole methodology.
    const TABS: [(&str, &str, &str); 5] = [
        (
            "tabOverview",
            "Overview",
            "What each body of games amounts to, and when they were played. \
             Nothing on this page crosses a configuration or pools people \
             with self-play: a bot corpus is bigger by orders of magnitude, \
             and an average over both is a bot average wearing a different \
             name.",
        ),
        (
            "tabSeats",
            "Seats",
            "Whether going first is worth anything. The draw shuffles who \
             sits where, so over enough games every seat holds every kind of \
             player and the rates are the seats' own.",
        ),
        (
            "tabPlayers",
            "Players",
            "Everyone the record can tell apart: an agent is its name and \
             build wherever it sits, a person is themselves across every \
             game including the ones played as a guest before signing up.",
        ),
        (
            "tabTurns",
            "Over the turns",
            "Mean victory points per seat as games run on, the whisker one \
             standard deviation across games either side. A game ends when \
             somebody reaches ten, so later turns average only the games \
             still going, which are the slow ones; every candle names its \
             games for that reason.",
        ),
        (
            "tabDice",
            "Dice",
            "The generator over every roll a section holds, pooled. One \
             game's dice being strange is weather and belongs on that game's \
             report; the pool is the climate.",
        ),
    ];
    let mut extra = String::new();
    for (id, _, _) in TABS {
        let _ = write!(
            extra,
            "#{id}:checked ~ .tabs label[for=\"{id}\"] {{ \
             background: var(--primary); color: var(--primary-foreground); }}\
             #{id}:checked ~ .pane{pane} {{ display: block; }}\n",
            pane = &id[3..],
        );
    }

    let mut b = String::new();
    let _ = write!(
        b,
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>Across games · Carranta</title>{ICON}\
         <style>{CSS}{extra}</style></head><body>\
         {head}<main>",
        head = crate::report::masthead_as("across games", &[], "corpus", viewer),
    );

    b.push_str("<div class=\"deck\">");
    if sections.is_empty() {
        b.push_str(
            "<p class=\"blurb\">Nothing to count yet: this server has \
             no saved games. Every finished game joins this page on its own.</p>",
        );
    } else {
        for (id, _, _) in TABS {
            let _ = write!(
                b,
                "<input class=\"tabPick\" type=\"radio\" name=\"tab\" \
                 id=\"{id}\" tabindex=\"-1\"{}>",
                if id == "tabSeats" { " checked" } else { "" },
            );
        }
        b.push_str("<nav class=\"tabs\">");
        for (id, label, tip) in TABS {
            let _ = write!(b, "<label for=\"{id}\" title=\"{tip}\">{label}</label>");
        }
        b.push_str("</nav>");

        // Overview: each section's totals, then when the games were played,
        // which is the one graph volume alone can honestly share.
        b.push_str("<div class=\"pane paneOverview\">");
        for s in &sections {
            let c = &s.corpus;
            let _ = write!(
                b,
                "<section><h3>{title}</h3>\
                 <p class=\"blurb\">{n} {games}, {f} finished, {t:.0} turns \
                 on average.</p></section>",
                title = section_title(s),
                n = c.games,
                games = plural(c.games as usize, "game", "games"),
                f = c.finished,
                t = c.mean_turns(),
            );
        }
        b.push_str(&activity_all(history));
        b.push_str("</div>");

        for (pane, body) in [
            (
                "Seats",
                &sections.iter().map(seats_block).collect::<String>(),
            ),
            (
                "Players",
                &sections
                    .iter()
                    .map(|s| actors_block(s, &names))
                    .collect::<String>(),
            ),
            (
                "Turns",
                &sections.iter().map(turns_block).collect::<String>(),
            ),
            ("Dice", &sections.iter().map(dice_block).collect::<String>()),
        ] {
            let _ = write!(b, "<div class=\"pane pane{pane}\">{body}</div>");
        }
    }
    if unreadable > 0 {
        let _ = write!(
            b,
            "<p class=\"blurb\">{unreadable} saved \
             {} this build could not replay {} left out of every number above.</p>",
            plural(unreadable, "game", "games"),
            if unreadable == 1 { "is" } else { "are" },
        );
    }
    b.push_str("</div></main></body></html>");
    b
}

/// When the games were played, all of them together: 26 weeks of days in the
/// account graph's own dress. Volume is the one number that can cross every
/// configuration without lying, because nothing is averaged.
fn activity_all(history: &[Saved]) -> String {
    const WEEKS: usize = 26;
    let today = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
        / 86_400_000) as i64;
    let weekday = |day: i64| (((day + 3) % 7) + 7) % 7;
    let last_monday = today - weekday(today);
    let first = last_monday - (WEEKS as i64 - 1) * 7;

    let mut counts = HashMap::new();
    for saved in history {
        let day = (saved.dealt / 86_400_000) as i64;
        if day >= first {
            *counts.entry(day).or_insert(0u32) += 1;
        }
    }
    let played: u32 = counts.values().sum();

    let mut b = String::from("<section><h3>Activity</h3>");
    let _ = write!(
        b,
        "<p class=\"blurb\">{played} {} on this server in the last half year.</p>",
        plural(played as usize, "game", "games"),
    );
    b.push_str("<div class=\"weeks\">");
    for w in 0..WEEKS as i64 {
        b.push_str("<div class=\"week\">");
        for d in 0..7 {
            let day = first + w * 7 + d;
            if day > today {
                b.push_str("<span class=\"day off\"></span>");
                continue;
            }
            let n = counts.get(&day).copied().unwrap_or(0);
            let level = match n {
                0 => 0,
                1 => 1,
                2..=3 => 2,
                4..=6 => 3,
                _ => 4,
            };
            let (y, m, dd) = crate::account::ymd(day);
            let _ = write!(
                b,
                "<span class=\"day l{level}\" title=\"{n} {} on {y:04}-{m:02}-{dd:02}\"></span>",
                plural(n as usize, "game", "games"),
            );
        }
        b.push_str("</div>");
    }
    b.push_str("</div>");
    b.push_str(
        "<p class=\"scale\">none<span class=\"day l1\"></span>\
         <span class=\"day l2\"></span><span class=\"day l3\"></span>\
         <span class=\"day l4\"></span>plenty</p>",
    );
    b.push_str("</section>");
    b
}

/// One configuration's name over one analysis body, or nothing when the body
/// has nothing to show: an empty heading would claim a table that is not
/// there.
fn block(title: &str, body: String) -> String {
    if body.is_empty() {
        return String::new();
    }
    format!("<section><h3>{title}</h3>{body}</section>")
}

fn seats_block(s: &Section) -> String {
    block(&section_title(s), seats(&s.corpus))
}

fn actors_block(s: &Section, names: &HashMap<u64, String>) -> String {
    block(&section_title(s), actors(&s.corpus, names))
}

fn turns_block(s: &Section) -> String {
    block(&section_title(s), turns(&s.corpus))
}

fn dice_block(s: &Section) -> String {
    block(&section_title(s), audit(&s.corpus))
}

/// A configuration's full name, over its block in every pane.
fn section_title(s: &Section) -> String {
    format!(
        "{:?} market · rules v{} · {}",
        s.config.trade_mode,
        s.config.rules_version,
        if s.people {
            "with people at the table"
        } else {
            "self-play"
        },
    )
}

/// Win rate by seat: the first-player-advantage question (A-4).
fn seats(c: &Corpus) -> String {
    if c.finished == 0 {
        return String::new();
    }
    let rates = c.seat_win_rate();
    let mut b = String::from(TABLE_OPEN);
    b.push_str(
        "<thead><tr><th>seat</th><th>games</th><th>wins</th><th>win rate</th></tr></thead><tbody>",
    );
    for p in 0..MAX_PLAYERS {
        if c.seat_games[p] == 0 {
            continue;
        }
        let _ = write!(
            b,
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{:.1}%</td></tr>",
            p + 1,
            c.seat_games[p],
            c.seat_wins[p],
            rates[p] * 100.0,
        );
    }
    b.push_str(TABLE_CLOSE);
    b
}

/// The per-player table, which is what the page is for.
fn actors(c: &Corpus, names: &HashMap<u64, String>) -> String {
    let rows = c.actor_rows();
    if rows.is_empty() {
        return String::new();
    }
    let mut b = String::from(TABLE_OPEN);
    b.push_str(
        "<thead><tr><th>player</th><th>games</th><th>seats</th><th>wins</th>\
         <th title=\"The interval is clustered by game, because chairs in one \
         game win and lose together.\">win rate</th><th>mean VP</th>\
         <th title=\"The §10.4 residual: points scored above what the \
         player's production predicts, the closest thing here to skill with \
         the luck taken out.\">conversion</th></tr></thead><tbody>",
    );
    for row in &rows {
        let _ = write!(
            b,
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td>\
             <td>{:.2}</td><td>{}</td></tr>",
            name_of(&row.who, names),
            row.games,
            row.seats,
            row.wins,
            share(row),
            row.mean_vp,
            row.residual
                .map(|r| format!("{r:+.2}"))
                .unwrap_or_else(|| "·".to_string()),
        );
    }
    b.push_str(TABLE_CLOSE);
    b
}

/// Mean VP over the turns as candles: the bar is the mean, the whisker one
/// standard deviation across games either side of it, every candle naming the
/// games it averages, because later turns hold only the slow ones.
fn turns(c: &Corpus) -> String {
    let rows = c.vp_turns.spread_rows();
    if rows.is_empty() {
        return String::new();
    }
    // Every fifth turn plus the first and the last: the shape at the page's
    // size, not the whole vector.
    let last = rows.len() - 1;
    let picked: Vec<&(usize, f64, f64, u32)> = rows
        .iter()
        .enumerate()
        .filter(|(i, (turn, _, _, _))| *i == 0 || *i == last || turn % 5 == 0)
        .map(|(_, r)| r)
        .collect();
    let tallest = picked
        .iter()
        .map(|(_, mean, sd, _)| mean + sd)
        .fold(0.0, f64::max);
    if tallest <= 0.0 {
        return String::new();
    }
    let mut b = String::from("<div class=\"tw\"><table class=\"rolls\"><thead>");
    b.push_str("<tr class=\"chart\">");
    for (turn, mean, sd, n) in &picked {
        let low = ((mean - sd).max(0.0) / tallest) * 100.0;
        let high = ((mean + sd) / tallest) * 100.0;
        let _ = write!(
            b,
            "<td><div class=\"col\" data-tip=\"turn {turn}: mean {mean:.2} VP \
             ± {sd:.2} over {n} {}\">\
             <div class=\"stem\" style=\"height:{h:.1}%\"></div>\
             <div class=\"whisk\" style=\"bottom:{low:.1}%;height:{wh:.1}%\"></div>\
             </div></td>",
            plural(*n as usize, "game", "games"),
            h = (mean / tallest) * 100.0,
            wh = (high - low).max(0.5),
        );
    }
    b.push_str("</tr><tr>");
    for (turn, _, _, _) in &picked {
        let _ = write!(b, "<th>{turn}</th>");
    }
    b.push_str("</tr></thead></table></div>");
    b
}

/// The pooled generator audit (§10.1b), drawn the way a single game's report
/// draws it: the rolls as bars with the fair-dice expectation marked across
/// them, and the pooled numbers under the chart.
fn audit(c: &Corpus) -> String {
    if c.rolls.is_empty() {
        return String::new();
    }
    let a = c.dice_audit();
    let mut counts = [0u32; 11];
    for &r in &c.rolls {
        if (2..=12).contains(&r) {
            counts[r as usize - 2] += 1;
        }
    }
    let total: u32 = counts.iter().sum();
    let expect = |n: u32| f64::from(total) * f64::from(6 - (n as i32 - 7).abs() as u32) / 36.0;
    let tallest = (2..=12u32)
        .map(|n| expect(n).max(f64::from(counts[n as usize - 2])))
        .fold(0.0, f64::max);
    if tallest <= 0.0 {
        return String::new();
    }
    let mut b = String::from("<div class=\"tw\"><table class=\"rolls\"><thead>");
    b.push_str("<tr class=\"chart\">");
    for n in 2..=12u32 {
        let got = counts[n as usize - 2];
        let _ = write!(
            b,
            "<td><div class=\"col\" data-tip=\"{n}: rolled {got}, expected {e:.1}\">\
             <div class=\"stem\" style=\"height:{h:.1}%\"></div>\
             <div class=\"owed\" style=\"bottom:{m:.1}%\"></div></div></td>",
            e = expect(n),
            h = 100.0 * f64::from(got) / tallest,
            m = 100.0 * expect(n) / tallest,
        );
    }
    b.push_str("</tr><tr>");
    for n in 2..=12u32 {
        let _ = write!(b, "<th>{n}</th>");
    }
    b.push_str("</tr></thead></table></div>");
    let _ = write!(
        b,
        "<p class=\"blurb\">{n} rolls · sevens {sevens:.1}% against 16.7% \
         expected · KL from theory {kl:.5} bits · chi-squared p = {p:.2}</p>",
        n = c.rolls.len(),
        sevens = a.seven_share * 100.0,
        kl = a.kl_bits,
        p = a.p_value,
    );
    b
}

fn name_of(who: &Who, names: &HashMap<u64, String>) -> String {
    match who {
        Who::Agent { name, version } => esc(&format!("{name}@{version}")),
        Who::Human { player } => match names.get(player) {
            Some(name) => esc(name),
            None => "a guest".to_string(),
        },
    }
}

fn share(row: &ActorRow) -> String {
    match row.half_width {
        Some(h) if h > 0.0 => format!("{:.1}% ± {:.1}", row.share * 100.0, h * 100.0),
        _ => format!("{:.1}%", row.share * 100.0),
    }
}

fn plural<'a>(n: usize, one: &'a str, many: &'a str) -> &'a str {
    if n == 1 { one } else { many }
}

const TABLE_OPEN: &str = "<div class=\"tw\"><table>";
const TABLE_CLOSE: &str = "</tbody></table></div>";

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nobody_who() -> crate::home::Who {
        crate::home::Who {
            offered: false,
            signed_in: false,
            name: String::new(),
        }
    }
    use crate::game::Session;
    use crate::people::NoAliases;
    use crate::store::{Chair, Setup, game_id};
    use carranta_core::state::TradeMode;

    /// Play one out and save it, the way the server would, with the chairs
    /// spelling out who sat where.
    fn played(seed: u64, mode: TradeMode, chairs: Vec<Chair>) -> Saved {
        let mut s = Session::new(4, seed, mode);
        for _ in 0..2000 {
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
            name: String::new(),
            by: String::new(),
            dealt: seed,
            winner: s.winner(),
            setup: Setup {
                chairs,
                ..Default::default()
            },
            moves: s.moves().to_vec(),
            times: s.times().to_vec(),
        }
    }

    fn bots() -> Vec<Chair> {
        vec![Chair::bot(), Chair::bot(), Chair::bot(), Chair::bot()]
    }

    fn with_person(name: &str) -> Vec<Chair> {
        vec![
            Chair::person("egonkey000000000", name),
            Chair::bot(),
            Chair::bot_as("trained@378"),
            Chair::bot(),
        ]
    }

    #[test]
    fn people_and_self_play_are_separate_sections() {
        let history = vec![
            played(1, TradeMode::Full, bots()),
            played(2, TradeMode::Full, with_person("Egon")),
        ];
        let html = page(&history, &NoAliases, &nobody_who());
        assert!(html.contains("with people at the table"));
        assert!(html.contains("self-play"));
        // One tab per analysis with Seats open, and inside every pane the
        // two bodies of one configuration stay apart, each under its own
        // full name: two blocks in the overview, never a pooled one.
        for id in [
            "tabOverview",
            "tabSeats",
            "tabPlayers",
            "tabTurns",
            "tabDice",
        ] {
            assert!(html.contains(&format!("id=\"{id}\"")), "{id}");
        }
        assert!(html.contains("id=\"tabSeats\" tabindex=\"-1\" checked"));
        // rfind, because the stylesheet names the panes before the body does.
        let overview = &html[html.rfind("paneOverview").expect("an overview pane")
            ..html.rfind("paneSeats").expect("a seats pane")];
        assert_eq!(overview.matches("Full market · rules v").count(), 2);
        assert!(overview.contains(">Activity</h3>"), "the all-games graph");
    }

    #[test]
    fn an_agent_is_named_by_its_build_and_a_person_by_their_chair() {
        let history = vec![played(3, TradeMode::Full, with_person("Egon"))];
        let html = page(&history, &NoAliases, &nobody_who());
        assert!(html.contains("trained@378"), "the champion's row");
        assert!(html.contains("house@1"), "the house rows pooled");
        assert!(html.contains("Egon"), "the person under their name");
    }

    #[test]
    fn configurations_never_share_a_section() {
        let history = vec![
            played(4, TradeMode::Full, bots()),
            played(5, TradeMode::Disabled, bots()),
        ];
        let html = page(&history, &NoAliases, &nobody_who());
        assert!(html.contains("Full market"));
        assert!(html.contains("Disabled market"));
    }

    #[test]
    fn every_per_turn_mean_is_printed_beside_its_n() {
        let history = vec![
            played(6, TradeMode::Full, bots()),
            played(7, TradeMode::Full, bots()),
        ];
        let html = page(&history, &NoAliases, &nobody_who());
        assert!(html.contains("Over the turns"));
        assert!(html.contains("<th>games</th>"), "the n column is there");
    }

    #[test]
    fn the_page_carries_no_script_and_escapes_names() {
        let history = vec![played(
            8,
            TradeMode::Full,
            with_person("<script>alert(1)</script>"),
        )];
        let html = page(&history, &NoAliases, &nobody_who());
        assert!(!html.contains("<script"), "no script on the corpus page");
        assert!(html.contains("&lt;script&gt;"), "the name is escaped");
    }

    #[test]
    fn an_empty_server_is_still_a_page() {
        let html = page(&[], &NoAliases, &nobody_who());
        assert!(html.contains("Nothing to count yet"));
    }
}
