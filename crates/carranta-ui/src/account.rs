//! Your account: the page behind your own name.
//!
//! Reached by clicking the name in the header, which is the one place on
//! every page that is already about you. Three things live here and nothing
//! else does: what you are called, how you are signed in, and what your games
//! add up to. The first is the only thing on the page you can change; the
//! record is the corpus's own arithmetic filtered to one person, computed by
//! the same code that computes it for everybody, so this page can never
//! disagree with the across-games one.
//!
//! No script, like every page that changes nothing without a request. The
//! rename is a form, the sign-out is a form, and everything else settled when
//! the games ended.

use std::fmt::Write as _;

use carranta_analytics::corpus::{Config, Corpus, Who as Actor};

/// The table settings a person deals by, in lobby vocabulary.
///
/// Stored as the query string [`deal`] parses and carried here as fields so
/// the form can be prefilled and the stored line can be rebuilt from a posted
/// form, sanitised: whatever arrives, what is written is one of these.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Defaults {
    pub seats: u8,
    /// `turn`, `chess` or `off`.
    pub clock: String,
    pub clock_secs: u64,
    pub clock_inc: u64,
    pub discard_secs: u64,
    /// `slow`, `fast` or `instant`.
    pub pace: String,
    pub bank_exact: bool,
    pub log: bool,
    pub public: bool,
    pub chat: bool,
}

impl Default for Defaults {
    fn default() -> Self {
        Defaults {
            seats: 4,
            clock: "turn".to_string(),
            clock_secs: 60,
            clock_inc: 0,
            discard_secs: crate::game::DEFAULT_DISCARD_SECS,
            pace: "fast".to_string(),
            bank_exact: true,
            log: true,
            public: false,
            chat: false,
        }
    }
}

impl Defaults {
    /// Read from a stored query string or a posted form, which are the same
    /// format. Anything absent or unparseable takes the server's default, and
    /// anything out of range is pulled back in, so a stored line is always one
    /// this build would have written.
    pub fn from_query(q: &str) -> Self {
        let get = |key: &str| -> Option<String> {
            q.split('&').find_map(|pair| {
                let (k, v) = pair.split_once('=')?;
                (k == key).then(|| v.to_string())
            })
        };
        let base = Defaults::default();
        let word = |key: &str, allowed: &[&str], or: String| -> String {
            match get(key) {
                Some(v) if allowed.contains(&v.as_str()) => v,
                _ => or,
            }
        };
        Defaults {
            seats: match get("seats").and_then(|v| v.parse().ok()) {
                Some(3) => 3,
                _ => 4,
            },
            clock: word("clock", &["turn", "chess", "off"], base.clock),
            clock_secs: get("clockSecs")
                .and_then(|v| v.parse().ok())
                .unwrap_or(base.clock_secs)
                .min(6000),
            clock_inc: get("clockInc")
                .and_then(|v| v.parse().ok())
                .unwrap_or(base.clock_inc)
                .min(600),
            discard_secs: get("discardSecs")
                .and_then(|v| v.parse().ok())
                .unwrap_or(base.discard_secs)
                .min(600),
            pace: word("pace", &["slow", "fast", "instant"], base.pace),
            bank_exact: get("bank").as_deref() != Some("rough"),
            log: get("log").as_deref() != Some("off"),
            public: get("visibility").as_deref() == Some("public"),
            chat: get("chat").as_deref() == Some("text"),
        }
    }

    /// The query string [`deal`] parses, in the vocabulary the lobby posts.
    pub fn to_query(&self) -> String {
        format!(
            "seats={}&clock={}&clockSecs={}&clockInc={}&discardSecs={}&pace={}&bank={}&log={}&visibility={}&chat={}",
            self.seats,
            self.clock,
            self.clock_secs,
            self.clock_inc,
            self.discard_secs,
            self.pace,
            if self.bank_exact { "exact" } else { "rough" },
            if self.log { "on" } else { "off" },
            if self.public { "public" } else { "private" },
            if self.chat { "text" } else { "off" },
        )
    }
}

use crate::analysis::{player_number, to_log_as};
use crate::home::Who;
use crate::people::Aliases;
use crate::report::{CSS, ICON};
use crate::store::Saved;

/// The page, for the signed-in person `principal`.
///
/// `history` is every saved game; the record keeps only the ones this person
/// sat in, resolved through the claims, so games played as a guest before the
/// account existed count exactly the way the rating already counts them.
pub fn page(
    history: &[Saved],
    aliases: &dyn Aliases,
    principal: &str,
    who: &Who,
    defaults: &Defaults,
) -> String {
    let me = player_number(&aliases.resolve(principal));
    let called = who.name.trim();

    let mut b = String::new();
    let _ = write!(
        b,
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>Your account · Carranta</title>{ICON}\
         <style>{CSS}{EXTRA}</style></head><body>\
         {head}<main>",
        head = crate::report::masthead_as(
            "your account",
            &[("/history", "History"), ("/corpus", "Across games")],
            &crate::home::account(who),
        ),
    );

    // The deck: one card the width of the window, the lobby's own shape, with
    // the two tabs riding a radio the way the front page's deals do, because
    // this page keeps the no-script rule and needs nothing more. The card
    // holds to the window's height and the panes scroll inside it, so the
    // frame never leaves the screen however long a record grows.
    b.push_str(
        "<div class=\"deck\">\
         <input class=\"tabPick\" type=\"radio\" name=\"tab\" id=\"tabAccount\" \
         tabindex=\"-1\" checked>\
         <input class=\"tabPick\" type=\"radio\" name=\"tab\" id=\"tabDealing\" \
         tabindex=\"-1\">\
         <nav class=\"tabs\">\
         <label for=\"tabAccount\">Account</label>\
         <label for=\"tabDealing\">Default game settings</label>\
         </nav>",
    );

    // ---- the Account pane ---------------------------------------------------
    b.push_str("<div class=\"pane paneAccount\">");
    b.push_str("<section><h2>Name</h2>");
    b.push_str(
        "<p class=\"blurb\">The name new tables will offer for your chair. \
         Games already written down keep the name they were played under.</p>",
    );
    let _ = write!(
        b,
        "<form class=\"rename\" method=\"post\" action=\"/account/name\">\
         <input name=\"name\" value=\"{}\" maxlength=\"40\" \
         placeholder=\"what the table should call you\">\
         <button class=\"go small\" type=\"submit\">Rename</button></form>",
        esc(called),
    );
    b.push_str("</section>");

    b.push_str(&activity(history, aliases, me));

    b.push_str("<section><h2>Signed in</h2>");
    b.push_str(
        "<p class=\"blurb\">Through Google. What this server keeps of that is \
         an opaque subject and nothing else: not your email address, not your \
         Google name, not your picture. Signing out removes this browser's \
         key; the account and its games stay.</p>",
    );
    b.push_str(
        "<form class=\"headOut\" method=\"post\" action=\"/signout\">\
         <button class=\"go small quiet\" type=\"submit\">Sign out</button></form>",
    );
    b.push_str("</section></div>");

    // ---- the Default game settings pane -------------------------------------
    b.push_str("<div class=\"pane paneDealing\">");
    b.push_str(&dealing(defaults));
    b.push_str("</div>");

    b.push_str("</div></main></body></html>");
    b
}

/// Whether this person sat in this game.
pub(crate) fn sat(saved: &Saved, aliases: &dyn Aliases, me: u64) -> bool {
    saved
        .setup
        .chairs
        .iter()
        .any(|c| c.is_person() && player_number(&aliases.resolve(&c.who)) == me)
}

/// The last half year of play, one cell a day, the way a contribution graph
/// draws it: weeks as columns, Monday at the top, today in the last column.
///
/// Drawn from the games' own dealt stamps, which is the one clock the store
/// already keeps. Server time, since the page has no script to ask a browser
/// what day it thinks it is; a game dealt near midnight may land a cell off
/// from the player's own calendar, which is what every such graph accepts.
fn activity(history: &[Saved], aliases: &dyn Aliases, me: u64) -> String {
    const WEEKS: usize = 26;
    let today = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
        / 86_400_000) as i64;
    // Day zero of the epoch was a Thursday; Monday-first columns.
    let weekday = |day: i64| (((day + 3) % 7) + 7) % 7;
    let last_monday = today - weekday(today);
    let first = last_monday - (WEEKS as i64 - 1) * 7;

    let mut counts = std::collections::HashMap::new();
    for saved in history {
        if !sat(saved, aliases, me) {
            continue;
        }
        let day = (saved.dealt / 86_400_000) as i64;
        if day >= first {
            *counts.entry(day).or_insert(0u32) += 1;
        }
    }
    let played: u32 = counts.values().sum();

    let mut b = String::from("<section><h2>Activity</h2>");
    let _ = write!(
        b,
        "<p class=\"blurb\">{played} {} in the last half year, dealt or sat \
         in, finished or not: the graph counts sitting down.</p>",
        if played == 1 { "game" } else { "games" },
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
            let (y, m, dd) = ymd(day);
            let _ = write!(
                b,
                "<span class=\"day l{level}\" title=\"{n} {} on {y:04}-{m:02}-{dd:02}\"></span>",
                if n == 1 { "game" } else { "games" },
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

/// Days since the epoch to a calendar date, the civil-from-days arithmetic,
/// here so a tooltip can name a day without the workspace growing a calendar
/// dependency for one line of text.
fn ymd(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// The standard values a new table of yours starts from.
///
/// Selects rather than anything cleverer, because the page has no script and
/// needs none: the browser holds the state and the form posts it whole. Every
/// field mirrors a lobby control, and the lobby can still override any of it
/// per table, which is what a default is.
fn dealing(d: &Defaults) -> String {
    let picked = |on: bool| if on { " selected" } else { "" };
    let mut b = String::from("<section><h2>How you deal</h2>");
    b.push_str(
        "<p class=\"blurb\">The settings a new table of yours starts from \
         when you press New game. The lobby can still change any of them for \
         one table; this is only where it starts.</p>",
    );
    let _ = write!(
        b,
        "<form class=\"dealing\" method=\"post\" action=\"/account/table\">\
         <label>players <select name=\"seats\">\
         <option value=\"4\"{s4}>4</option><option value=\"3\"{s3}>3</option>\
         </select></label>\
         <label>turn clock <select name=\"clock\">\
         <option value=\"turn\"{ct}>per turn</option>\
         <option value=\"chess\"{cc}>chess clock</option>\
         <option value=\"off\"{co}>no clock</option></select></label>\
         <label>seconds <input type=\"number\" name=\"clockSecs\" value=\"{secs}\" \
         min=\"0\" max=\"6000\"></label>\
         <label>increment <input type=\"number\" name=\"clockInc\" value=\"{inc}\" \
         min=\"0\" max=\"600\"></label>\
         <label>to discard on a seven <input type=\"number\" name=\"discardSecs\" \
         value=\"{disc}\" min=\"0\" max=\"600\"></label>\
         <label>bot speed <select name=\"pace\">\
         <option value=\"slow\"{ps}>slow</option>\
         <option value=\"fast\"{pf}>fast</option>\
         <option value=\"instant\"{pi}>instant</option></select></label>\
         <label>bank <select name=\"bank\">\
         <option value=\"exact\"{be}>exact count</option>\
         <option value=\"rough\"{br}>stack size</option></select></label>\
         <label>log <select name=\"log\">\
         <option value=\"on\"{lo}>keep a log</option>\
         <option value=\"off\"{lf}>play from memory</option></select></label>\
         <label>visibility <select name=\"visibility\">\
         <option value=\"private\"{vp}>private</option>\
         <option value=\"public\"{vu}>public</option></select></label>\
         <label>chat <select name=\"chat\">\
         <option value=\"off\"{cn}>no chat</option>\
         <option value=\"text\"{cx}>text</option></select></label>\
         <button class=\"go small\" type=\"submit\">Save table defaults</button>\
         </form></section>",
        s4 = picked(d.seats == 4),
        s3 = picked(d.seats == 3),
        ct = picked(d.clock == "turn"),
        cc = picked(d.clock == "chess"),
        co = picked(d.clock == "off"),
        secs = d.clock_secs,
        inc = d.clock_inc,
        disc = d.discard_secs,
        ps = picked(d.pace == "slow"),
        pf = picked(d.pace == "fast"),
        pi = picked(d.pace == "instant"),
        be = picked(d.bank_exact),
        br = picked(!d.bank_exact),
        lo = picked(d.log),
        lf = picked(!d.log),
        vp = picked(!d.public),
        vu = picked(d.public),
        cn = picked(!d.chat),
        cx = picked(d.chat),
    );
    b
}

/* The two forms, in the page's own idiom: text inputs and selects at the
card's size, labels as quiet words beside them, one control to a line in
the dealing grid so ten settings read as a list rather than a wall. */

const EXTRA: &str = "
#tabAccount:checked ~ .tabs label[for=\"tabAccount\"],
#tabDealing:checked ~ .tabs label[for=\"tabDealing\"] {
  background: var(--primary); color: var(--primary-foreground); }
#tabAccount:checked ~ .paneAccount { display: block; }
#tabDealing:checked ~ .paneDealing { display: block; }
.rename { display: flex; gap: .6em; align-items: center; margin-top: .4rem; }
.rename input { flex: 0 1 22em; }
.rename input, .dealing input, .dealing select {
  font: 400 14px Figtree, system-ui, sans-serif; color: var(--foreground);
  background: var(--background); border: 1px solid var(--border);
  border-radius: var(--radius-md); padding: .45em .6em; }
.dealing { display: grid; grid-template-columns: repeat(auto-fill, minmax(15em, 1fr));
           gap: .7em 1.2em; align-items: center; margin-top: .4rem; }
.dealing label { display: flex; justify-content: space-between; align-items: center;
                 gap: .8em; font-size: 14px; color: var(--muted-foreground); }
.dealing input { width: 6em; }
.dealing button { grid-column: 1 / -1; justify-self: start; margin-top: .3rem; }
.weeks { display: flex; gap: 3px; overflow-x: auto; padding-bottom: .3rem; }
.week { display: flex; flex-direction: column; gap: 3px; }
.day { width: 11px; height: 11px; border-radius: 2px; background: var(--background); }
.day.off { background: none; }
.day.l1 { background: #F2CDB2; }
.day.l2 { background: #EDA477; }
.day.l3 { background: #E8703C; }
.day.l4 { background: #C2492A; }
.scale { display: flex; gap: 3px; align-items: center; margin: .3rem 0 0;
         font-size: 12px; color: var(--muted-foreground); }
";

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
    use crate::game::Session;
    use crate::people::NoAliases;
    use crate::store::{Chair, Setup, game_id};
    use carranta_core::state::TradeMode;

    fn signed_in(name: &str) -> Who {
        Who {
            offered: true,
            signed_in: true,
            name: name.to_string(),
        }
    }

    fn played(seed: u64, key: &str, name: &str) -> Saved {
        let mut s = Session::new(4, seed, TradeMode::Full);
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
                chairs: vec![
                    Chair::person(key, name),
                    Chair::bot(),
                    Chair::bot(),
                    Chair::bot(),
                ],
                ..Default::default()
            },
            moves: s.moves().to_vec(),
            times: s.times().to_vec(),
        }
    }

    #[test]
    fn the_page_is_the_name_the_provider_and_the_record() {
        let history = vec![played(11, "egonkey000000000", "Egon")];
        let html = page(
            &history,
            &NoAliases,
            "egonkey000000000",
            &signed_in("Egon"),
            &Defaults::default(),
        );
        assert!(html.contains("value=\"Egon\""), "the name, editable");
        assert!(html.contains("action=\"/account/name\""));
        assert!(html.contains("action=\"/signout\""));
        assert!(html.contains("id=\"tabAccount\""), "the account tab");
        assert!(html.contains("id=\"tabDealing\""), "the settings tab");
        assert!(html.contains("class=\"weeks\""), "the activity graph");
        assert!(
            !html.contains("What your games add up to"),
            "the record lives on the history page now"
        );
        assert!(!html.contains("<script"), "no script on the account page");
    }

    #[test]
    fn somebody_else_s_games_are_not_your_record() {
        let history = vec![played(12, "somebodyelse00000", "Frida")];
        let html = page(
            &history,
            &NoAliases,
            "egonkey000000000",
            &signed_in("Egon"),
            &Defaults::default(),
        );
        assert!(
            html.contains("0 games in the last half year"),
            "the activity counts nothing rather than borrowing somebody's games"
        );
    }

    #[test]
    fn defaults_survive_the_trip_through_their_query_string() {
        let d = Defaults {
            seats: 3,
            clock: "chess".to_string(),
            clock_secs: 300,
            clock_inc: 5,
            discard_secs: 25,
            pace: "instant".to_string(),
            bank_exact: false,
            log: false,
            public: true,
            chat: true,
        };
        assert_eq!(Defaults::from_query(&d.to_query()), d, "exact round trip");
        // Junk is not stored: whatever arrives, what is written is a line this
        // build would have written itself.
        let mangled =
            Defaults::from_query("seats=11&clock=sundial&discardSecs=9999&pace=ludicrous");
        assert_eq!(mangled.seats, 4);
        assert_eq!(mangled.clock, "turn");
        assert_eq!(mangled.discard_secs, 600, "clamped, not trusted");
        assert_eq!(mangled.pace, "fast");
        // The page carries the form, prefilled.
        let html = page(&[], &NoAliases, "egonkey000000000", &signed_in("Egon"), &d);
        assert!(html.contains("action=\"/account/table\""));
        assert!(html.contains("value=\"3\" selected"));
        assert!(html.contains("value=\"chess\" selected"));
    }

    #[test]
    fn a_hostile_name_is_escaped_everywhere_it_appears() {
        let html = page(
            &[],
            &NoAliases,
            "egonkey000000000",
            &signed_in("<script>alert(1)</script>"),
            &Defaults::default(),
        );
        assert!(!html.contains("<script>alert"));
        assert!(html.contains("&lt;script&gt;"));
    }
}
