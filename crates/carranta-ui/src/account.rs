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
//! This page carries one small script and nothing more: the settings save
//! themselves when changed and the name saves itself when typed, because a
//! console that needs a save button pressed is a console that loses whatever
//! was set before somebody remembered to press it. The forms are still forms
//! and the server still answers posts, so the page degrades to Enter-to-save
//! without the script rather than to nothing.

use std::fmt::Write as _;

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

use crate::analysis::player_number;
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
    public: bool,
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
        head = crate::report::masthead_as("your account", &[], "account", who),
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
    b.push_str(
        "<section><h2 title=\"The name new tables will offer for your chair. \
         Games already written down keep the name they were played under.\">\
         Name</h2>",
    );
    let _ = write!(
        b,
        "<form class=\"rename\" method=\"post\" action=\"/account/name\">\
         <input name=\"name\" value=\"{}\" maxlength=\"40\" \
         placeholder=\"what the table should call you\"></form>",
        esc(called),
    );
    b.push_str("</section>");

    b.push_str(&activity(history, aliases, me));

    b.push_str(
        "<section><h2 title=\"A public profile, its games and its statistics, \
         is readable by anybody at /player/ followed by your name. Private is \
         only you, which is where everybody starts.\">Profile</h2>",
    );
    let on = |yes: bool| if yes { " checked" } else { "" };
    let _ = write!(
        b,
        "<form class=\"privacy autosave\" method=\"post\" action=\"/account/privacy\">\
         <div class=\"pills\">\
         <input type=\"radio\" name=\"profile\" value=\"private\" \
         id=\"profile-private\"{}>\
         <label class=\"pill\" for=\"profile-private\">private</label>\
         <input type=\"radio\" name=\"profile\" value=\"public\" \
         id=\"profile-public\"{}>\
         <label class=\"pill\" for=\"profile-public\">public</label>\
         </div></form>",
        on(!public),
        on(public),
    );
    if public && !called.is_empty() {
        let _ = write!(
            b,
            "<p class=\"blurb\">Yours is at <a href=\"/player/{n}\">/player/{n}</a>.</p>",
            n = esc(called),
        );
    }
    b.push_str("</section>");

    b.push_str(
        "<section><h2 title=\"Through Google. What this server keeps is an \
         opaque subject and nothing else: not your email address, not your \
         Google name, not your picture. Signing out removes this browser's \
         key; the account and its games stay.\">Signed in</h2>",
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

    b.push_str("</div></main>");
    b.push_str(SAVES_ITSELF);
    b.push_str("</body></html>");
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
        "<p class=\"blurb\">{played} {} in the last half year.</p>",
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

/// The standard values a new table of yours starts from, in the lobby's own
/// dress: a small capital label over a row of pills, a number with its unit
/// after it, stacked the way the lobby stacks them, so the person who set a
/// table up an hour ago recognises every control here.
///
/// The pills are radios wearing labels, the trick the tabs already use. The
/// form saves itself when anything changes; there is no button to forget.
fn dealing(d: &Defaults) -> String {
    let on = |yes: bool| if yes { " checked" } else { "" };
    let pill = |name: &str, value: &str, label: &str, checked: bool| {
        format!(
            "<input type=\"radio\" name=\"{name}\" value=\"{value}\" \
             id=\"{name}-{value}\"{}><label class=\"pill\" \
             for=\"{name}-{value}\">{label}</label>",
            on(checked),
        )
    };
    let mut b = String::from(
        "<section><h2 title=\"The settings a new table of yours starts from \
         when you press New game. The lobby can still change any of them for \
         one table; changes here save themselves.\">How you deal</h2>",
    );
    b.push_str("<form class=\"dealing autosave\" method=\"post\" action=\"/account/table\">");

    let _ = write!(
        b,
        "<div class=\"set\"><h3>Mode</h3><div class=\"pills\">{}{}</div></div>",
        pill("seats", "4", "4 players", d.seats == 4),
        pill("seats", "3", "3 players", d.seats == 3),
    );
    let _ = write!(
        b,
        "<div class=\"set\"><h3>Chat</h3><div class=\"pills\">{}{}</div></div>",
        pill("chat", "off", "no chat", !d.chat),
        pill("chat", "text", "text", d.chat),
    );
    let _ = write!(
        b,
        "<div class=\"set\"><h3>Turn clock</h3><div class=\"pills\">{}{}{}</div>\
         <div class=\"amount\"><input type=\"number\" name=\"clockSecs\" \
         value=\"{}\" min=\"0\" max=\"6000\"><span class=\"unit\">seconds</span></div>\
         <div class=\"amount\"><input type=\"number\" name=\"clockInc\" \
         value=\"{}\" min=\"0\" max=\"600\"><span class=\"unit\">increment</span></div>\
         <div class=\"amount\"><input type=\"number\" name=\"discardSecs\" \
         value=\"{}\" min=\"0\" max=\"600\">\
         <span class=\"unit\">to discard on a seven</span></div>",
        pill("clock", "turn", "per turn", d.clock == "turn"),
        pill("clock", "chess", "chess clock", d.clock == "chess"),
        pill("clock", "off", "no clock", d.clock == "off"),
        d.clock_secs,
        d.clock_inc,
        d.discard_secs,
    );
    let _ = write!(
        b,
        "<div class=\"set\"><h3>Bank</h3><div class=\"pills\">{}{}</div></div>",
        pill("bank", "exact", "exact count", d.bank_exact),
        pill("bank", "rough", "stack size", !d.bank_exact),
    );
    let _ = write!(
        b,
        "<div class=\"set\"><h3>Log</h3><div class=\"pills\">{}{}</div></div>",
        pill("log", "on", "keep a log", d.log),
        pill("log", "off", "play from memory", !d.log),
    );
    let _ = write!(
        b,
        "<div class=\"set\"><h3>Bot speed</h3><div class=\"pills\">{}{}{}</div></div>",
        pill("pace", "slow", "slow", d.pace == "slow"),
        pill("pace", "fast", "fast", d.pace == "fast"),
        pill("pace", "instant", "instant", d.pace == "instant"),
    );
    let _ = write!(
        b,
        "<div class=\"set\"><h3>Visibility</h3><div class=\"pills\">{}{}</div></div>",
        pill("visibility", "private", "private", !d.public),
        pill("visibility", "public", "public", d.public),
    );
    b.push_str("</form></section>");
    b
}

/// The one script the page carries: forms that save themselves.
///
/// A change on the dealing form posts it whole; typing in the rename posts it
/// after the typing pauses. Failures are silent because the next change posts
/// everything again: the form is the state and the server stores whatever the
/// last post said.
const SAVES_ITSELF: &str = "<script>\
document.querySelectorAll('form.autosave').forEach(f=>\
f.addEventListener('change',()=>\
fetch(f.action,{method:'POST',body:new URLSearchParams(new FormData(f))})));\
const r=document.querySelector('.rename');\
if(r){const i=r.querySelector('input');let t;\
const save=()=>fetch(r.action,{method:'POST',body:new URLSearchParams(new FormData(r))});\
i.addEventListener('input',()=>{clearTimeout(t);t=setTimeout(save,600)});\
r.addEventListener('submit',e=>{e.preventDefault();save();});}\
</script>";

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const EXTRA: &str = "
#tabAccount:checked ~ .tabs label[for=\"tabAccount\"],
#tabDealing:checked ~ .tabs label[for=\"tabDealing\"] {
  background: var(--primary); color: var(--primary-foreground); }
#tabAccount:checked ~ .paneAccount { display: block; }
#tabDealing:checked ~ .paneDealing { display: block; }
/* Air inside the scroll container, so a focus ring is drawn rather than
   shaved off at the pane's edge. */
.pane { padding: 4px 6px; }
.rename input {
  font: 400 15px Figtree, system-ui, sans-serif; color: var(--foreground);
  background: var(--background); border: 1px solid var(--border);
  border-radius: var(--radius-md); padding: .5em .7em; width: min(22em, 100%); }
/* The lobby's controls, control for control: a small capital label, a row of
   pills on a quiet track, a number with its unit after it. */
.set { margin: 0 0 1rem; }
.set h3 { font: 600 12px Figtree, system-ui, sans-serif; text-transform: uppercase;
          letter-spacing: .08em; color: var(--muted-foreground); margin: 0 0 .45rem; }
.pills { display: inline-flex; gap: 4px; background: var(--background);
         border-radius: var(--radius-md); padding: 4px; }
.pills input { position: absolute; width: 1px; height: 1px; opacity: 0; }
.pill { font: 600 14px Figtree, system-ui, sans-serif; cursor: pointer;
        color: var(--muted-foreground); padding: .45em 1em;
        border-radius: var(--radius-md); }
.pills input:checked + .pill { background: var(--primary);
                               color: var(--primary-foreground); }
.pills input:focus-visible + .pill { outline: 2px solid var(--foreground);
                                     outline-offset: 2px; }
.amount { display: flex; align-items: center; gap: .7em; margin: .6rem 0 0; }
.amount input {
  font: 400 15px Figtree, system-ui, sans-serif; color: var(--foreground);
  background: var(--background); border: 1px solid var(--border);
  border-radius: var(--radius-md); padding: .5em .7em; width: 7em; }
.amount .unit { font-size: 14px; color: var(--muted-foreground); }
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
            false,
        );
        assert!(html.contains("value=\"Egon\""), "the name, editable");
        assert!(html.contains("action=\"/account/name\""));
        assert!(html.contains("action=\"/signout\""));
        assert!(html.contains("id=\"tabAccount\""), "the account tab");
        assert!(html.contains("id=\"tabDealing\""), "the settings tab");
        assert!(
            !html.contains("id=\"tabPrivacy\""),
            "privacy is a section of Account, not a tab"
        );
        assert!(
            html.contains("id=\"profile-private\" checked"),
            "private is where everybody starts"
        );
        assert!(html.contains("class=\"weeks\""), "the activity graph");
        assert!(
            !html.contains("What your games add up to"),
            "the record lives on the history page now"
        );
        assert_eq!(
            html.matches("<script").count(),
            1,
            "one script, the self-saving forms, and nothing else"
        );
        assert!(!html.contains("Save table defaults"), "no button to forget");
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
            false,
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
        let html = page(
            &[],
            &NoAliases,
            "egonkey000000000",
            &signed_in("Egon"),
            &d,
            true,
        );
        assert!(html.contains("action=\"/account/table\""));
        assert!(html.contains("name=\"seats\" value=\"3\" id=\"seats-3\" checked"));
        assert!(html.contains("name=\"clock\" value=\"chess\" id=\"clock-chess\" checked"));
    }

    #[test]
    fn a_hostile_name_is_escaped_everywhere_it_appears() {
        let html = page(
            &[],
            &NoAliases,
            "egonkey000000000",
            &signed_in("<script>alert(1)</script>"),
            &Defaults::default(),
            false,
        );
        assert!(!html.contains("<script>alert"));
        assert!(html.contains("&lt;script&gt;"));
    }
}
