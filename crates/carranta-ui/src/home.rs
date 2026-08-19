//! The home page: where a game comes from.
//!
//! Before this there was nowhere to stand. The root redirected to whatever game
//! the server happened to be holding, the only way to deal another was a panel
//! inside a game you were already in, and a game you had finished was reachable
//! only if you had kept the link. The board page's wordmark said as much in a
//! comment: it opened the lobby because there was no home to go to.
//!
//! Three sections, in the order somebody wants them: start one, join one, or
//! look at one you played. Links only, and **no script at all**, like the
//! analytics page and for the same reason: nothing here changes without a
//! request, so there is nothing for a script to do. Starting one is a link to
//! the lobby rather than a form of its own, because the lobby already asks
//! everything a table needs and two half-forms would drift apart.
//!
//! Whose games are whose comes from a cookie, which is honest about what it is:
//! a key handed to a browser, not a login. It is enough to answer "show me
//! mine" on one machine, it is not enough to answer "is this you" anywhere else,
//! and the page says so rather than implying an account it does not have.

use std::fmt::Write as _;

use carranta_core::state::TradeMode;

use crate::analysis::Finishing;
use crate::report::{CSS, ICON};
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
    /// Seats somebody arriving could take: chairs the host held open and chairs
    /// the bots are keeping warm, while the game has not begun. Not the same as
    /// what the table is waiting for, which is the held ones alone.
    pub takeable: usize,
    /// Whether the visitor asking is already sitting at it.
    pub seated: bool,
}

/// The page, rendered.
///
/// `open` is every table in memory, newest first; `mine` is what the store has
/// that belongs to whoever is asking.
/// Who is reading, as far as the header needs to know.
///
/// Three states rather than two, and the third is the ordinary one: a server
/// with no provider configured offers nothing at all rather than a button that
/// leads to a four hundred and four. That is what a checkout without secrets in
/// it looks like, and it has to look like a whole application rather than a
/// broken one.
pub struct Who {
    /// Whether this server can sign anybody in.
    pub offered: bool,
    /// Whether this reader is signed in.
    pub signed_in: bool,
    /// What to call them, when they have said.
    pub name: String,
}

/// The account strip, at the top right, and the only place accounts appear.
///
/// Deliberately small and deliberately not on the way to anything. Signing in
/// buys one thing, your games following you to another machine, and a page that
/// asked for it before letting somebody play would be charging for a game in
/// advance of showing it. Guests play everything.
fn account(who: &Who) -> String {
    if !who.offered {
        return String::new();
    }
    if !who.signed_in {
        return "<a class=\"headLink\" href=\"/signin\">Sign in</a>".to_string();
    }
    let called = match who.name.trim() {
        "" => "Signed in".to_string(),
        name => esc(name),
    };
    // A form rather than a link, because signing out changes something and a
    // link that changes something is a link a prefetcher can press. No script:
    // this page renders on the server and a button in a form needs none.
    format!(
        "<span class=\"headWho\">{called}</span>\
         <form class=\"headOut\" method=\"post\" action=\"/signout\">\
         <button class=\"headLink\" type=\"submit\">Sign out</button></form>"
    )
}

pub fn page(open: &[Open], mine: &[Saved], who: &Who, staying: Finishing) -> String {
    let mut b = String::new();
    let _ = write!(
        b,
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>Carranta</title>{ICON}\
         <style>{CSS}{EXTRA}</style></head><body>\
         {head}<main>",
        // No links. Every other page's header offers a way back to this one and a
        // way to a new game; this page is the way back, and the button below is
        // the way to a new game, said properly and in the place the eye lands. A
        // header link to the page's own first button is furniture.
        head = crate::report::masthead_home(&[], &account(who)),
    );

    b.push_str(&deal());
    b.push_str(&joining(open));
    b.push_str(&played(mine, staying, who.signed_in));
    b.push_str("</main></body></html>");
    b
}

/// What the page is for: one button, and three lines saying what is behind it.
///
/// Not a card. The card is the shape this page uses for a list of things, and a
/// list of one thing dressed as a list reads as the first of several: the button
/// sat inside a bordered box below a heading and a paragraph, third in the
/// reading order of its own section. Here it is the first thing on the page,
/// which is what it is.
///
/// The application's own button, at the application's own size. It was briefly
/// scaled up to say "this is the important one", which is a thing position and
/// emptiness already say: a button alone at the top of an otherwise empty page
/// is not competing with anything, so the extra size was volume rather than
/// emphasis, and it made the one control on the front page the one control that
/// does not look like the rest of them.
///
/// It does not deal anything either: it leads to the lobby, which is where the
/// settings live and always did. Two forms asking overlapping halves of the same
/// question is one too many, and the half here was the smaller one.
///
/// The three lines under it are the three reasons to press it, one clause each,
/// in the order somebody meets them: the game, then what the server keeps of it,
/// then who else is at the table. Long enough to say something and short enough
/// to be read without deciding to read them.
fn deal() -> String {
    let mut b = String::from("<section class=\"hero\">");
    b.push_str(
        "<a class=\"go\" href=\"/lobby\">New game</a>\
         <ul class=\"claims\">\
         <li>Settle an island, trade, and strategize.</li>\
         <li>Analyze your games with advanced analytics.</li>\
         <li>Play humans and the best AI in the world.</li>\
         </ul>",
    );
    b.push_str("</section>");
    b
}

/// The card that lists tables you could open, or nothing at all.
///
/// Nothing at all is the point of the early return. A card saying "no tables"
/// is a hole in the page on the one visit where the page has the least to say
/// and the most to prove: somebody arriving at an idle server was shown two
/// empty boxes under the button, which is a server with nothing on it rather
/// than a game to play. What is not there is not mentioned.
fn joining(open: &[Open]) -> String {
    // Somewhere to sit, which a finished game is not: a game with a winner is
    // history, and history is the card below. Your own unlisted tables are here
    // because you have to be able to get back to a game you dealt, and so are
    // ones you are sitting at, because a private table somebody invited you to
    // is a game of yours whoever dealt it. Other people's are here only if they
    // published them.
    let live: Vec<&Open> = open
        .iter()
        .filter(|t| t.winner.is_none() && (t.mine || t.seated || t.public))
        .collect();
    if live.is_empty() {
        return String::new();
    }
    let mut b = String::from("<section>");
    b.push_str(&head(
        "Tables",
        "Games this server is holding in memory. A listed table is one whose \
         host chose to publish it; your own show up whether or not you did. A \
         table with a chair nobody is in says so, and that is the row to take \
         if you came here to play somebody.",
    ));
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
        // Both when both, because they answer different questions and one does
        // not imply the other. Yours says why it is on your list; listed says it
        // is on everybody's, which is the answer that cannot be taken back and
        // therefore the one worth saying out loud.
        let mut tags = String::new();
        if t.mine {
            tags.push_str("<span class=\"tag quiet\">yours</span> ");
        }
        if t.public {
            tags.push_str("<span class=\"tag quiet\">listed</span> ");
        }
        // The one that decides whether this row is an invitation. Loud rather
        // than quiet: it is the only thing on this page you can be too late for.
        if t.takeable > 0 && !t.seated {
            let _ = write!(
                tags,
                "<span class=\"tag\">{}</span> ",
                if t.takeable == 1 {
                    "a seat free".to_string()
                } else {
                    format!("{} seats free", t.takeable)
                }
            );
        }
        let _ = write!(
            b,
            "<tr><td>{name} {tags}</td>\
             <td>{seats}</td><td>{mode}</td><td>{played}</td><td>{age}</td>\
             <td class=\"act\">\
             <a class=\"go small{quiet}\" href=\"/{id}/\">{go}</a></td></tr>",
            name = table_name(t),
            seats = t.seats,
            mode = market(t.mode),
            age = ago(t.age),
            id = t.id,
            // Sitting down is what a free seat offers and what a table you are
            // already at does not: going back to your own game is not joining
            // it, and saying so twice would make the loud word meaningless.
            go = if t.seated {
                "Back to it"
            } else if t.takeable > 0 {
                "Sit down"
            } else {
                "Watch"
            },
            quiet = if t.seated || t.takeable > 0 {
                ""
            } else {
                " quiet"
            },
        );
    }
    b.push_str("</tbody>");
    b.push_str(TABLE_CLOSE);
    b.push_str("</section>");
    b
}

/// The card that lists games already played, or nothing at all.
///
/// Only this visitor's. There was a second card under it listing every other game
/// in the store, which was a browsable pile of other people's games on the front
/// page: interesting while the store held six demo games and nothing else, and
/// not a section anybody wants once it holds theirs.
///
/// Absent rather than empty, for the same reason the table list is: a first
/// visit has no history and does not need to be told twice that it has none.
/// The page grows as somebody plays, and starts as the one thing they came for.
fn played(mine: &[Saved], staying: Finishing, signed_in: bool) -> String {
    if mine.is_empty() {
        return String::new();
    }
    let mut b = String::from("<section>");
    b.push_str(&head(
        "Your games",
        if signed_in {
            "Your games, newest first, wherever you played them. Signing in on \
             another machine brings that machine's games here too."
        } else {
            "Games from this browser, newest first. They are held against a \
             cookie, so they follow the browser rather than you: another \
             browser, or this one with its cookies cleared, is somebody else as \
             far as this page can tell. Signing in is what fixes that, and it \
             brings these with it."
        },
    ));
    b.push_str(&list(mine));
    b.push_str(&staying_line(staying));
    b.push_str("</section>");
    b
}

/// Below how many games the record is noise rather than a record.
///
/// One abandoned first game should not brand anybody, and a page that says
/// "0 of 1" is reporting a coin toss as a habit.
const ENOUGH_TO_SAY: u32 = 3;

/// Whether you stay at the tables you sit down at.
///
/// The number the rating deliberately does not carry, so it is said here, plainly
/// and as a count. Walking out costs you the place you finished in, which is the
/// rating's business; how often you do it is this line's, and answering both with
/// one number would answer neither.
///
/// Yours only. It would be easy to put this beside everybody's name at a table
/// and much harder to undo: a number about somebody else's reliability, on a
/// screen they cannot see, is a thing people would use on each other. If it ever
/// goes public it should be a decision rather than a side effect of this line.
fn staying_line(staying: Finishing) -> String {
    if staying.played() < ENOUGH_TO_SAY {
        return String::new();
    }
    if staying.left == 0 {
        return format!(
            "<p class=\"note staying\">You finished all {} of your games.</p>",
            staying.played()
        );
    }
    format!(
        "<p class=\"note staying\">You were at the table at the end of {} of \
         your {} games.</p>",
        staying.stayed,
        staying.played()
    )
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
/// The tokens, the header, the card, the table and the tooltip are all already
/// there and are the same design. What is not: a button, which the report has
/// none of.
const EXTRA: &str = "
/* No h1 in the column: on this page the mark in the header *is* the heading, and
   a second large `Carranta` under a small one is the logo twice. The button
   opens the page instead, because the button is what the page is for.

   Centred, and the only centred thing in the application: everywhere else a
   column of text is read from its left edge, and here there is no column to
   read. Given air above it rather than sat under the header, so the eye lands
   on it and not on the first thing that happens to be at the top. */
.hero { border: 0; background: none; box-shadow: none; padding: 0;
        margin: clamp(2.5rem, 9vh, 6rem) 0 clamp(1rem, 3vh, 2rem);
        text-align: center; }
/* Three lines, one clause each, under the thing they are reasons for. A list
   because they are a list, without the bullets, which would make three items
   out of what reads as three sentences.

   The page's own size, not a size of its own. Scaling them with the viewport
   made the front page the one screen in the application whose body text is not
   the body text, which is a different typeface decision every time the window
   moves. */
.claims { list-style: none; margin: 1.6rem 0 0; padding: 0;
          display: grid; gap: .5rem;
          color: var(--muted-foreground); }
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

    /// A reader on a server with no sign-in configured, which is what a
    /// checkout is and what most of these tests are about.
    fn nobody() -> Who {
        Who {
            offered: false,
            signed_in: false,
            name: String::new(),
        }
    }

    #[test]
    fn the_account_strip_says_one_of_three_things() {
        // A server with nothing configured offers nothing at all, rather than a
        // button that leads to a four hundred and four. That is what a checkout
        // looks like, and it has to look like a whole application.
        let html = page(&[], &[], &nobody(), Finishing::default());
        assert!(!html.contains("Sign in"), "nothing offered");
        assert!(!html.contains("/signout"));

        // Offered, and not taken up.
        let out = page(
            &[],
            &[],
            &Who {
                offered: true,
                signed_in: false,
                name: String::new(),
            },
            Finishing::default(),
        );
        assert!(out.contains("href=\"/signin\">Sign in</a>"));
        assert!(!out.contains("/signout"), "nothing to sign out of");

        // Taken up. A form rather than a link, because signing out changes
        // something and a link that changes something is one a prefetcher can
        // press.
        let inn = page(
            &[],
            &[],
            &Who {
                offered: true,
                signed_in: true,
                name: "Egon".to_string(),
            },
            Finishing::default(),
        );
        assert!(inn.contains(">Egon</span>"), "named");
        assert!(inn.contains("method=\"post\" action=\"/signout\""));
        assert!(!inn.contains("href=\"/signin\""), "already in");
        // And no script anywhere near it: this page renders on the server.
        assert!(
            !inn[..inn.find("</header>").expect("a header")].contains("<script"),
            "the header needs none"
        );

        // Signed in and never named. Not "Player 1", which is a seat's word.
        let unnamed = page(
            &[],
            &[],
            &Who {
                offered: true,
                signed_in: true,
                name: "   ".to_string(),
            },
            Finishing::default(),
        );
        assert!(unnamed.contains(">Signed in</span>"));
        assert!(unnamed.contains("/signout"));
    }

    #[test]
    fn whether_you_stay_is_said_once_there_is_something_to_say() {
        let games = [game("bbbb-bbbb-bbbb", "Egon", Some(0))];
        let line = |stayed, left| {
            let html = page(&[], &games, &nobody(), Finishing { stayed, left });
            html.find("staying").map(|at| {
                let rest = &html[at..];
                let start = rest.find('>').expect("a tag") + 1;
                rest[start..start + rest[start..].find('<').expect("a close")].to_string()
            })
        };
        // One abandoned first game should not brand anybody, and "0 of 1" is a
        // coin toss reported as a habit.
        assert_eq!(line(0, 1), None, "too early to say");
        assert_eq!(line(1, 1), None);
        // A clean record says so as a whole number rather than as a fraction of
        // itself, because "3 of 3" is a sentence that makes somebody check.
        assert_eq!(
            line(3, 0).as_deref(),
            Some("You finished all 3 of your games.")
        );
        assert_eq!(
            line(4, 2).as_deref(),
            Some("You were at the table at the end of 4 of your 6 games.")
        );
        // And it is nowhere at all when there are no games to list, because the
        // section it belongs to is not there either.
        assert!(
            !page(&[], &[], &nobody(), Finishing { stayed: 9, left: 1 }).contains("staying"),
            "no games, no section, no line"
        );
    }

    #[test]
    fn a_name_in_the_header_is_text_rather_than_markup() {
        // It is somebody else's text, typed into a seat and kept on a person.
        let html = page(
            &[],
            &[],
            &Who {
                offered: true,
                signed_in: true,
                name: "<script>alert(1)</script>".to_string(),
            },
            Finishing::default(),
        );
        assert!(!html.contains("<script>alert"), "escaped");
        assert!(html.contains("&lt;script&gt;alert"));
    }
    use crate::game::Step;
    use crate::store::Setup;
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
            takeable: 0,
            seated: mine,
        }
    }

    /// A table with a chair nobody is in.
    fn waiting_table(id: &str, seated: bool) -> Open {
        Open {
            takeable: 2,
            seated,
            ..table(id, true, false, None)
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
            setup: Setup::default(),
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
    fn the_page_leads_with_the_one_thing_it_is_for() {
        let html = page(&[], &[], &nobody(), Finishing::default());
        // One button, and it leads to the lobby rather than dealing anything: the
        // settings live there, and a second form here would be half of them.
        assert!(html.contains("href=\"/lobby\">New game</a>"));
        assert!(!html.contains("<form"), "the settings are the lobby's");
        // Three lines under it, the three reasons to press it.
        assert!(html.contains("Settle an island, trade, and strategize."));
        assert!(html.contains("Analyze your games with advanced analytics."));
        assert!(html.contains("Play humans and the best AI in the world."));
        // The button is first: it is what the page is for, and anything above it
        // is something read before the thing somebody came to do.
        let button = html.find("href=\"/lobby\"").expect("the button");
        let first_claim = html.find("Settle an island").expect("the claims");
        assert!(button < first_claim, "the button, then the reasons");
        // The rule of this page and of the report: no script at all.
        assert!(!html.contains("<script"), "no script on the home page");
        assert!(!html.contains("onclick"));
    }

    #[test]
    fn an_empty_server_is_the_button_and_nothing_else() {
        // Two cards saying they have nothing in them is a hole in the page on
        // the one visit where it has the least to say. What is not there is not
        // mentioned; the sections arrive as somebody plays.
        let html = page(&[], &[], &nobody(), Finishing::default());
        assert!(!html.contains(">Tables</h2>"), "no empty table list");
        assert!(!html.contains(">Your games</h2>"), "and no empty history");
        assert!(!html.contains("Also on this server"));
        assert!(html.contains("New game"), "only the way to start one");
        // And they arrive the moment there is something to put in them.
        assert!(
            page(
                &[table("aaaa-aaaa-aaaa", true, false, None)],
                &[],
                &nobody(),
                Finishing::default()
            )
            .contains(">Tables</h2>")
        );
        assert!(
            page(
                &[],
                &[game("bbbb-bbbb-bbbb", "Egon", Some(0))],
                &nobody(),
                Finishing::default()
            )
            .contains(">Your games</h2>")
        );
    }

    #[test]
    fn a_private_table_is_listed_to_its_own_host_and_to_nobody_else() {
        let mine = table("aaaa-aaaa-aaaa", false, true, None);
        let theirs = table("bbbb-bbbb-bbbb", false, false, None);
        let listed = table("cccc-cccc-cccc", true, false, None);
        let html = page(
            &[mine, theirs, listed],
            &[],
            &nobody(),
            Finishing::default(),
        );
        assert!(html.contains("aaaa-aaaa-aaaa"), "my own unlisted table");
        assert!(
            !html.contains("bbbb-bbbb-bbbb"),
            "somebody else's unlisted table is not mine to see"
        );
        assert!(html.contains("cccc-cccc-cccc"), "a listed table");
        // Mine and published says both: one says why it is on my list, the other
        // says it is on everybody's, and that is the half that cannot be undone.
        let both = table("eeee-eeee-eeee", true, true, None);
        let row = page(&[both], &[], &nobody(), Finishing::default());
        let row = row
            .split("eeee-eeee-eeee")
            .next()
            .expect("a row before the link");
        assert!(row.contains(">yours</span>"));
        assert!(row.contains(">listed</span>"));
    }

    #[test]
    fn a_table_with_a_chair_free_says_so_and_offers_it() {
        // The one thing on this page you can be too late for, so it is the one
        // tag that is not quiet.
        let html = page(
            &[waiting_table("aaaa-aaaa-aaaa", false)],
            &[],
            &nobody(),
            Finishing::default(),
        );
        assert!(html.contains(">2 seats free</span>"));
        assert!(html.contains(">Sit down</a>"));
        // One reads as one.
        let one = Open {
            takeable: 1,
            ..waiting_table("bbbb-bbbb-bbbb", false)
        };
        assert!(page(&[one], &[], &nobody(), Finishing::default()).contains(">a seat free</span>"));
        // Already in it: going back to your own game is not joining it, and
        // saying so twice would make the loud word mean nothing.
        let html = page(
            &[waiting_table("cccc-cccc-cccc", true)],
            &[],
            &nobody(),
            Finishing::default(),
        );
        assert!(html.contains(">Back to it</a>"));
        assert!(!html.contains("seats free"));
        // A full table is somewhere to watch, quietly.
        let full = Open {
            takeable: 0,
            seated: false,
            ..waiting_table("dddd-dddd-dddd", false)
        };
        let html = page(&[full], &[], &nobody(), Finishing::default());
        assert!(html.contains(">Watch</a>"));
        assert!(html.contains("go small quiet"));
    }

    #[test]
    fn a_table_you_are_sitting_at_is_on_your_page_whoever_dealt_it() {
        // Somebody else's private table that you were invited to. Not yours,
        // not listed, and yours to get back to.
        let theirs = Open {
            public: false,
            mine: false,
            seated: true,
            takeable: 0,
            ..table("eeee-eeee-eeee", false, false, None)
        };
        let html = page(&[theirs], &[], &nobody(), Finishing::default());
        assert!(html.contains("eeee-eeee-eeee"), "it is on their page");
        assert!(
            !html.contains(">yours</span>"),
            "without claiming they dealt it"
        );
    }

    #[test]
    fn a_finished_table_is_not_offered_as_somewhere_to_sit() {
        // Over is over, whoever dealt it. The table card is somewhere to sit and
        // there is nothing to do at a game somebody has won; it belongs to the
        // history card, which reads it off the store rather than out of memory.
        for mine in [true, false] {
            let over = table("dddd-dddd-dddd", true, mine, Some(1));
            let html = page(&[over], &[], &nobody(), Finishing::default());
            assert!(!html.contains("dddd-dddd-dddd"), "mine: {mine}");
            assert!(!html.contains(">Tables</h2>"), "and no list at all");
        }
    }

    #[test]
    fn a_played_game_offers_its_board_and_its_report() {
        let g = game("ffff-ffff-ffff", "Egon", Some(0));
        let html = page(
            &[],
            std::slice::from_ref(&g),
            &nobody(),
            Finishing::default(),
        );
        assert!(html.contains("href=\"/ffff-ffff-ffff/\""));
        assert!(html.contains("href=\"/ffff-ffff-ffff/analytics\""));
        // Turns, counted the way the report counts them, not moves.
        assert_eq!(turns_of(&g), 2);
        assert!(html.contains("<td>2</td>"));
        // Seat nought is whoever was at the keyboard, so their win is a win.
        assert!(html.contains(">won</span>"));
    }

    #[test]
    fn the_page_shows_nobody_elses_games() {
        // There was a second card under the history listing every other game in
        // the store. Fine while the store held six demo games; a browsable pile
        // of other people's games on the front page once it holds theirs.
        let g = game("1111-1111-1111", "", None);
        let html = page(
            &[],
            std::slice::from_ref(&g),
            &nobody(),
            Finishing::default(),
        );
        assert!(html.contains("1111-1111-1111"), "mine is shown");
        assert!(html.contains(">unfinished</span>"));
        assert!(!html.contains("Also on this server"), "and only mine");
        // Nor is there a header link to the page's own first button.
        assert!(!html.contains("class=\"headLink\""));
    }

    #[test]
    fn a_long_history_is_capped_and_says_that_it_is() {
        let games: Vec<Saved> = (0..SHOWN + 3)
            .map(|i| game(&format!("{:04}-0000-0000", i + 1000), "Egon", Some(0)))
            .collect();
        let html = page(&[], &games, &nobody(), Finishing::default());
        assert!(html.contains("1000-0000-0000"), "the newest is shown");
        let last = format!("{:04}-0000-0000", SHOWN + 1002);
        assert!(!html.contains(&last), "the oldest is not");
        assert!(html.contains("And 3 older"));
    }

    #[test]
    fn a_name_from_a_player_cannot_write_html() {
        // Names and table names reach this page from a text field somebody else
        // typed into, and they are written into markup and into attributes.
        let sneaky = table("2222-2222-2222", true, false, None);
        let sneaky = Open {
            host: "\" onfocus=\"x".to_string(),
            game: "<script>alert(1)</script>".to_string(),
            ..sneaky
        };
        let html = page(&[sneaky], &[], &nobody(), Finishing::default());
        assert!(!html.contains("<script"), "no script on the home page");
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("onfocus=\"x"));
        assert!(html.contains("&quot; onfocus=&quot;x"));
        // And through the history, where the name of a game goes the same way.
        let g = Saved {
            name: "<b>bold</b>".to_string(),
            ..game("3333-3333-3333", "", Some(0))
        };
        let html = page(&[], &[g], &nobody(), Finishing::default());
        assert!(!html.contains("<b>bold</b>"));
        assert!(html.contains("&lt;b&gt;bold&lt;/b&gt;"));
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
