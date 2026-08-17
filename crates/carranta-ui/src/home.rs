//! The home page: where a game comes from.
//!
//! Before this there was nowhere to stand. The root redirected to whatever game
//! the server happened to be holding, the only way to deal another was a panel
//! inside a game you were already in, and a game you had finished was reachable
//! only if you had kept the link. The board page's wordmark said as much in a
//! comment: it opened the lobby because there was no home to go to.
//!
//! Three sections, in the order somebody wants them: start one, join one, or
//! look at one you played. Server-rendered with a form and links, and **no
//! script at all**, like the analytics page and for the same reason: nothing
//! here changes without a request, so there is nothing for a script to do.
//!
//! Whose games are whose comes from a cookie, which is honest about what it is:
//! a key handed to a browser, not a login. It is enough to answer "show me
//! mine" on one machine, it is not enough to answer "is this you" anywhere else,
//! and the page says so rather than implying an account it does not have.

use std::fmt::Write as _;

use carranta_core::state::TradeMode;

use crate::report::CSS;
use crate::store::Saved;

/// One table the server is holding, as the home page needs it.
pub struct Open {
    pub id: String,
    /// The name the table was given, or empty.
    pub game: String,
    /// What the person who dealt it calls themselves, or empty.
    pub host: String,
    pub seats: u8,
    pub mode: TradeMode,
    /// Whether it was dealt as a listed table.
    pub public: bool,
    /// Whether anybody has moved on it yet. A table can sit dealt and untouched,
    /// and "on turn 1" is a poor way of saying so.
    pub started: bool,
    /// The turn it is on, counting from one, which is how far along it is. Turns
    /// rather than moves, because that is the unit the report counts a game in.
    pub turns: u32,
    pub winner: Option<u8>,
    /// Milliseconds since it was dealt.
    pub age: u64,
    /// Whether the visitor asking is the one who dealt it.
    pub mine: bool,
}

/// The page, rendered.
///
/// `open` is every table in memory, newest first; `mine` and `others` are what
/// the store has, already split by whose they are.
pub fn page(open: &[Open], mine: &[Saved], others: &[Saved], name: &str) -> String {
    let mut b = String::new();
    let _ = write!(
        b,
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>Carranta</title><style>{CSS}{EXTRA}</style></head><body>\
         <header><a class=\"mark\" href=\"/\">Carranta</a></header><main>\
         <h1>Carranta</h1>\
         <p class=\"lede\">Settle an island, trade for what the dice would not \
         give you, and be first to ten points.</p>"
    );

    b.push_str(&deal(name));
    b.push_str(&joining(open));
    b.push_str(&played(mine, others));
    b.push_str("</main></body></html>");
    b
}

/// The card that deals a table.
///
/// A form rather than a script, so it works before anything has loaded and the
/// browser remembers what you typed. Every field has a default that plays a
/// normal game, so the button on its own is a whole answer.
fn deal(name: &str) -> String {
    let mut b = String::from("<section>");
    b.push_str(&head(
        "New game",
        "You take a seat and the rest of the table is played by the house bot. \
         Four seats is the game most people mean; three is faster and a little \
         sharper, since the same board is shared between fewer players.",
    ));
    let _ = write!(
        b,
        "<form class=\"deal\" method=\"post\" action=\"/new\">\
         <label class=\"field\"><span>your name</span>\
         <input name=\"name\" value=\"{}\" maxlength=\"24\" placeholder=\"you\"></label>\
         <label class=\"field\"><span>table name</span>\
         <input name=\"game\" maxlength=\"32\" placeholder=\"optional\"></label>\
         <label class=\"field\"><span>seats</span>\
         <select name=\"seats\"><option value=\"4\">4</option>\
         <option value=\"3\">3</option></select></label>\
         <label class=\"field\"><span>listing</span>\
         <select name=\"visibility\">\
         <option value=\"private\">just me</option>\
         <option value=\"public\">listed below</option></select></label>\
         <label class=\"field\"><span>bots move</span>\
         <select name=\"pace\">\
         <option value=\"lively\">at a lively pace</option>\
         <option value=\"calm\">calmly</option>\
         <option value=\"instant\">at once</option></select></label>\
         <button class=\"go\" type=\"submit\">Deal a table</button></form>",
        esc(name)
    );
    b.push_str("</section>");
    b
}

/// The card that lists tables you could open.
fn joining(open: &[Open]) -> String {
    let mut b = String::from("<section>");
    b.push_str(&head(
        "Tables",
        "Games this server is holding in memory. A listed table is one whose \
         host chose to publish it; your own show up whether or not you did. One \
         seat is a person and the rest are bots, so joining a table somebody \
         else dealt puts you at the same seat as them for now: seats for a \
         second person are the next thing to build, and this list is what they \
         will be reached from.",
    ));
    // Somewhere to sit, which a finished game is not: a game with a winner is
    // history, and history is the card below. Your own unlisted tables are here
    // because you have to be able to get back to a game you dealt; other
    // people's are here only if they published them.
    let live: Vec<&Open> = open
        .iter()
        .filter(|t| t.winner.is_none() && (t.mine || t.public))
        .collect();
    if live.is_empty() {
        b.push_str(
            "<p class=\"note\">No tables. Deal one above and it will be here \
             until somebody wins it.</p>",
        );
        b.push_str("</section>");
        return b;
    }
    b.push_str(TABLE_OPEN);
    b.push_str(
        "<thead><tr><th></th><th>seats</th><th>market</th><th>played</th>\
                <th>dealt</th><th></th></tr></thead><tbody>",
    );
    for t in live {
        // Which turn it is on rather than how many are done, because that is
        // what a live table has: a game in its seventh turn is on turn seven.
        let played = if t.started {
            format!("turn {}", t.turns)
        } else {
            "not started".to_string()
        };
        let _ = write!(
            b,
            "<tr><td>{name} <span class=\"tag quiet\">{tag}</span></td>\
             <td>{seats}</td><td>{mode}</td><td>{played}</td><td>{age}</td>\
             <td class=\"act\">\
             <a class=\"go small\" href=\"/{id}/\">Sit down</a></td></tr>",
            name = table_name(t),
            // Yours before listed: a table of your own is on this list whether
            // you published it or not, so that is the more useful of the two.
            tag = if t.mine { "yours" } else { "listed" },
            seats = t.seats,
            mode = market(t.mode),
            age = ago(t.age),
            id = t.id,
        );
    }
    b.push_str("</tbody>");
    b.push_str(TABLE_CLOSE);
    b.push_str("</section>");
    b
}

/// The card that lists games already played.
fn played(mine: &[Saved], others: &[Saved]) -> String {
    let mut b = String::from("<section>");
    b.push_str(&head(
        "Your games",
        "Games dealt by this browser, newest first. Held against a key in a \
         cookie rather than an account, so they follow the browser and not you: \
         another browser, or this one with its cookies cleared, is somebody else \
         as far as this page can tell. Accounts are what fix that, and the key is \
         stored in a way that lets an account claim it later.",
    ));
    if mine.is_empty() {
        b.push_str(
            "<p class=\"note\">None yet. A game arrives here when it ends; until \
             then it is a table above.</p>",
        );
    } else {
        b.push_str(&list(mine));
    }
    b.push_str("</section>");

    if !others.is_empty() {
        b.push_str("<section>");
        b.push_str(&head(
            "Also on this server",
            "Every other game in the store: games dealt in another browser, and \
             the ones the server played itself so that the analytics have \
             something to read. Anybody who can reach this server can read them, \
             which is what a loopback server on your own machine means.",
        ));
        b.push_str(&list(others));
        b.push_str("</section>");
    }
    b
}

/// Games listed at once, newest first.
///
/// A cap rather than the lot, because this page is read at a glance and a
/// hundredth row is not read at all. Nothing is lost by it: every game keeps its
/// address, and a page that answers "across all of them" is the corpus page's
/// job rather than this one's.
const SHOWN: usize = 24;

/// A table of games, board and analytics either side of the result.
fn list(games: &[Saved]) -> String {
    let mut b = String::from(TABLE_OPEN);
    b.push_str(
        "<thead><tr><th></th><th>seats</th><th>market</th><th>turns</th>\
         <th>result</th><th></th></tr></thead><tbody>",
    );
    for g in games.iter().take(SHOWN) {
        let result = match g.winner {
            // Seat nought is whoever was at the keyboard.
            Some(0) => "<span class=\"up\">won</span>".to_string(),
            Some(w) => format!("seat {w} won"),
            None => "<span class=\"worth\">unfinished</span>".to_string(),
        };
        // One of the two is the thing to do and the other is beside it. Which is
        // which depends on the game: an unfinished one is waiting to be played,
        // and a finished one has nothing left but what it says.
        let over = g.winner.is_some();
        let board = if over { " quiet" } else { "" };
        let study = if over { "" } else { " quiet" };
        let _ = write!(
            b,
            "<tr><td>{name}</td><td>{seats}</td><td>{mode}</td><td>{turns}</td>\
             <td>{result}</td><td class=\"act\">\
             <a class=\"go small{board}\" href=\"/{id}/\">Board</a> \
             <a class=\"go small{study}\" href=\"/{id}/analytics\">Analytics</a>\
             </td></tr>",
            name = named(g),
            seats = g.seats,
            mode = market(g.mode),
            // Turns rather than moves: the same figure the analytics count by.
            turns = turns_of(g),
            id = g.id,
        );
    }
    b.push_str("</tbody>");
    b.push_str(TABLE_CLOSE);
    if games.len() > SHOWN {
        let _ = write!(
            b,
            "<p class=\"note\">And {} older, still on disk and still at their \
             own addresses. This page shows the newest {SHOWN}.</p>",
            games.len() - SHOWN
        );
    }
    b
}

/// Turns in a saved game, counted the way the analytics count them.
fn turns_of(g: &Saved) -> usize {
    use crate::game::Step;
    use carranta_core::action::Action;
    g.moves
        .iter()
        .filter(|s| matches!(s, Step::Move(Action::EndTurn)))
        .count()
}

/// A card's heading, with its rule behind it, as the report does it.
fn head(title: &str, why: &str) -> String {
    format!(
        "<div class=\"card-head\"><h2 data-tip=\"{}\">{title}</h2></div>",
        esc(why)
    )
}

fn table_name(t: &Open) -> String {
    let host = if t.host.is_empty() {
        "somebody"
    } else {
        &t.host
    };
    if t.game.is_empty() {
        format!("<strong>{}</strong>'s table", esc(host))
    } else {
        format!(
            "<strong>{}</strong> <span class=\"worth\">{}</span>",
            esc(&t.game),
            esc(host)
        )
    }
}

fn named(g: &Saved) -> String {
    if g.name.is_empty() {
        format!("<span class=\"worth\">{}</span>", esc(&g.id))
    } else {
        format!(
            "<strong>{}</strong> <span class=\"worth\">{}</span>",
            esc(&g.name),
            esc(&g.id)
        )
    }
}

fn market(mode: TradeMode) -> &'static str {
    match mode {
        TradeMode::Full => "open",
        TradeMode::Restricted => "one for one",
        TradeMode::Disabled => "none",
    }
}

/// How long ago, in the roundest unit that still says something.
fn ago(millis: u64) -> String {
    let secs = millis / 1000;
    match secs {
        0..=45 => "just now".to_string(),
        46..=5400 => format!("{} min ago", (secs + 30) / 60),
        _ => format!("{} h ago", (secs + 1800) / 3600),
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const TABLE_OPEN: &str = "<div class=\"tw\"><table>";
const TABLE_CLOSE: &str = "</table></div>";

/// What this page needs on top of the report's stylesheet.
///
/// The tokens, the card, the table and the tooltip are all already there and are
/// the same design; a form and a button are not, because the report has neither.
const EXTRA: &str = "
main { padding-top: 1rem; }
h1 { margin-top: .2em; }
/* The form: a row of fields that wraps, and a button that never wraps away from
   them. Labels above their inputs, because a label beside a field on a narrow
   screen is a field two characters wide. */
.deal { display: flex; flex-wrap: wrap; gap: .75rem 1rem; align-items: flex-end; }
.field { display: flex; flex-direction: column; gap: .3rem; font-size: 13px;
         color: var(--muted-foreground); }
.field input, .field select {
  font: 400 15px Figtree, system-ui, sans-serif; color: var(--foreground);
  background: var(--card); border: 1px solid var(--border);
  border-radius: var(--radius-md); padding: .45em .6em; min-width: 9rem; }
.field input:focus-visible, .field select:focus-visible {
  outline: 2px solid var(--primary); outline-offset: 1px; }
/* The one button on the page that starts something, in the colour the win is
   written in, and the same shape as a place badge so the family holds. */
.go { display: inline-block; text-decoration: none; cursor: pointer;
      font: 600 15px Figtree, system-ui, sans-serif;
      background: var(--primary); color: var(--primary-foreground);
      border: 1px solid var(--primary); border-radius: var(--radius-md);
      padding: .5em 1.1em; }
.go:hover { filter: brightness(1.06); }
.go:focus-visible { outline: 2px solid var(--foreground); outline-offset: 2px; }
.go.small { font-size: 13px; padding: .35em .7em; }
/* A second action beside a first is quieter: same shape, the page's own ink. */
.go.quiet { background: var(--card); color: var(--muted-foreground);
            border-color: var(--border); }
.go.quiet:hover { color: var(--foreground); border-color: var(--muted-foreground); }
/* The actions column holds buttons rather than figures, so it is left alone by
   the table's right alignment and never wraps. */
td.act { text-align: right; white-space: nowrap; }
";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Step;
    use carranta_core::action::Action;

    fn table(id: &str, public: bool, mine: bool, winner: Option<u8>) -> Open {
        Open {
            id: id.to_string(),
            game: String::new(),
            host: "Egon".to_string(),
            seats: 4,
            mode: TradeMode::Full,
            public,
            started: true,
            turns: 3,
            winner,
            age: 0,
            mine,
        }
    }

    fn game(id: &str, name: &str, winner: Option<u8>) -> Saved {
        Saved {
            id: id.to_string(),
            seats: 4,
            seed: 1,
            mode: TradeMode::Full,
            name: name.to_string(),
            by: String::new(),
            dealt: 0,
            winner,
            moves: vec![
                Step::Move(Action::Roll),
                Step::Move(Action::EndTurn),
                Step::Move(Action::Roll),
                Step::Move(Action::EndTurn),
            ],
            times: Vec::new(),
        }
    }

    #[test]
    fn the_page_offers_the_three_things_it_is_for() {
        let html = page(&[], &[], &[], "");
        // Deal one, and by a form, because there is no script here to post for
        // it: a button that needs one is a button that does nothing.
        assert!(html.contains("method=\"post\" action=\"/new\""));
        assert!(html.contains("Deal a table"));
        assert!(html.contains(">Tables</h2>"));
        assert!(html.contains(">Your games</h2>"));
        // The rule of this page and of the report: no script at all.
        assert!(!html.contains("<script"), "no script on the home page");
        assert!(!html.contains("onclick"));
    }

    #[test]
    fn an_empty_server_says_so_rather_than_showing_nothing() {
        let html = page(&[], &[], &[], "");
        assert!(html.contains("No tables."));
        assert!(html.contains("None yet."));
        // And there is nothing to say about other people's games when there are
        // none, so that card is absent rather than empty.
        assert!(!html.contains("Also on this server"));
    }

    #[test]
    fn a_private_table_is_listed_to_its_own_host_and_to_nobody_else() {
        let mine = table("aaaa-aaaa-aaaa", false, true, None);
        let theirs = table("bbbb-bbbb-bbbb", false, false, None);
        let listed = table("cccc-cccc-cccc", true, false, None);
        let html = page(&[mine, theirs, listed], &[], &[], "");
        assert!(html.contains("aaaa-aaaa-aaaa"), "my own unlisted table");
        assert!(
            !html.contains("bbbb-bbbb-bbbb"),
            "somebody else's unlisted table is not mine to see"
        );
        assert!(html.contains("cccc-cccc-cccc"), "a listed table");
    }

    #[test]
    fn a_finished_table_is_not_offered_as_somewhere_to_sit() {
        // Over is over, whoever dealt it. The table card is somewhere to sit and
        // there is nothing to do at a game somebody has won; it belongs to the
        // history card, which reads it off the store rather than out of memory.
        for mine in [true, false] {
            let over = table("dddd-dddd-dddd", true, mine, Some(1));
            let html = page(&[over], &[], &[], "");
            assert!(!html.contains("dddd-dddd-dddd"), "mine: {mine}");
            assert!(html.contains("No tables."));
        }
    }

    #[test]
    fn a_played_game_offers_its_board_and_its_report() {
        let g = game("ffff-ffff-ffff", "Egon", Some(0));
        let html = page(&[], std::slice::from_ref(&g), &[], "Egon");
        assert!(html.contains("href=\"/ffff-ffff-ffff/\""));
        assert!(html.contains("href=\"/ffff-ffff-ffff/analytics\""));
        // Turns, counted the way the report counts them, not moves.
        assert_eq!(turns_of(&g), 2);
        assert!(html.contains("<td>2</td>"));
        // Seat nought is whoever was at the keyboard, so their win is a win.
        assert!(html.contains(">won</span>"));
        // And the name already given is in the form, so a second game does not
        // ask for it again.
        assert!(html.contains("value=\"Egon\""));
    }

    #[test]
    fn other_peoples_games_get_their_own_card() {
        let theirs = [game("1111-1111-1111", "", None)];
        let html = page(&[], &[], &theirs, "");
        assert!(html.contains("Also on this server"));
        assert!(html.contains("1111-1111-1111"));
        assert!(html.contains(">unfinished</span>"));
    }

    #[test]
    fn a_long_history_is_capped_and_says_that_it_is() {
        let games: Vec<Saved> = (0..SHOWN + 3)
            .map(|i| game(&format!("{:04}-0000-0000", i + 1000), "Egon", Some(0)))
            .collect();
        let html = page(&[], &games, &[], "Egon");
        assert!(html.contains("1000-0000-0000"), "the newest is shown");
        let last = format!("{:04}-0000-0000", SHOWN + 1002);
        assert!(!html.contains(&last), "the oldest is not");
        assert!(html.contains("And 3 older"));
    }

    #[test]
    fn a_name_from_a_player_cannot_write_html() {
        // The one field on this page that a person fills in, and it is echoed
        // back into the form. A table name goes the same way.
        let html = page(&[], &[], &[], "<script>alert(1)</script>");
        assert!(!html.contains("<script"));
        assert!(html.contains("&lt;script&gt;"));
        let sneaky = table("2222-2222-2222", true, false, None);
        let sneaky = Open {
            host: "\" onfocus=\"x".to_string(),
            ..sneaky
        };
        let html = page(&[sneaky], &[], &[], "");
        assert!(!html.contains("onfocus=\"x"));
        assert!(html.contains("&quot; onfocus=&quot;x"));
    }

    #[test]
    fn how_long_ago_is_said_in_one_unit() {
        assert_eq!(ago(0), "just now");
        assert_eq!(ago(45_000), "just now");
        assert_eq!(ago(60_000), "1 min ago");
        assert_eq!(ago(90_000), "2 min ago");
        assert_eq!(ago(3_600_000), "60 min ago");
        assert_eq!(ago(7_200_000), "2 h ago");
    }
}
