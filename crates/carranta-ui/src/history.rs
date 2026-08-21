//! Your history: the page behind the header's History.
//!
//! The account console's shape, one card, tabs on top, holding what your
//! games amount to rather than what your account is. One tab today,
//! Overview, carrying your line of the corpus; the structure is a deck
//! because the next tabs, games, opponents, whatever earns a place, arrive
//! without a redesign, which is the same argument the account page made.
//!
//! Signed-in only and no script, for the account page's reasons.

use std::fmt::Write as _;

use carranta_analytics::corpus::{Config, Corpus, Who as Actor};

use crate::account::sat;
use crate::analysis::{Finishing, player_number, to_log_as};
use crate::home::Who;
use crate::people::Aliases;
use crate::report::{CSS, ICON};
use crate::store::Saved;

/// The page, for the signed-in person `principal`.
///
/// `mine` is the games that are theirs the way the home page counts them,
/// played in or dealt, through the claims; `staying` is whether they finish
/// what they start, which the games tab says under the list.
pub fn page(
    history: &[Saved],
    mine: &[Saved],
    staying: Finishing,
    aliases: &dyn Aliases,
    principal: &str,
    who: &Who,
) -> String {
    let me = player_number(&aliases.resolve(principal));

    let mut b = String::new();
    let _ = write!(
        b,
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>Your history · Carranta</title>{ICON}\
         <style>{CSS}{EXTRA}</style></head><body>\
         {head}<main>",
        head = crate::report::masthead_as(
            "your history",
            &[],
            &crate::home::account(who),
            "history",
            true,
        ),
    );

    b.push_str(
        "<div class=\"deck\">\
         <input class=\"tabPick\" type=\"radio\" name=\"tab\" id=\"tabOverview\" \
         tabindex=\"-1\" checked>\
         <input class=\"tabPick\" type=\"radio\" name=\"tab\" id=\"tabGames\" \
         tabindex=\"-1\">\
         <nav class=\"tabs\">\
         <label for=\"tabOverview\">Overview</label>\
         <label for=\"tabGames\">Games</label>\
         </nav>",
    );
    b.push_str("<div class=\"pane paneOverview\">");
    b.push_str(&record(history, aliases, me));
    b.push_str("</div>");
    b.push_str("<div class=\"pane paneGames\">");
    b.push_str(&crate::home::played(mine, staying, true));
    b.push_str("</div></div></main></body></html>");
    b
}

/// The person's own line of the corpus, one section per configuration./// The person's own line of the corpus, one section per configuration.
///
/// Built by the same fold the across-games page uses, over only the games
/// this person sat in, and read out of the same actor table, so the numbers
/// here and the numbers there are one computation.
fn record(history: &[Saved], aliases: &dyn Aliases, me: u64) -> String {
    let mut corpora: Vec<Corpus> = Vec::new();
    for saved in history {
        if !sat(saved, aliases, me) {
            continue;
        }
        let Some(log) = to_log_as(saved, aliases) else {
            continue;
        };
        let config = Config::of(&log);
        match corpora.iter_mut().find(|c| c.config == config) {
            Some(c) => {
                c.add(&log, 0);
            }
            None => {
                let mut c = Corpus::new(config);
                c.add(&log, 0);
                corpora.push(c);
            }
        }
    }

    let mut b = String::from("<section><h2>What your games add up to</h2>");
    if corpora.is_empty() {
        b.push_str(
            "<p class=\"blurb\">Nothing yet: no finished game on this server \
             has you at the table. The first one joins this page on its own.</p>\
             </section>",
        );
        return b;
    }
    b.push_str(
        "<p class=\"blurb\">Your line of the corpus, one row per market. The \
         interval on a win rate is clustered by game, and conversion is points \
         scored above what your production predicts, the closest thing here \
         to skill with the luck taken out.</p>",
    );
    b.push_str(TABLE_OPEN);
    b.push_str(
        "<thead><tr><th>market</th><th>games</th><th>wins</th><th>win rate</th>\
         <th>mean VP</th><th>conversion</th></tr></thead><tbody>",
    );
    for c in &corpora {
        let Some(row) = c
            .actor_rows()
            .into_iter()
            .find(|r| r.who == Actor::Human { player: me })
        else {
            continue;
        };
        let share = match row.half_width {
            Some(h) if h > 0.0 => format!("{:.1}% ± {:.1}", row.share * 100.0, h * 100.0),
            _ => format!("{:.1}%", row.share * 100.0),
        };
        let _ = write!(
            b,
            "<tr><td>{:?}</td><td>{}</td><td>{}</td><td>{}</td><td>{:.2}</td><td>{}</td></tr>",
            c.config.trade_mode,
            row.games,
            row.wins,
            share,
            row.mean_vp,
            row.residual
                .map(|r| format!("{r:+.2}"))
                .unwrap_or_else(|| "·".to_string()),
        );
    }
    b.push_str(TABLE_CLOSE);
    b.push_str("</section>");
    b
}

const TABLE_OPEN: &str = "<div class=\"tw\"><table>";
const TABLE_CLOSE: &str = "</tbody></table></div>";

const EXTRA: &str = "
#tabOverview:checked ~ .tabs label[for=\"tabOverview\"],
#tabGames:checked ~ .tabs label[for=\"tabGames\"] {
  background: var(--primary); color: var(--primary-foreground); }
#tabOverview:checked ~ .paneOverview { display: block; }
#tabGames:checked ~ .paneGames { display: block; }
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
    fn the_overview_is_the_record_in_the_console_shape() {
        let history = vec![played(21, "egonkey000000000", "Egon")];
        let html = page(
            &history,
            &history,
            Finishing::default(),
            &NoAliases,
            "egonkey000000000",
            &signed_in("Egon"),
        );
        assert!(html.contains("id=\"tabOverview\""), "the overview tab");
        assert!(html.contains("id=\"tabGames\""), "the games tab");
        assert!(html.contains("Your games"), "the list moved in from home");
        assert!(html.contains("What your games add up to"));
        assert!(html.contains("<td>Full</td>"), "their row, by market");
        assert!(!html.contains("<script"), "no script on the history page");
    }

    #[test]
    fn an_empty_history_says_so() {
        let html = page(
            &[],
            &[],
            Finishing::default(),
            &NoAliases,
            "egonkey000000000",
            &signed_in("Egon"),
        );
        assert!(html.contains("Nothing yet"));
    }
}
