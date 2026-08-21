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

    // The console: one tab per section, the pattern the account and history
    // pages already wear, so the page itself never scrolls; a long table
    // scrolls inside its pane. The tab strip is the page's whole table of
    // contents, and the ids are made here because the sections are counted
    // here.
    let mut extra = String::new();
    for i in 0..sections.len() {
        let _ = write!(
            extra,
            "#tabC{i}:checked ~ .tabs label[for=\"tabC{i}\"] {{ \
             background: var(--primary); color: var(--primary-foreground); }}\
             #tabC{i}:checked ~ .paneC{i} {{ display: block; }}\n",
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
        for i in 0..sections.len() {
            let _ = write!(
                b,
                "<input class=\"tabPick\" type=\"radio\" name=\"tab\" \
                 id=\"tabC{i}\" tabindex=\"-1\"{}>",
                if i == 0 { " checked" } else { "" },
            );
        }
        b.push_str("<nav class=\"tabs\">");
        for (i, s) in sections.iter().enumerate() {
            let _ = write!(
                b,
                "<label for=\"tabC{i}\" title=\"{long}. Nothing here \
                 crosses a configuration or pools people with self-play: a \
                 bot corpus is bigger by orders of magnitude, and an average \
                 over both is a bot average wearing a different \
                 name.\">{short}</label>",
                long = section_title(s),
                short = tab_label(s),
            );
        }
        b.push_str("</nav>");
        for (i, s) in sections.iter().enumerate() {
            let _ = write!(b, "<div class=\"pane paneC{i}\">");
            b.push_str(&section(s, &names));
            b.push_str("</div>");
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

/// The tab's word: short enough for a strip of several.
fn tab_label(s: &Section) -> String {
    format!(
        "{:?} v{} · {}",
        s.config.trade_mode,
        s.config.rules_version,
        if s.people { "people" } else { "self-play" },
    )
}

/// The full name the heading used to carry, now the tab's tooltip.
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

fn section(s: &Section, names: &HashMap<u64, String>) -> String {
    let c = &s.corpus;
    let mut b = String::from("<section>");
    // The tab names the configuration; the pane opens with its numbers alone.
    let _ = write!(
        b,
        "<p class=\"blurb\">{n} {games}, {f} finished, {t:.0} turns on average.</p>",
        n = c.games,
        games = plural(c.games as usize, "game", "games"),
        f = c.finished,
        t = c.mean_turns(),
    );

    b.push_str(&seats(c));
    b.push_str(&actors(c, names));
    b.push_str(&turns(c));
    b.push_str(&audit(c));
    b.push_str("</section>");
    b
}

/// Win rate by seat: the first-player-advantage question (A-4).
fn seats(c: &Corpus) -> String {
    if c.finished == 0 {
        return String::new();
    }
    let rates = c.seat_win_rate();
    let mut b = String::from(
        "<h3 title=\"Whether going first is worth anything. The draw \
         shuffles who sits where, so over enough games every seat holds every \
         kind of player and the rates below are the seats' own.\">Seats</h3>",
    );
    b.push_str(TABLE_OPEN);
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
    let mut b = String::from(
        "<h3 title=\"Everyone the record can tell apart: an agent is its \
         name and build wherever it sits, a person is themselves across \
         every game including the ones played as a guest before signing \
         up.\">Players</h3>",
    );
    b.push_str(TABLE_OPEN);
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

/// Mean VP over the turns, every mean beside the games it was computed over.
fn turns(c: &Corpus) -> String {
    let rows = c.vp_turns.rows();
    if rows.is_empty() {
        return String::new();
    }
    let mut b = String::from(
        "<h3 title=\"Mean victory points per seat as games run on. A game \
         ends when somebody reaches ten, so the later rows average only the \
         games still going, which are the slow ones; the games column is \
         what keeps that honest.\">Over the turns</h3>",
    );
    b.push_str(TABLE_OPEN);
    b.push_str("<thead><tr><th>turn</th><th>mean VP</th><th>games</th></tr></thead><tbody>");
    let last = rows.len() - 1;
    for (i, (turn, mean, n)) in rows.iter().enumerate() {
        // Every fifth turn plus the first and the last: the shape at the
        // page's size, not the whole vector.
        if i != 0 && i != last && turn % 5 != 0 {
            continue;
        }
        let _ = write!(b, "<tr><td>{turn}</td><td>{mean:.2}</td><td>{n}</td></tr>",);
    }
    b.push_str(TABLE_CLOSE);
    b
}

/// The pooled generator audit (§10.1b), which is the only fairness question a
/// corpus can answer.
fn audit(c: &Corpus) -> String {
    if c.rolls.is_empty() {
        return String::new();
    }
    let a = c.dice_audit();
    format!(
        "<h3 title=\"The generator over every roll this section holds, \
         pooled. One game's dice being strange is weather and belongs on that \
         game's report; the pool is the climate.\">Dice</h3>\
         <p class=\"blurb\">{n} rolls · sevens {sevens:.1}% against 16.7% \
         expected · KL from theory {kl:.5} bits · chi-squared p = {p:.2}</p>",
        n = c.rolls.len(),
        sevens = a.seven_share * 100.0,
        kl = a.kl_bits,
        p = a.p_value,
    )
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
        // Two tabs of one configuration, not one of two games; each is a
        // pane of the console, and the first tab is the open one.
        assert_eq!(html.matches("Full market · rules v").count(), 2);
        assert_eq!(html.matches("class=\"pane paneC").count(), 2);
        assert!(html.contains("id=\"tabC0\" tabindex=\"-1\" checked"));
        assert!(html.contains(">Full v1 · people</label>"));
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
